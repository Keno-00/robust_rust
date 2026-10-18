//! Product-scoped demonstration storage, DMP fitting, tracking regression and inference.
//! Coordinates are fractions of the dataset window width; runtime uses monitor width.
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SCHEMA: u32 = 2;
pub const PRODUCT: &str = "verux-apb";
pub const FLICK_TRIALS: usize = 60;
pub const TRACK_TRIALS: usize = 10;
pub const TRACK_SECONDS: f64 = 15.0;
pub(crate) const BASIS: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub t: f64,
    pub cursor: [f64; 2],
    pub target: [f64; 2],
    pub raw_counts: [f64; 2],
    #[serde(default)]
    pub displayed_target: Option<[f64; 2]>,
    #[serde(default = "visible_default")]
    pub visible: bool,
    #[serde(default)]
    pub target_id: usize,
}

fn visible_default() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trial {
    pub tracking: bool,
    pub scenario: usize,
    pub radius: f64,
    pub misses: usize,
    #[serde(default)]
    pub acquisition_end: Option<f64>,
    pub samples: Vec<Sample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dataset {
    pub schema: u32,
    pub product: String,
    pub id: String,
    pub width: f64,
    pub height: f64,
    pub source: String,
    pub trials: Vec<Trial>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub schema: u32,
    pub product: String,
    pub id: String,
    pub name: String,
    pub created_at: u64,
    pub datasets: Vec<String>,
    pub flick_count: usize,
    pub tracking_count: usize,
    pub tracking_rmse: f64,
    pub personality: crate::ml_talisman::Personality,
}

#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

pub fn new_id() -> Result<String, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    Ok(format!("{}-{}", stamp.as_nanos(), std::process::id()))
}

fn valid_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 80 || !id.bytes().all(|c| c.is_ascii_digit() || c == b'-') {
        return Err(
            "Invalid ML artifact ID. Collect a new dataset or select a valid model.".into(),
        );
    }
    Ok(())
}

impl Store {
    pub fn open() -> Result<Self, String> {
        let home = std::env::var_os("USERPROFILE")
            .ok_or("USERPROFILE is unavailable; ML storage cannot open.")?;
        let store = Self {
            root: PathBuf::from(home)
                .join("Documents")
                .join("VeruxAPB")
                .join(crate::protocol_names::model_directory())
                .join("personality"),
        };
        for sub in ["datasets", "models"] {
            fs::create_dir_all(store.root.join(sub))
                .map_err(|e| format!("Cannot open ML storage {}: {e}", store.root.display()))?;
        }
        Ok(store)
    }

    fn path(&self, kind: &str, id: &str) -> Result<PathBuf, String> {
        valid_id(id)?;
        Ok(self.root.join(kind).join(format!("{id}.json")))
    }

    fn read<T: serde::de::DeserializeOwned>(&self, path: &Path) -> Result<T, String> {
        let bytes = fs::read(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| format!("Invalid ML file {}: {e}", path.display()))
    }

    fn write<T: Serialize>(&self, path: &Path, value: &T, replace: bool) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(value)
            .map_err(|e| format!("Cannot serialize ML artifact: {e}"))?;
        let temporary = path.with_extension(format!("{}.tmp", new_id()?));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            if replace {
                use std::os::windows::ffi::OsStrExt;
                use windows::Win32::Storage::FileSystem::{
                    MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
                };
                let source: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
                let destination: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
                unsafe {
                    MoveFileExW(
                        windows::core::PCWSTR(source.as_ptr()),
                        windows::core::PCWSTR(destination.as_ptr()),
                        MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                    )
                }
                .map_err(std::io::Error::other)?;
            } else {
                // Atomic create without overwriting an existing model or dataset.
                fs::hard_link(&temporary, path)?;
                fs::remove_file(&temporary)?;
            }
            Ok::<_, std::io::Error>(())
        })();
        if let Err(error) = result {
            if let Err(cleanup) = fs::remove_file(&temporary) {
                if cleanup.kind() != std::io::ErrorKind::NotFound {
                    crate::log_err!("ML temporary-file cleanup failed: {cleanup}");
                }
            }
            return Err(format!("Cannot save {}: {error}", path.display()));
        }
        Ok(())
    }

    fn files(&self, kind: &str) -> Result<Vec<PathBuf>, String> {
        let mut paths = Vec::new();
        for entry in
            fs::read_dir(self.root.join(kind)).map_err(|e| format!("Cannot list ML {kind}: {e}"))?
        {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                paths.push(path);
            }
        }
        paths.sort();
        Ok(paths)
    }

    pub fn save_dataset(&self, data: &Dataset) -> Result<(), String> {
        validate_training_dataset(data)?;
        self.write(&self.path("datasets", &data.id)?, data, false)?;
        crate::log_info!(
            "ML dataset {} saved: {} flicks, {} tracking runs.",
            data.id,
            FLICK_TRIALS,
            TRACK_TRIALS
        );
        Ok(())
    }

    pub fn datasets(&self) -> Result<Vec<Dataset>, String> {
        let mut data = Vec::new();
        for path in self.files("datasets")? {
            let dataset: Dataset = self.read(&path)?;
            validate_training_dataset(&dataset).map_err(|e| format!("{}: {e}", path.display()))?;
            if self.path("datasets", &dataset.id)? != path {
                return Err(format!("Dataset ID does not match {}.", path.display()));
            }
            data.push(dataset);
        }
        Ok(data)
    }

    pub fn models(&self) -> Result<Vec<Model>, String> {
        let mut models = Vec::new();
        for path in self.files("models")? {
            let model: Model = self.read(&path)?;
            model
                .validate()
                .map_err(|error| format!("Invalid model {}: {error}", path.display()))?;
            if self.path("models", &model.id)? != path {
                return Err(format!("Model ID does not match {}.", path.display()));
            }
            models.push(model);
        }
        Ok(models)
    }

    pub fn load_model(&self, id: &str) -> Result<Model, String> {
        let model: Model = self.read(&self.path("models", id)?)?;
        model.validate()?;
        if model.id != id {
            return Err("ML model ID does not match the selected file.".into());
        }
        Ok(model)
    }

    pub fn save_model(&self, model: &Model) -> Result<(), String> {
        model.validate()?;
        self.write(&self.path("models", &model.id)?, model, false)?;
        crate::log_info!("ML model {} created: {} ({} flicks, {} tracking runs; training tracking RMSE {:.5} window widths/sec).", model.id, model.name, model.flick_count, model.tracking_count, model.tracking_rmse);
        Ok(())
    }

    pub fn rename_model(&self, id: &str, name: &str) -> Result<(), String> {
        let mut model = self.load_model(id)?;
        model.name = name.trim().to_owned();
        model.validate()?;
        self.write(&self.path("models", id)?, &model, true)?;
        crate::log_info!("ML model {id} renamed to {}.", model.name);
        Ok(())
    }

    pub fn delete_model(&self, id: &str) -> Result<(), String> {
        self.load_model(id)?;
        fs::remove_file(self.path("models", id)?)
            .map_err(|e| format!("Cannot delete ML model: {e}"))?;
        crate::log_info!("ML model {id} deleted.");
        Ok(())
    }
}

pub(crate) fn validate_training_dataset(data: &Dataset) -> Result<(), String> {
    valid_id(&data.id)?;
    if !matches!(data.source.as_str(), "color" | "memory")
        || data.schema != SCHEMA
        || data.product != PRODUCT
        || !data.width.is_finite()
        || data.width < 1.0
        || !data.height.is_finite()
        || data.height < 1.0
    {
        return Err("Incompatible ML recording. Record the new personality course; old recordings are retired.".into());
    }
    let flicks = data.trials.iter().filter(|t| !t.tracking).count();
    let tracks = data.trials.iter().filter(|t| t.tracking).count();
    if flicks != FLICK_TRIALS || tracks != TRACK_TRIALS {
        return Err("ML dataset is incomplete. Finish all scenarios before training.".into());
    }
    for trial in &data.trials {
        if trial.samples.len() < 3 || !trial.radius.is_finite() || trial.radius <= 0.0 {
            return Err("ML trial has insufficient samples or invalid target size.".into());
        }
        if trial
            .acquisition_end
            .is_some_and(|t| !t.is_finite() || t < 0.03 || t > trial.samples.last().unwrap().t)
        {
            return Err("Invalid acquisition boundary.".into());
        }
        let mut previous = -1.0;
        for sample in &trial.samples {
            if !sample.t.is_finite()
                || sample.t < 0.0
                || sample.t <= previous
                || !sample
                    .cursor
                    .iter()
                    .chain(&sample.target)
                    .chain(&sample.raw_counts)
                    .chain(sample.displayed_target.iter().flatten())
                    .all(|v| v.is_finite())
            {
                return Err("ML trial has invalid positions or non-monotonic timestamps.".into());
            }
            previous = sample.t;
        }
    }
    Ok(())
}

impl Model {
    pub fn validate(&self) -> Result<(), String> {
        valid_id(&self.id)?;
        if self.name.trim().is_empty()
            || self.name.chars().count() > 64
            || self.name.chars().any(char::is_control)
            || self.name.contains("##")
        {
            return Err("Model names must contain 1ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â€šÂ¬Ã…â€œ64 characters, without control characters or ##. Enter a valid name.".into());
        }
        if self.schema != SCHEMA
            || self.product != PRODUCT
            || self.flick_count < FLICK_TRIALS
            || self.tracking_count < TRACK_TRIALS
        {
            return Err("Incompatible ML model. Old models are retired; record the personality course and train again.".into());
        }
        self.personality.validate()?;
        if !self.tracking_rmse.is_finite() || self.tracking_rmse < 0.0 {
            return Err("Invalid tracking fit diagnostics.".into());
        }
        Ok(())
    }
}

pub fn train(data: &[Dataset], mut progress: impl FnMut(&str)) -> Result<Model, String> {
    if data.is_empty() {
        return Err("No matching complete course exists. Record Dataset for the selected coordinate source first.".into());
    }
    for d in data {
        validate_training_dataset(d)?;
    }
    crate::forge_runtime::train(data, &mut progress)
}

/// Authenticated memory-only inference; model weights remain local.
#[derive(Debug)]
pub struct Controller {
    runtime: crate::oracle_runtime::InferenceRuntime,
    source: String,
    model_id: String,
    width: i32,
    fps: u32,
}
impl Controller {
    pub fn new(model: Model, width: i32, fps: u32) -> Result<Self, String> {
        Self::prepare(model,width,fps,&std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0)),&std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)))
    }
    pub(crate) fn prepare(model: Model,width:i32,fps:u32,progress:&std::sync::Arc<std::sync::atomic::AtomicU32>,cancel:&std::sync::Arc<std::sync::atomic::AtomicBool>)->Result<Self,String> {
        let source=model.personality.source.clone();
        let model_id=model.id.clone();
        Ok(Self {runtime:crate::oracle_runtime::InferenceRuntime::prepare(&model,width,fps,progress,cancel)?,source,model_id,width,fps})
    }
    pub(crate) fn receive(model:Model,width:i32,fps:u32)->Result<Self,String> {
        let source=model.personality.source.clone();
        let model_id=model.id.clone();
        Ok(Self {runtime:crate::oracle_runtime::InferenceRuntime::receive(&model,width,fps)?,source,model_id,width,fps})
    }
    pub(crate) fn matches(&self,config:&crate::config::ProfileConfig)->bool {self.model_id==config.ml_unicorn_model && self.width==config.monitor_resolution.x && self.fps==config.fps}
    pub(crate) fn check_authorization(&self)->Result<(),String> {self.runtime.check_authorization()}
    pub fn check_source(&self,source:&str)->Result<(),String> {
        if self.source==source {Ok(())} else {Err(format!("ML personality was trained for another coordinate source. Select a {source} model or record its course."))}
    }
    pub fn incidental_vertical(&self)->bool {self.source=="color"}
    pub fn reset(&mut self) {self.runtime.reset();}
    pub fn step(&mut self,error:[f64;2],now:f64)->Result<[f64;2],String> {self.runtime.step(error,now)}
}
