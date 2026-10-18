use crate::config::ProfileConfig;
use crate::ml_unicorn::{self, Model, Store};
use imgui::Ui;
use std::process::{Child, Command};
use std::sync::mpsc::{self, Receiver, TryRecvError};

#[derive(Debug)]
enum TrainingEvent {
    Progress(String),
    Finished(Result<Model, String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MlWindow {
    Dataset,
    Preview,
}

impl MlWindow {
    fn name(self) -> &'static str {
        match self {
            Self::Dataset => "Dataset",
            Self::Preview => "Model Preview",
        }
    }
}

#[derive(Debug, Default)]
pub struct MlUi {
    models: Vec<Model>,
    initialized: bool,
    training: Option<Receiver<TrainingEvent>>,
    training_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    training_session: Option<String>,
    window: Option<(Child, MlWindow)>,
    window_loading: Option<Receiver<Result<(Child,MlWindow),String>>>,
    dataset_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    dataset_progress: Option<std::sync::Arc<std::sync::atomic::AtomicU32>>,
    status: String,
    rename: String,
    rename_id: String,
    source: Option<crate::config::CoordinateSource>,
}

impl MlUi {
    fn refresh(&mut self) -> Result<(), String> {
        self.models = Store::open()?.models()?;
        Ok(())
    }

    pub fn busy(&self) -> bool {
        self.window.is_some() || self.window_loading.is_some() || self.training.is_some()
    }

    pub fn close_window(&mut self) {
        self.window_loading=None;
        self.dataset_progress=None;
        if let Some(cancel)=self.dataset_cancel.take() {cancel.store(true,std::sync::atomic::Ordering::Release);}
        if let Some(cancel)=self.training_cancel.take() { cancel.store(true,std::sync::atomic::Ordering::Release); }
        if let Some((mut child, kind)) = self.window.take() {
            match child.try_wait() {
                Ok(Some(_)) => {}
                Ok(None) => {
                    if let Err(error) = child.kill().and_then(|_| child.wait().map(|_| ())) {
                        crate::log_err!("Cannot close owned {} process: {error}", kind.name());
                    }
                }
                Err(error) => {
                    crate::log_err!(
                        "Cannot read owned {} process status during shutdown: {error}",
                        kind.name()
                    );
                }
            }
        }
    }

    pub fn open_window(&mut self, kind: MlWindow, config: &ProfileConfig) -> Result<(), String> {
        if !config.eligibility().aim {
            return Err("Enable an eligible aim source before opening ML controls.".into());
        }
        if self.busy() {
            return Err("Close the active ML window or finish training first.".into());
        }
        let exe = std::env::current_exe()
            .map_err(|e| format!("Cannot locate ML window executable: {e}"))?;
        let mut command = Command::new(exe);
        match kind {
            MlWindow::Dataset => {
                let id = ml_unicorn::new_id()?;
                command.arg("--ml-dataset").arg(&id).arg("--ml-source").arg(
                    if config.coordinate_source == crate::config::CoordinateSource::Memory {
                        "memory"
                    } else {
                        "color"
                    },
                );
                crate::log_info!("ML dataset collection requested: {id}.");
            }
            MlWindow::Preview => {
                let model = Store::open()?.load_model(&config.ml_unicorn_model)?;
                if model.personality.source != Self::source_name(config) {
                    return Err("Select a model matching the active aim source before previewing.".into());
                }
                command
                    .arg("--ml-preview")
                    .arg(&config.ml_unicorn_model)
                    .arg(config.monitor_resolution.x.to_string())
                    .arg(config.monitor_resolution.y.to_string())
                    .arg(config.fps.to_string());
                crate::log_info!("ML preview requested for model {}.", config.ml_unicorn_model);
            }
        }
        {
            let cancel=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let progress=std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
            self.dataset_cancel=Some(cancel.clone());self.dataset_progress=Some(progress.clone());
            let (tx,rx)=mpsc::channel();self.window_loading=Some(rx);
            std::thread::spawn(move || {
                let result=match kind {MlWindow::Dataset=>crate::mosaic_runtime::launch(command,progress,cancel.clone()),MlWindow::Preview=>crate::mosaic_runtime::launch_algorithm(command,progress,cancel.clone(),"verux.apb.inference",1024*1024)}.map(|child|(child,kind));
                if let Err(undelivered)=tx.send(result) {cancel.store(true,std::sync::atomic::Ordering::Release);if let Ok((mut child,_))=undelivered.0 {let _=child.kill();let _=child.wait();}}
            });
            return Ok(());
        }
    }

    pub fn poll(&mut self, config: &mut ProfileConfig) -> Result<(), String> {
        if self.source.is_some_and(|source| source != config.coordinate_source)
            || (!config.eligibility().aim && self.busy()) {
            self.close_window();
        }
        self.source = Some(config.coordinate_source);
        if let Some(receiver)=&self.window_loading {
            match receiver.try_recv() {
                Ok(result)=>{self.window_loading=None;self.dataset_progress=None;match result {Ok(value)=>self.window=Some(value),Err(error)=>{if let Some(c)=self.dataset_cancel.take(){c.store(true,std::sync::atomic::Ordering::Release);}return Err(error);}}}
                Err(TryRecvError::Empty)=>{},
                Err(TryRecvError::Disconnected)=>{self.window_loading=None;self.dataset_progress=None;if let Some(c)=self.dataset_cancel.take(){c.store(true,std::sync::atomic::Ordering::Release);}return Err("ML preparation stopped. Reopen the selected window.".into());}
            }
        }

        if !self.initialized {
            self.initialized = true;
            self.refresh()?;
        }
        if let Some((child, kind)) = &mut self.window {
            let kind = *kind;
            if let Some(status) = child
                .try_wait()
                .map_err(|e| format!("Cannot read {} process status: {e}", kind.name()))?
            {
                self.window = None;
                if let Some(c)=self.dataset_cancel.take(){c.store(true,std::sync::atomic::Ordering::Release);}
                match status.code() {
                    Some(0) => { crate::log_info!("{} window finished.", kind.name()); }
                    Some(2) if kind == MlWindow::Dataset => { crate::log_info!("ML dataset collection canceled; previous datasets retained."); }
                    _ => return Err(format!("{} failed. Check its error dialog and Documents\\{}\\verux_apb.log, then reopen the window.", kind.name(), crate::passport::APP_DIRECTORY)),
                }
            }
        }
        if let Some(receiver) = &self.training {
            loop {
                match receiver.try_recv() {
                    Ok(TrainingEvent::Progress(message)) => self.status = message,
                    Ok(TrainingEvent::Finished(result)) => {
                        let current=self.training_cancel.as_ref().is_some_and(|c|!c.load(std::sync::atomic::Ordering::Acquire))
                            && self.training_session.as_ref().is_some_and(|h|crate::passport::check_algorithm_session(h).is_ok());
                        self.training_session=None;
                        self.training = None;
                        self.training_cancel=None;
                        if !current {self.status="Cancelled".into();break;}
                        match result {
                            Ok(model) => {
                                let compatible = model.personality.source == Self::source_name(config);
                                Store::open()?.save_model(&model)?;
                                if compatible && config.eligibility().aim { config.ml_unicorn_model = model.id; }
                                self.status = "Training complete".into();
                                self.refresh()?;
                            }
                            Err(error) => {
                                self.status = "Training failed".into();
                                return Err(error);
                            }
                        }
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.training = None;
                        self.training_cancel=None;
                        self.status = "Training failed".into();
                        return Err("ML training worker stopped without a result. Check verux_apb.log and select Train again.".into());
                    }
                }
            }
        }
        if self.models.iter().any(|model| model.id == config.ml_unicorn_model
            && model.personality.source != Self::source_name(config)) {
            config.ml_unicorn_model.clear();
            config.ml_unicorn_enabled = false;
            self.status = "Aim source changed; incompatible model selection cleared.".into();
            crate::log_info!("ML model selection cleared: model source does not match selected aim source.");
        }
        Ok(())
    }

    fn source_name(config: &ProfileConfig) -> &'static str {
        if config.coordinate_source == crate::config::CoordinateSource::Memory { "memory" } else { "color" }
    }

    fn train(&mut self, source: String) {
        self.status = "0%".into();
        let cancel=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.training_cancel=Some(cancel.clone());
        self.training_session=crate::passport::algorithm_session().ok().map(|(_,hash)|hash);
        let (tx, rx) = mpsc::channel();
        self.training = Some(rx);
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| {
                let store = Store::open()?;
                let all = store.datasets()?;
                let data: Vec<_> = all.into_iter().filter(|d| d.schema == crate::ml_talisman::SCHEMA && d.source == source).collect();
                crate::log_info!("ML training selects {source} personality recordings ({} complete courses); other-source recordings remain stored.",data.len());
                let model = crate::forge_runtime::train_cancelable(&data, &mut |status| {
                    let _ = tx.send(TrainingEvent::Progress(status.into()));
                }, cancel.clone())?;
                if cancel.load(std::sync::atomic::Ordering::Acquire) { return Err("Training cancelled.".into()); }
                Ok::<_, String>(model)
            })
            .unwrap_or_else(|_| {
                Err(
                    "ML training panicked. Check the dataset and verux_apb.log before retrying."
                        .into(),
                )
            });
            if let Err(error) = &result {
                crate::log_err!("ML training failed: {error}");
            }
            let _ = tx.send(TrainingEvent::Finished(result));
        });
    }

    /// The caller stops live quill before launching an ML window.
    pub(crate) fn draw(
        &mut self,
        ui: &Ui,
        config: &mut ProfileConfig,
        surface: &crate::petal_text::Frame,
    ) -> Result<Option<MlWindow>, String> {
        if !config.eligibility().aim { return Ok(None); }
        let disabled = ui.begin_disabled(self.busy());
        let dataset_requested = ui.button(surface.text("u118"));
        let mut requested = dataset_requested.then_some(MlWindow::Dataset);
        if ui.button(surface.text("u119")) {
            self.train(
                if config.coordinate_source == crate::config::CoordinateSource::Memory {
                    "memory".into()
                } else {
                    "color".into()
                },
            );
        }
        drop(disabled);
        if let Some(progress)=&self.dataset_progress {
            let percent=progress.load(std::sync::atomic::Ordering::Acquire).min(99);
            imgui::ProgressBar::new(percent as f32/100.0).overlay_text(format!("{percent}%")).build(ui);
            if ui.button(surface.text("u120")) { self.close_window(); }
        }
        if self.training.is_some() && ui.button(surface.text("u121")) {if let Some(c)=&self.training_cancel {c.store(true,std::sync::atomic::Ordering::Release);}}
        if let Some(percent)=self.status.strip_suffix('%').and_then(|v|v.parse::<u32>().ok()) {
            imgui::ProgressBar::new(percent.min(100) as f32/100.0).overlay_text(&self.status).build(ui);
        } else if !self.status.is_empty() {ui.text(&self.status);}


        let selected = self.models.iter().position(|m| m.id == config.ml_unicorn_model);
        let preview = selected.map(|i| self.models[i].name.as_str()).unwrap_or(
            if config.ml_unicorn_model.is_empty() {
                surface.text("u122")
            } else {
                surface.text("u123")
            },
        );
        if let Some(_combo) = ui.begin_combo(surface.text("u124"), preview) {
            for model in &self.models {
                if model.personality.source != Self::source_name(config) { continue; }
                if ui
                    .selectable_config(format!("{}##{}", model.name, model.id))
                    .selected(model.id == config.ml_unicorn_model)
                    .build()
                {
                    config.ml_unicorn_model = model.id.clone();
                }
            }
        }
        let selected = self.models.iter().position(|m| m.id == config.ml_unicorn_model);
        let disabled = ui.begin_disabled(selected.is_none() || self.busy());
        if ui.button(surface.text("u125")) {
            if let Some(i) = selected {
                self.rename = self.models[i].name.clone();
                self.rename_id = self.models[i].id.clone();
                ui.open_popup(surface.text("u126"));
            }
        }
        ui.same_line();
        if ui.button(surface.text("u127")) {
            Store::open()?.delete_model(&config.ml_unicorn_model)?;
            config.ml_unicorn_model.clear();
            config.ml_unicorn_enabled = false;
            self.refresh()?;
        }
        ui.same_line();
        if ui.button(surface.text("u128")) {
            requested = Some(MlWindow::Preview);
        }
        drop(disabled);

        let mut rename_error = None;
        ui.modal_popup_config(surface.text("u126"))
            .always_auto_resize(true)
            .build(|| {
                ui.input_text(surface.text("u129"), &mut self.rename).build();
                if ui.button(surface.text("u029")) {
                    match Store::open()
                        .and_then(|store| store.rename_model(&self.rename_id, &self.rename))
                    {
                        Ok(()) => {
                            ui.close_current_popup();
                            if let Err(e) = self.refresh() {
                                rename_error = Some(e);
                            }
                        }
                        Err(e) => rename_error = Some(e),
                    }
                }
                ui.same_line();
                if ui.button(surface.text("u130")) {
                    ui.close_current_popup();
                }
            });
        if let Some(error) = rename_error {
            return Err(error);
        }
        if ui.checkbox(surface.text("u131"), &mut config.ml_unicorn_enabled) && config.ml_unicorn_enabled {
            if let Err(error) = Store::open().and_then(|store| {
                let model = store.load_model(&config.ml_unicorn_model)?;
                let source = if config.coordinate_source == crate::config::CoordinateSource::Memory
                {
                    "memory"
                } else {
                    "color"
                };
                if model.personality.source != source {
                    return Err(format!(
                        "Selected model is for {}. Select a {source} model or record its course.",
                        model.personality.source
                    ));
                }
                Ok(model)
            }) {
                config.ml_unicorn_enabled = false;
                return Err(format!("ML Unicorn could not enable: {error}"));
            }
        }
        ui.slider(surface.text("u132"), 0.0, 100.0, &mut config.ml_unicorn_influence);
        Ok(requested)
    }
}

impl Drop for MlUi { fn drop(&mut self) { self.close_window(); } }
