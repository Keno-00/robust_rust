//! A separate native ImGui window/process for demonstration collection.
use crate::ml_unicorn::{
    Dataset, Sample, Store, Trial, FLICK_TRIALS, PRODUCT, TRACK_SECONDS, TRACK_TRIALS,
};
use glium::winit::event::{ElementState, Event, MouseButton, WindowEvent};
use glium::winit::platform::run_on_demand::EventLoopExtRunOnDemand;
use glium::Surface;
use imgui::{Condition, WindowFlags};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use std::time::{Duration, Instant};

const SIZE: [f32; 2] = [960.0, 640.0];

fn native_cursor(scale: f64) -> Result<[f64; 2], String> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::ScreenToClient;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, GetForegroundWindow, GetWindowThreadProcessId,
    };
    let mut point = POINT::default();
    unsafe {
        let window = GetForegroundWindow();
        let mut pid = 0;
        GetWindowThreadProcessId(window, Some(&mut pid));
        if pid != std::process::id() {
            return Err("Dataset lost foreground ownership. Reopen Dataset and keep it focused while recording.".into());
        }
        GetCursorPos(&mut point).map_err(|e| format!("Dataset cursor capture failed: {e}"))?;
        if !ScreenToClient(window, &mut point).as_bool() {
            return Err("Dataset cursor-to-window coordinate conversion failed. Reopen Dataset on the intended monitor.".into());
        }
    }
    Ok([f64::from(point.x) / scale, f64::from(point.y) / scale])
}

struct Session {
    runtime: crate::mosaic_runtime::DatasetRuntime,
    data: Dataset,
    current: Option<Trial>,
    began: Instant,
    cursor: [f64; 2],
    raw_counts: [f64; 2],
    visible: bool,
    target_id: usize,
    vertical_active: bool,
    acquired_at: Option<f64>,
    acquired_target: [f64; 2],
    target: [f64; 2],
    radius: f64,
    ready: bool,
}

impl Session {
    fn new(id: String, width: f64, height: f64, source: String, runtime: crate::mosaic_runtime::DatasetRuntime) -> Self {
        Self {
            runtime,
            data: Dataset {
                schema: crate::ml_talisman::SCHEMA,
                product: PRODUCT.into(),
                id,
                width,
                height,
                source,
                trials: Vec::new(),
            },
            current: None,
            began: Instant::now(),
            cursor: [width * 0.5, height * 0.55],
            raw_counts: [0.0; 2],
            visible: true,
            target_id: 0,
            vertical_active: false,
            acquired_at: None,
            acquired_target: [0.0; 2],
            target: [width * 0.5, height * 0.55],
            radius: 20.0,
            ready: false,
        }
    }

    fn tracking(&self) -> bool {
        self.data.trials.len() >= FLICK_TRIALS
    }

    fn next(&mut self) -> Result<(),String> {
        let number=self.data.trials.len();let tracking=number>=FLICK_TRIALS;
        let scenario=if tracking {number-FLICK_TRIALS}else{number/6};
        self.radius=self.runtime.call(0,number,self.data.source=="color",self.data.width,self.data.height,self.cursor,0.0,2)?;
        self.began=Instant::now();self.visible=true;self.target_id=number;self.vertical_active=false;self.acquired_at=None;
        for axis in 0..2 {self.target[axis]=self.runtime.call(0,number,self.data.source=="color",self.data.width,self.data.height,self.cursor,0.0,axis as i32)?;}
        self.current=Some(Trial {tracking,scenario,radius:self.radius,misses:0,acquisition_end:None,samples:Vec::new()});
        self.record()
    }
    fn track_position(&mut self,elapsed:f64,scenario:usize)->Result<[f64;2],String> {
        Ok([self.runtime.call(1,scenario,self.data.source=="color",self.data.width,self.data.height,self.cursor,elapsed,0)?,self.runtime.call(1,scenario,self.data.source=="color",self.data.width,self.data.height,self.cursor,elapsed,1)?])
    }

    fn scenario_name(&self) -> &'static str {
        if !self.tracking() {
            return "Directional acquisition";
        }
        if self.data.source == "color" {
            return [
                "Sustained horizontal follow",
                "Horizontal reversals",
                "Horizontal speed changes",
                "Approach (simulated)",
                "Slow horizontal follow",
                "Fast horizontal follow",
                "Brief loss and reacquisition",
                "Target switches",
                "Moving acquisition",
                "Horizontal stop/start",
            ][self.data.trials.len() - FLICK_TRIALS];
        }
        [
            "Sustained follow",
            "Reversals",
            "Speed changes",
            "Approach (simulated)",
            "Jump (measured timing; simulated screen amplitude)",
            "Fall (simulated)",
            "Brief loss and reacquisition",
            "Target switches",
            "Moving acquisition",
            "Elevation changes",
        ][self.data.trials.len() - FLICK_TRIALS]
    }

    fn record(&mut self) -> Result<(),String> {
        if let Some(trial) = &mut self.current {
            let t = if trial.tracking {
                self.began.elapsed().as_secs_f64().min(TRACK_SECONDS)
            } else {
                self.began.elapsed().as_secs_f64()
            };
            if trial.samples.last().is_none_or(|sample| t > sample.t) {
                let mut correction_target = self.target;
                if self.data.source == "color" {
                    correction_target[1] = self.cursor[1];
                } else if trial.tracking || self.acquired_at.is_some() {
                    correction_target[1]=self.runtime.call(2,usize::from(self.vertical_active),false,self.target[1],self.radius,self.cursor,0.0,0)?;
                    self.vertical_active=self.runtime.call(2,usize::from(self.vertical_active),false,self.target[1],self.radius,self.cursor,0.0,1)?!=0.0;
                }
                trial.samples.push(Sample {
                    t,
                    cursor: self.cursor,
                    target: correction_target,
                    displayed_target: Some(self.target),
                    visible: self.visible,
                    target_id: self.target_id,
                    raw_counts: self.raw_counts,
                });
            }
        }
        Ok(())
    }

    fn click(&mut self) -> Result<bool, String> {
        if (self.target[0] - self.cursor[0]).hypot(if self.data.source == "color" {
            0.0
        } else {
            self.target[1] - self.cursor[1]
        }) > self.radius
        {
            if let Some(trial) = &mut self.current {
                if !trial.tracking {
                    trial.misses += 1;
                }
            }
            self.record()?;
            return Ok(false);
        }
        if !self.ready {
            self.ready = true;
            self.next()?;
        } else if !self.tracking() && self.acquired_at.is_none() {
            self.record()?;
            let time = self.began.elapsed().as_secs_f64();
            self.acquired_at = Some(time);
            self.acquired_target = self.target;
            self.current
                .as_mut()
                .ok_or("Dataset trial was not initialized.")?
                .acquisition_end = Some(time);
        }
        Ok(false)
    }

    fn tick(&mut self) -> Result<bool, String> {
        if !self.ready {
            return Ok(false);
        }
        self.record()?;
        if !self.tracking()
            && self
                .acquired_at
                .is_some_and(|at| self.began.elapsed().as_secs_f64() - at >= 1.2)
        {
            let trial = self
                .current
                .take()
                .ok_or("Dataset trial was not initialized.")?;
            if trial.samples.len() < 3 {
                return Err("Acquisition has insufficient input samples. Record again.".into());
            }
            self.data.trials.push(trial);
            if self.data.trials.len() == FLICK_TRIALS {
                self.ready = false;
                self.target = self.track_position(0.0, 0)?;
            } else {
                self.next()?;
            }
        }
        if self
            .current
            .as_ref()
            .is_some_and(|trial| trial.samples.len() > 250_000)
        {
            return Err("Dataset recording exceeded its sample limit. Close Dataset and start a new recording.".into());
        }
        if self.tracking() && self.began.elapsed().as_secs_f64() >= TRACK_SECONDS {
            let trial = self
                .current
                .take()
                .ok_or("Tracking recording was not initialized.")?;
            self.data.trials.push(trial);
            if self.data.trials.len() == FLICK_TRIALS + TRACK_TRIALS {
                return Ok(true);
            }
            self.ready = false;
            self.target = self.track_position(0.0, self.data.trials.len() - FLICK_TRIALS)?;
        }
        Ok(false)
    }
}

pub fn run(id: String, source: String) -> Result<bool, String> {
    if !matches!(source.as_str(), "color" | "memory") {
        return Err("Dataset requires --ml-source color or memory.".into());
    }
    let runtime=crate::mosaic_runtime::DatasetRuntime::receive()?;
    let store = Store::open()?;
    let mut event_loop = glium::winit::event_loop::EventLoop::builder()
        .build()
        .map_err(|e| format!("Dataset event loop could not start: {e}"))?;
    event_loop.listen_device_events(glium::winit::event_loop::DeviceEvents::WhenFocused);
    let (window, display) = glium::backend::glutin::SimpleWindowBuilder::new()
        .with_title("Dataset")
        .with_inner_size(SIZE[0] as u32, SIZE[1] as u32)
        .build(&event_loop);
    window.set_resizable(false);
    let mut imgui = imgui::Context::create();
    imgui.set_ini_filename(None);
    imgui
        .fonts()
        .add_font(&[imgui::FontSource::DefaultFontData { config: None }]);
    let mut platform = WinitPlatform::new(&mut imgui);
    platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Default);
    let mut renderer = imgui_glium_renderer::Renderer::new(&mut imgui, &display)
        .map_err(|e| format!("Dataset renderer could not start: {e}"))?;
    let logical = window.inner_size().to_logical::<f64>(window.scale_factor());
    let mut session = Session::new(id, logical.width, logical.height, source, runtime);
    let mut last_frame = Instant::now();
    let mut focused = window.has_focus();
    let mut completed = false;
    let mut failure = None;
    let mut next_redraw = Instant::now();
    #[allow(deprecated)] // Match the application's existing winit closure loop.
    event_loop.run_on_demand(|event, target| {
        let outcome = (|| -> Result<(), String> {
            session.runtime.check()?;
            match &event {
                Event::NewEvents(_) => {
                    let now = Instant::now();
                    imgui.io_mut().update_delta_time(now - last_frame);
                    last_frame = now;
                }
                Event::DeviceEvent { event: glium::winit::event::DeviceEvent::MouseMotion { delta }, .. } if focused => {
                    session.raw_counts[0] += delta.0;
                    session.raw_counts[1] += delta.1;
                }
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => target.exit(),
                    WindowEvent::Focused(value) => {
                        focused = *value;
                        if !focused && session.ready { return Err("Dataset recording lost focus. Open Dataset and complete the scenarios with its window focused.".into()); }
                    }
                    WindowEvent::CursorMoved { position, .. } if focused => {
                        let position = position.to_logical::<f64>(window.scale_factor());
                        session.cursor = [position.x, position.y];
                        session.record()?;
                    }
                    WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } if focused => {
                        session.cursor = native_cursor(window.scale_factor())?;
                        session.click()?;
                    }
                    WindowEvent::ScaleFactorChanged { .. } => {
                        return Err("Dataset display scaling changed. Reopen Dataset on the intended monitor and record again.".into());
                    }
                    WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => display.resize((*size).into()),
                    WindowEvent::RedrawRequested => {
                        // Native positions also cover stationary quill and a
                        // cursor crossing the client edge to close the window.
                        if focused { session.cursor = native_cursor(window.scale_factor())?; }
                        if session.tick()? {
                            store.save_dataset(&session.data)?;
                            completed = true;
                            target.exit();
                        } else {
                            // Record the target that was actually displayed until
                            // this redraw; quill events between redraws use it too.
                            if session.ready && session.tracking() {
                                let t=session.began.elapsed().as_secs_f64();
                                let scenario=session.data.trials.len()-FLICK_TRIALS;
                                session.visible = session.runtime.call(4,scenario,false,session.data.width,session.data.height,session.cursor,t,0)?!=0.0;
                                let id=session.runtime.call(5,scenario,false,session.data.width,session.data.height,session.cursor,t,0)? as usize;
                                if id != session.target_id { session.vertical_active=false; }
                                session.target_id=id;
                                if scenario==3 { session.radius=session.runtime.call(6,scenario,false,session.data.width,session.data.height,session.cursor,t,0)?; }
                                session.target = session.track_position(session.began.elapsed().as_secs_f64(), session.data.trials.len() - FLICK_TRIALS)?;
                            }
                            if let Some(at)=session.acquired_at {
                                if !session.tracking() {
                                    session.target[0] = session.runtime.call(3,0,false,session.data.width,session.data.height,session.acquired_target,session.began.elapsed().as_secs_f64()-at,0)?;
                                }
                            }
                            let ui = imgui.frame();
                            ui.window("Dataset")
                                .position([0.0, 0.0], Condition::Always).size(ui.io().display_size, Condition::Always)
                                .flags(WindowFlags::NO_TITLE_BAR | WindowFlags::NO_MOVE | WindowFlags::NO_RESIZE | WindowFlags::NO_COLLAPSE | WindowFlags::NO_SCROLLBAR | WindowFlags::NO_SAVED_SETTINGS)
                                .build(|| {
                                    ui.text(format!("{} course: {}",session.data.source,session.scenario_name()));
                                    ui.text(if session.acquired_at.is_some() && !session.tracking() { "Keep following until the next target appears" } else if !session.ready { "Click the starting target to begin" } else if session.tracking() { if session.data.source == "memory" { "Follow horizontally; correct vertically only to enter the body band" } else { "Follow horizontally; keep your natural hand movement" } } else { "Click the targets as accurately as possible" });
                                    if session.visible { ui.get_window_draw_list().add_circle([session.target[0] as f32, session.target[1] as f32], session.radius as f32, [0.95, 0.35, 0.30, 1.0]).filled(true).build();
                                    if session.data.source == "memory" && session.tracking() {
                                        ui.get_window_draw_list().add_rect([(session.target[0]-session.radius) as f32,(session.target[1]-session.radius*2.0) as f32],[(session.target[0]+session.radius) as f32,(session.target[1]+session.radius*2.0) as f32],[0.95,0.35,0.30,1.0]).build();
                                    } }
                                });
                            let mut frame = display.draw();
                            frame.clear_color_srgb(0.05, 0.05, 0.05, 1.0);
                            platform.prepare_render(ui, &window);
                            renderer.render(&mut frame, imgui.render()).map_err(|e| format!("Dataset rendering failed: {e}"))?;
                            frame.finish().map_err(|e| format!("Dataset presentation failed: {e}"))?;
                        }
                    }
                    _ => {}
                },
                Event::AboutToWait => {
                    if Instant::now() >= next_redraw {
                        platform.prepare_frame(imgui.io_mut(), &window).map_err(|e| format!("Dataset frame preparation failed: {e}"))?;
                        window.request_redraw();
                        next_redraw = Instant::now() + Duration::from_millis(8);
                    }
                    target.set_control_flow(glium::winit::event_loop::ControlFlow::WaitUntil(next_redraw));
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(error) = outcome { failure = Some(error); target.exit(); }
        platform.handle_event(imgui.io_mut(), &window, &event);
    }).map_err(|e| format!("Dataset window failed: {e}"))?;
    if let Some(error) = failure {
        return Err(error);
    }
    Ok(completed)
}
