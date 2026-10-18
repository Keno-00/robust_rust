//! Visual closed-loop preview of the saved controller; no product quill is sent.
use crate::quill::MouseCountQuantizer;
use crate::ml_unicorn::{Controller, Model, Store};
use glium::winit::event::{Event, WindowEvent};
use glium::winit::platform::run_on_demand::EventLoopExtRunOnDemand;
use glium::Surface;
use imgui::{Condition, WindowFlags};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flick,
    Tracking,
}

struct Preview {
    model: Model,
    controller: Controller,
    quantizer: MouseCountQuantizer,
    width: i32,
    height: i32,
    fps: u32,
    mode: Mode,
    pattern: usize,
    paused: bool,
    failure: Option<String>,
    cursor: [f64; 2],
    target: [f64; 2],
    time: f64,
    pending_time: f64,
    flick_index: usize,
    trail: VecDeque<[f64; 2]>,
    target_trail: VecDeque<[f64; 2]>,
    error_total: f64,
    frames: usize,
    vertical_active: bool,
}

impl Preview {
    fn new(model: Model, width: i32, height: i32, fps: u32) -> Result<Self, String> {
        if !(4..=1000).contains(&fps) {
            return Err("Model preview requires FPS between 4 and 1000. Correct General settings and retry.".into());
        }
        if height <= 0 {
            return Err("Model preview requires a valid monitor height. Correct General settings and retry.".into());
        }
        let controller = Controller::receive(model.clone(), width, fps)?;
        let mut preview = Self {
            model,
            controller,
            quantizer: MouseCountQuantizer::default(),
            width,
            height,
            fps,
            mode: Mode::Flick,
            pattern: 0,
            paused: false,
            failure: None,
            cursor: [0.0; 2],
            target: [0.0; 2],
            time: 0.0,
            pending_time: 0.0,
            flick_index: 0,
            trail: VecDeque::new(),
            target_trail: VecDeque::new(),
            error_total: 0.0,
            frames: 0,
            vertical_active: false,
        };
        preview.target = preview.flick_target();
        preview.restart();
        Ok(preview)
    }

    fn flick_target(&self) -> [f64; 2] {
        let positions = [
            [0.8, 0.3],
            [0.2, 0.7],
            [0.8, 0.7],
            [0.2, 0.3],
            [0.5, 0.2],
            [0.5, 0.8],
            [0.7, 0.5],
            [0.3, 0.5],
        ];
        let mut p = positions[self.flick_index % positions.len()];
        if self.controller.incidental_vertical() {
            p[1] = 0.5;
        }
        [p[0] * f64::from(self.width), p[1] * f64::from(self.height)]
    }

    fn tracking_target(&self, time: f64) -> [f64; 2] {
        let (x, mut y) = match self.pattern {
            0 => (time.sin().asin() * 2.0 / std::f64::consts::PI, 0.0),
            1 => (time.cos(), time.sin()),
            _ => ((time + 0.4 * (time * 2.3).sin()).sin(), (time * 1.4).sin()),
        };
        if self.controller.incidental_vertical() {
            y = 0.0;
        }
        [
            f64::from(self.width) * (0.5 + 0.3 * x),
            f64::from(self.height) * (0.5 + 0.3 * y),
        ]
    }

    fn restart(&mut self) {
        self.cursor = [f64::from(self.width) * 0.5, f64::from(self.height) * 0.5];
        self.reset_motion();
        self.paused = false;
        self.failure = None;
        if self.mode == Mode::Tracking {
            self.target = self.tracking_target(0.0);
        }
    }

    fn reset_motion(&mut self) {
        self.controller.reset();
        self.vertical_active = false;
        self.quantizer.reset();
        self.time = 0.0;
        self.pending_time = 0.0;
        self.trail.clear();
        self.target_trail.clear();
        self.trail.push_back(self.cursor);
        self.error_total = 0.0;
        self.frames = 0;
    }

    fn stop(&mut self, error: String) {
        crate::log_err!("ML preview stopped for model {}: {error}", self.model.id);
        self.failure = Some(error);
        self.paused = true;
    }

    fn update(&mut self, wall_dt: f64) {
        if self.paused || self.failure.is_some() {
            return;
        }
        if !wall_dt.is_finite() || wall_dt < 0.0 || wall_dt > 0.25 {
            self.stop("Preview timing stalled. Select Restart to resume the model preview.".into());
            return;
        }
        let dt = 1.0 / f64::from(self.fps);
        self.pending_time += wall_dt;
        while self.pending_time >= dt {
            self.pending_time -= dt;
            self.time += dt;
            if self.mode == Mode::Tracking {
                self.target = self.tracking_target(self.time);
            }
            let mut error = [
                self.target[0] - self.cursor[0],
                self.target[1] - self.cursor[1],
            ];
            if self.controller.incidental_vertical() {
                error[1] = 0.0;
            } else {
                if !self.vertical_active && error[1].abs() > 36.0 {
                    self.vertical_active = true;
                }
                if self.vertical_active && error[1].abs() <= 18.0 {
                    self.vertical_active = false;
                }
                error[1] = if self.vertical_active {
                    error[1].signum() * (error[1].abs() - 18.0)
                } else {
                    0.0
                };
            }
            let mut delta = match self.controller.step(error, self.time) {
                Ok(delta) => delta,
                Err(error) => {
                    self.stop(format!("{error} Select Restart or train a new model."));
                    return;
                }
            };
            if !self.controller.incidental_vertical() && error[1] == 0.0 {
                delta[1] = 0.0;
            }
            let (dx, dy) = self.quantizer.quantize(delta);
            self.cursor[0] += f64::from(dx);
            self.cursor[1] += f64::from(dy);
            self.trail.push_back(self.cursor);
            self.target_trail.push_back(self.target);
            while self.trail.len() > self.fps as usize * 4 {
                self.trail.pop_front();
            }
            while self.target_trail.len() > self.fps as usize * 4 {
                self.target_trail.pop_front();
            }
            self.error_total +=
                (self.target[0] - self.cursor[0]).hypot(self.target[1] - self.cursor[1]);
            self.frames += 1;
            if self.cursor[0] < 0.0
                || self.cursor[0] > f64::from(self.width)
                || self.cursor[1] < 0.0
                || self.cursor[1] > f64::from(self.height)
            {
                self.stop("Model moved outside the simulated screen. Select Restart or retrain this model.".into());
                return;
            }
        }
    }

    fn draw(&mut self, ui: &imgui::Ui) {
        ui.text(&self.model.name);
        ui.text(format!(
            "{} personality course",
            self.model.personality.source
        ));
        ui.text("Target: red     Simulated cursor: cyan     100% model");
        if ui.radio_button("Flick", &mut self.mode, Mode::Flick) {
            self.target = self.flick_target();
            self.restart();
        }
        ui.same_line();
        if ui.radio_button("Tracking", &mut self.mode, Mode::Tracking) {
            self.restart();
        }
        ui.same_line();
        let disabled = ui.begin_disabled(self.failure.is_some());
        if ui.button(if self.paused { "Resume" } else { "Pause" }) {
            self.paused = !self.paused;
        }
        drop(disabled);
        ui.same_line();
        if ui.button("Restart") {
            self.restart();
        }
        if self.mode == Mode::Flick {
            ui.same_line();
            let disabled = ui.begin_disabled(self.failure.is_some());
            if ui.button("Next Target") {
                self.flick_index += 1;
                self.target = self.flick_target();
                self.reset_motion();
            }
            drop(disabled);
        } else if ui.combo_simple_string(
            "Path",
            &mut self.pattern,
            &["Horizontal", "Circle", "Changing Speed"],
        ) {
            self.restart();
        }
        let error = (self.target[0] - self.cursor[0]).hypot(self.target[1] - self.cursor[1]);
        ui.text(format!(
            "Error: {error:.1} px     Mean error: {:.1} px",
            if self.frames == 0 {
                error
            } else {
                self.error_total / self.frames as f64
            }
        ));
        if let Some(error) = &self.failure {
            ui.text_wrapped(error);
        }

        let available = ui.content_region_avail();
        if available[0] <= 1.0 || available[1] <= 1.0 {
            return;
        }
        let top = ui.cursor_screen_pos();
        let scale = (available[0] / self.width as f32).min(available[1] / self.height as f32);
        let origin = [
            top[0] + (available[0] - self.width as f32 * scale) * 0.5,
            top[1] + (available[1] - self.height as f32 * scale) * 0.5,
        ];
        let end = [
            origin[0] + self.width as f32 * scale,
            origin[1] + self.height as f32 * scale,
        ];
        ui.invisible_button("model_canvas", available);
        if self.mode == Mode::Flick && self.failure.is_none() && ui.is_item_clicked() {
            let p = ui.io().mouse_pos;
            if p[0] >= origin[0] && p[0] <= end[0] && p[1] >= origin[1] && p[1] <= end[1] {
                self.target = [
                    f64::from((p[0] - origin[0]) / scale),
                    f64::from((p[1] - origin[1]) / scale),
                ];
                self.reset_motion();
            }
        }
        let point = |p: [f64; 2]| {
            [
                origin[0] + p[0] as f32 * scale,
                origin[1] + p[1] as f32 * scale,
            ]
        };
        let draw = ui.get_window_draw_list();
        draw.add_rect(origin, end, [0.05, 0.05, 0.05, 1.0])
            .filled(true)
            .build();
        draw.add_rect(origin, end, [0.3, 0.3, 0.3, 1.0]).build();
        draw.with_clip_rect(origin, end, || {
            for (trail, color) in [
                (&self.target_trail, [0.7, 0.25, 0.25, 0.4]),
                (&self.trail, [0.2, 0.85, 0.95, 0.65]),
            ] {
                let mut previous = None;
                for p in trail {
                    if let Some(before) = previous {
                        draw.add_line(point(before), point(*p), color).build();
                    }
                    previous = Some(*p);
                }
            }
            draw.add_circle(point(self.target), 11.0, [1.0, 0.3, 0.3, 1.0])
                .thickness(2.0)
                .build();
            draw.add_circle(point(self.cursor), 5.0, [0.2, 0.85, 0.95, 1.0])
                .filled(true)
                .build();
        });
    }
}

pub fn run(id: &str, width: i32, height: i32, fps: u32) -> Result<(), String> {
    let model = Store::open()?.load_model(id)?;
    crate::log_info!("ML preview opened: model {} ({}), {width}x{height}, {fps} FPS, full model output, simulated 1 px/count response; no Arduino writes.", model.id, model.name);
    let mut preview = Preview::new(model, width, height, fps)?;
    let mut event_loop = glium::winit::event_loop::EventLoop::builder()
        .build()
        .map_err(|e| format!("Preview event loop could not start: {e}"))?;
    let (window, display) = glium::backend::glutin::SimpleWindowBuilder::new()
        .with_title("Model Preview")
        .with_inner_size(960, 640)
        .build(&event_loop);
    let mut imgui = imgui::Context::create();
    imgui.set_ini_filename(None);
    imgui
        .fonts()
        .add_font(&[imgui::FontSource::DefaultFontData { config: None }]);
    let mut platform = WinitPlatform::new(&mut imgui);
    platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Default);
    let mut renderer = imgui_glium_renderer::Renderer::new(&mut imgui, &display)
        .map_err(|e| format!("Preview renderer could not start: {e}"))?;
    let mut last_frame = Instant::now();
    let mut last_draw = Instant::now();
    let mut next_redraw = Instant::now();
    let mut failure = None;
    #[allow(deprecated)] // Same closure-loop integration as the dataset window.
    event_loop
        .run_on_demand(|event, target| {
            let outcome = (|| -> Result<(), String> {
                preview.controller.check_authorization()?;
                match &event {
                    Event::NewEvents(_) => {
                        let now = Instant::now();
                        imgui.io_mut().update_delta_time(now - last_frame);
                        last_frame = now;
                    }
                    Event::WindowEvent { event, .. } => match event {
                        WindowEvent::CloseRequested => target.exit(),
                        WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                            display.resize((*size).into())
                        }
                        WindowEvent::RedrawRequested => {
                            let now = Instant::now();
                            preview.update(now.duration_since(last_draw).as_secs_f64());
                            last_draw = now;
                            let ui = imgui.frame();
                            ui.window("Model Preview")
                                .position([0.0, 0.0], Condition::Always)
                                .size(ui.io().display_size, Condition::Always)
                                .flags(
                                    WindowFlags::NO_TITLE_BAR
                                        | WindowFlags::NO_MOVE
                                        | WindowFlags::NO_RESIZE
                                        | WindowFlags::NO_COLLAPSE
                                        | WindowFlags::NO_SAVED_SETTINGS,
                                )
                                .build(|| preview.draw(ui));
                            let mut frame = display.draw();
                            frame.clear_color_srgb(0.05, 0.05, 0.05, 1.0);
                            platform.prepare_render(ui, &window);
                            renderer
                                .render(&mut frame, imgui.render())
                                .map_err(|e| format!("Preview rendering failed: {e}"))?;
                            frame
                                .finish()
                                .map_err(|e| format!("Preview presentation failed: {e}"))?;
                        }
                        _ => {}
                    },
                    Event::AboutToWait => {
                        if Instant::now() >= next_redraw {
                            platform
                                .prepare_frame(imgui.io_mut(), &window)
                                .map_err(|e| format!("Preview frame preparation failed: {e}"))?;
                            window.request_redraw();
                            next_redraw = Instant::now() + Duration::from_millis(8);
                        }
                        target.set_control_flow(glium::winit::event_loop::ControlFlow::WaitUntil(
                            next_redraw,
                        ));
                    }
                    _ => {}
                }
                Ok(())
            })();
            if let Err(error) = outcome {
                failure = Some(error);
                target.exit();
            }
            platform.handle_event(imgui.io_mut(), &window, &event);
        })
        .map_err(|e| format!("Model preview window failed: {e}"))?;
    if let Some(error) = failure {
        return Err(error);
    }
    Ok(())
}
