use crate::config::{
    UnicornActivation, UnicornLineOrigin, CoordinateSource, ArcCurve, AuroraStyle, ProfileConfig,
    LanternFilter, VelvetEffect, TideApply, Rgb, BeesmasType, OrbitOrder,
    PegasusActivation, Vec2i,
};
use crate::quill::QuillBackend;
use glium::texture::{RawImage2d, Texture2d};
use glium::uniforms::{
    MagnifySamplerFilter, MinifySamplerFilter, SamplerBehavior, SamplerWrapFunction,
};
use glium::Surface;
use imgui::{Condition, Image, TabBar, TabItem, TextureId, Ui, WindowFlags};
use imgui_glium_renderer::{Renderer, Texture};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use std::error::Error;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

pub const APP_VERSION: &str = "2.1";
pub const APP_NAME: &str = "Verux.APB";
pub const WINDOW_SIZE: [f32; 2] = [500.0, 500.0];
pub const BANNER_SIZE: [f32; 2] = [480.0, 50.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingTarget {
    Pause,
    Block,
    Unicorn,
    Pegasus,
    Aurora,
    Lanterns,
}

#[derive(Debug)]
pub struct UiState {
    visual_selection: usize,
    risk_snapshot: Option<Vec<crate::sb_protect::Risk>>,
    risk_enabled: bool,
    pub ml: crate::ml_ui::MlUi,
    pub config: ProfileConfig,
    pub logged_in: bool,
    pub license_key: String,
    pub login_notify: String,
    login_result: Option<Receiver<crate::passport::VerifyResponse>>,
    catalog_result: Option<Receiver<Result<std::sync::Arc<crate::petal_text::Catalog>, String>>>,
    catalog: Option<std::sync::Arc<crate::petal_text::Catalog>>,
    heartbeat_result: Option<Receiver<Option<crate::passport::VerifyResponse>>>,
    next_heartbeat_poll: Instant,
    pub auth_notify: String,
    pub status: String,
    pub current_save_entry_name: String,
    pub selected_com_port: usize,
    pub com_ports: Vec<String>,
    pub apb_pid: Option<u32>,
    pub apb_process_status: String,
    pub echo_service_status: String,
    pub echo_service_result: Option<Receiver<Result<String, String>>>,
    memory_status_result: Option<Receiver<(Result<u32, String>, Result<String, String>)>>,
    next_memory_status_check: Instant,
    pub quill_backend: QuillBackend,
    pub pegasus_bounds: [i32; 2],
    pub color_picker_tolerance: i32,
    pub show_targeting_tab: bool,
    pub lines_color: [f32; 3],
    pub halo_color: [f32; 3],
    pub ui_color: [f32; 3],
    pub profiles: Vec<String>,
    pub selected_profile: usize,
    pub start_requested: bool,
    preparation: Option<Preparation>,
    prepared_runtime: Option<crate::algorithm_runtime::AlgorithmRuntime>,
    pub binding_target: Option<BindingTarget>,
    pub awaiting_release: bool,
    pub runner_shared: Option<std::sync::Arc<std::sync::Mutex<crate::runner::RunnerControl>>>,
    pub runner_thread: Option<std::thread::JoinHandle<()>>,
}

impl UiState {
    pub fn new(com_ports: Vec<String>) -> std::io::Result<Self> {
        let profiles = crate::config::get_profile_names()?;
        let selected_profile = profiles.iter().position(|p| p == "default").unwrap_or(0);
        let config = crate::config::load_profile(&profiles[selected_profile])?.unwrap_or_default();
        let license_key = crate::config::load_passport_key();
        let selected_com_port = com_ports
            .iter()
            .position(|port| port.eq_ignore_ascii_case(&config.com_port))
            .or_else(|| {
                com_ports
                    .iter()
                    .position(|port| port.eq_ignore_ascii_case("COM1"))
            })
            .unwrap_or(0);
        Ok(Self {
            visual_selection: 0,
            risk_snapshot: None,
            risk_enabled: false,
            ml: crate::ml_ui::MlUi::default(),
            lines_color: rgb_to_float(config.lines_color),
            halo_color: rgb_to_float(config.halo_color),
            ui_color: rgb_to_float(config.ui_color),
            config,
            logged_in: false,
            license_key,
            login_notify: String::new(),
            login_result: None,
            catalog_result: None,
            catalog: None,
            heartbeat_result: None,
            next_heartbeat_poll: Instant::now(),
            auth_notify: String::new(),
            status: String::new(),
            current_save_entry_name: profiles[selected_profile].clone(),
            selected_com_port,
            com_ports,
            apb_pid: None,
            apb_process_status: "Checking...".into(),
            echo_service_status: "Checking...".into(),
            echo_service_result: None,
            memory_status_result: None,
            next_memory_status_check: Instant::now(),
            quill_backend: QuillBackend::Arduino,
            pegasus_bounds: [10, 10],
            color_picker_tolerance: 0,
            show_targeting_tab: false,
            profiles,
            selected_profile,
            start_requested: false,
            preparation: None,
            prepared_runtime: None,
            binding_target: None,
            awaiting_release: false,
            runner_shared: None,
            runner_thread: None,
        })
    }
}

#[derive(Debug)]
struct Preparation {
    result: Receiver<Result<Option<crate::algorithm_runtime::AlgorithmRuntime>, String>>,
    progress: std::sync::Arc<std::sync::atomic::AtomicU32>,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for Preparation {
    fn drop(&mut self) {
        self.cancelled.store(true, std::sync::atomic::Ordering::Release);
    }
}

struct ClipboardSupport(arboard::Clipboard);

impl imgui::ClipboardBackend for ClipboardSupport {
    fn get(&mut self) -> Option<String> {
        self.0.get_text().ok()
    }
    fn set(&mut self, value: &str) {
        let _ = self.0.set_text(value.to_owned());
    }
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let event_loop = glium::winit::event_loop::EventLoop::builder().build()?;
    let (window, display) = glium::backend::glutin::SimpleWindowBuilder::new()
        .with_title(APP_NAME)
        .with_inner_size(WINDOW_SIZE[0] as u32, WINDOW_SIZE[1] as u32)
        .build(&event_loop);
    window.set_resizable(false);
    window.set_decorations(true);

    if let Ok(icon_image) = image::load_from_memory(include_bytes!("../assets/veruxapp.ico")) {
        let icon_rgba = icon_image.to_rgba8();
        let (width, height) = icon_rgba.dimensions();
        if let Ok(window_icon) =
            glium::winit::window::Icon::from_rgba(icon_rgba.into_raw(), width, height)
        {
            window.set_window_icon(Some(window_icon));
        }
    }

    let mut imgui = imgui::Context::create();
    imgui.set_ini_filename(None);
    if let Ok(clipboard) = arboard::Clipboard::new() {
        imgui.set_clipboard_backend(ClipboardSupport(clipboard));
    }
    imgui
        .fonts()
        .add_font(&[imgui::FontSource::DefaultFontData { config: None }]);

    let mut platform = WinitPlatform::new(&mut imgui);
    platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Default);

    let mut renderer = Renderer::new(&mut imgui, &display)?;
    let banner_texture = load_banner_texture(&display, &mut renderer)?;
    let mut last_frame = Instant::now();
    let mut state = UiState::new(serialport::available_ports()?
        .into_iter().map(|port| port.port_name).collect())?;

    apply_theme(&mut imgui, state.config.ui_color);

    let proxy = event_loop.create_proxy();

    #[allow(deprecated)]
    event_loop.run(move |event, window_target| match event {
        glium::winit::event::Event::UserEvent(()) => {
            window.set_visible(true);
            let passport_error = state.runner_shared.as_ref().and_then(|shared| {
                shared
                    .lock()
                    .ok()
                    .and_then(|mut control| control.passport_error.take())
            });
            if let Some(message) = passport_error {
                state.ml.close_window();
                state.logged_in = false;
                state.heartbeat_result = None;
                state.start_requested = false;
                state.preparation = None;
                state.prepared_runtime = None;
                state.auth_notify = message;
            }
            let runtime_error = state.runner_shared.as_ref().and_then(|shared| {
                shared
                    .lock()
                    .ok()
                    .and_then(|mut control| control.runtime_error.take())
            });
            if let Some(message) = runtime_error {
                state.start_requested = false;
                let text: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
                let title: Vec<u16> = "Verux Runtime Error"
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                unsafe {
                    windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                        None,
                        windows::core::PCWSTR(text.as_ptr()),
                        windows::core::PCWSTR(title.as_ptr()),
                        windows::Win32::UI::WindowsAndMessaging::MB_OK
                            | windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
                    );
                }
            }
        }
        glium::winit::event::Event::NewEvents(_) => {
            let now = Instant::now();
            imgui.io_mut().update_delta_time(now - last_frame);
            last_frame = now;
        }
        glium::winit::event::Event::AboutToWait => {
            platform
                .prepare_frame(imgui.io_mut(), &window)
                .expect("Failed to prepare imgui frame");
            window.request_redraw();
        }
        glium::winit::event::Event::WindowEvent {
            event: glium::winit::event::WindowEvent::RedrawRequested,
            ..
        } => {
            if state.logged_in {
                if let Err(error) = state.ml.poll(&mut state.config) {
                    show_error(&error);
                }
                if let Some(res) = poll_ui_heartbeat(&mut state) {
                    if !res.valid {
                        state.ml.close_window();
                        state.logged_in = false;
                        state.heartbeat_result = None;
                        state.preparation = None;
                        state.prepared_runtime = None;
                        state.start_requested = false;
                        state.auth_notify = res.message.clone();
                        if let Some(ref shared) = state.runner_shared {
                            if let Ok(mut lock) = shared.lock() {
                                lock.active = false;
                                lock.exit = true;
                            }
                        }
                        if let Some(handle) = state.runner_thread.take() {
                            let _ = handle.join();
                        }
                        state.runner_shared = None;
                        window.set_visible(true);
                    }
                }
            }

            if let Some(target) = state.binding_target {
                let mut any_pressed = false;
                unsafe {
                    for i in 1..256 {
                        if windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(i) < 0 {
                            any_pressed = true;
                            if !state.awaiting_release {
                                let vk_code = i as u32;
                                let name = get_vk_name(vk_code);
                                match target {
                                    BindingTarget::Pause => {
                                        state.config.pause_key = vk_code;
                                        state.config.pause_key_name = name;
                                    }
                                    BindingTarget::Block => {
                                        state.config.block_key = vk_code;
                                        state.config.block_key_name = name;
                                    }
                                    BindingTarget::Unicorn => {
                                        state.config.unicorn_key = vk_code;
                                        state.config.unicorn_key_name = name;
                                    }
                                    BindingTarget::Pegasus => {
                                        state.config.pegasus_key = vk_code;
                                        state.config.pegasus_key_name = name;
                                    }
                                    BindingTarget::Lanterns => {
                                        state.config.lantern_key = vk_code;
                                        state.config.lantern_key_name = name;
                                    }
                                    BindingTarget::Aurora => {
                                        state.config.aurora_key = vk_code;
                                        state.config.aurora_key_name = name;
                                    }
                                }
                                if let Err(error) = crate::config::save_profile(
                                    &state.current_save_entry_name,
                                    &state.config,
                                ) { show_error(&format!("Save config failed: {error}")); }
                                state.binding_target = None;
                                break;
                            }
                        }
                    }
                }
                if !any_pressed && state.awaiting_release {
                    state.awaiting_release = false;
                }
            }

            apply_theme(&mut imgui, float_to_rgb(state.ui_color));
            let ui = imgui.frame();
            draw_primary_window(ui, &mut state, banner_texture);

            let mut target = display.draw();
            target.clear_color_srgb(0.0, 0.0, 0.0, 1.0);
            platform.prepare_render(ui, &window);
            let draw_data = imgui.render();
            renderer
                .render(&mut target, draw_data)
                .expect("Rendering failed");
            target.finish().expect("Failed to swap buffers");

            if state.start_requested {
                state.start_requested = false;
                if let Err(error) = state.config.validate_eligibility() {
                    state.prepared_runtime = None;
                    show_error(&error);
                    return;
                }
                let algorithms = state.prepared_runtime.take();
                if let Err(error) = crate::config::save_profile(&state.current_save_entry_name, &state.config) {
                    state.prepared_runtime = None;
                    show_error(&format!("Save config failed: {error}"));
                    return;
                }
                window.set_visible(false);
                let config = state.config.effective_config();
                let com_port = if state.com_ports.is_empty() {
                    Some("COM1".to_string())
                } else {
                    Some(state.com_ports[state.selected_com_port].clone())
                };
                state.config.com_port = com_port.clone().unwrap_or_else(|| "COM1".to_string());

                if let Some(ref shared) = state.runner_shared {
                    if let Ok(mut lock) = shared.lock() {
                        lock.exit = true;
                    }
                }
                if let Some(handle) = state.runner_thread.take() {
                    let _ = handle.join();
                }

                let shared =
                    std::sync::Arc::new(std::sync::Mutex::new(crate::runner::RunnerControl {
                        active: true,
                        config,
                        com_port,
                        exit: false,
                        thread_running: true,
                        trigger_notice: None,
                        passport_error: None,
                        runtime_error: None,
                    }));
                state.runner_shared = Some(shared.clone());
                let thread_proxy = proxy.clone();
                let handle = std::thread::spawn(move || {
                    crate::runner::run_action_loop(shared, thread_proxy, algorithms);
                });
                state.runner_thread = Some(handle);

            }
        }
        glium::winit::event::Event::WindowEvent {
            event: glium::winit::event::WindowEvent::CloseRequested,
            ..
        } => {
            state.preparation = None;
            state.prepared_runtime = None;
            state.ml.close_window();
            if let Some(ref shared) = state.runner_shared {
                if let Ok(mut lock) = shared.lock() {
                    lock.exit = true;
                }
            }
            if let Some(handle) = state.runner_thread.take() {
                let _ = handle.join();
            }
            crate::passport::end_usage_session();
            window_target.exit();
        }
        glium::winit::event::Event::WindowEvent {
            event: glium::winit::event::WindowEvent::Resized(new_size),
            ..
        } => {
            if new_size.width > 0 && new_size.height > 0 {
                display.resize((new_size.width, new_size.height));
            }
        }
        event => {
            platform.handle_event(imgui.io_mut(), &window, &event);
        }
    })?;

    Ok(())
}

fn load_banner_texture<F: glium::backend::Facade>(
    display: &F,
    renderer: &mut Renderer,
) -> Result<TextureId, Box<dyn Error>> {
    let image = image::load_from_memory(include_bytes!("../assets/verux.png"))?.to_rgba8();
    let dimensions = image.dimensions();
    let raw = RawImage2d::from_raw_rgba(image.into_raw(), dimensions);
    let texture = Texture2d::new(display, raw)?;
    let sampler = SamplerBehavior {
        minify_filter: MinifySamplerFilter::Linear,
        magnify_filter: MagnifySamplerFilter::Linear,
        wrap_function: (
            SamplerWrapFunction::Clamp,
            SamplerWrapFunction::Clamp,
            SamplerWrapFunction::Clamp,
        ),
        ..Default::default()
    };
    Ok(renderer.textures().insert(Texture {
        texture: Rc::new(texture),
        sampler,
    }))
}

fn apply_theme(imgui: &mut imgui::Context, color: Rgb) {
    let style = imgui.style_mut();
    let r = color.r as f32 / 255.0;
    let g = color.g as f32 / 255.0;
    let b = color.b as f32 / 255.0;
    let dim = [
        ((color.r as i16 - 35).max(0) as f32) / 255.0,
        ((color.g as i16 - 35).max(0) as f32) / 255.0,
        ((color.b as i16 - 35).max(0) as f32) / 255.0,
        1.0,
    ];
    let active = [r, g, b, 1.0];

    // Backgrounds
    style.colors[imgui::StyleColor::WindowBg as usize] = [0.10, 0.10, 0.10, 1.0];
    style.colors[imgui::StyleColor::ChildBg as usize] = [0.10, 0.10, 0.10, 1.0];
    style.colors[imgui::StyleColor::PopupBg as usize] = [0.15, 0.15, 0.15, 1.0];

    // Borders
    style.colors[imgui::StyleColor::Border as usize] = [0.25, 0.25, 0.25, 1.0];
    style.colors[imgui::StyleColor::BorderShadow as usize] = [0.0, 0.0, 0.0, 0.0];

    // Frame
    style.colors[imgui::StyleColor::FrameBg as usize] = [0.15, 0.15, 0.15, 1.0];
    style.colors[imgui::StyleColor::FrameBgHovered as usize] = dim;
    style.colors[imgui::StyleColor::FrameBgActive as usize] = active;

    // Title
    style.colors[imgui::StyleColor::TitleBg as usize] = [0.12, 0.12, 0.12, 1.0];
    style.colors[imgui::StyleColor::TitleBgActive as usize] = active;
    style.colors[imgui::StyleColor::TitleBgCollapsed as usize] = [0.12, 0.12, 0.12, 1.0];

    // Scrollbar
    style.colors[imgui::StyleColor::ScrollbarBg as usize] = [0.10, 0.10, 0.10, 1.0];
    style.colors[imgui::StyleColor::ScrollbarGrab as usize] = [0.25, 0.25, 0.25, 1.0];
    style.colors[imgui::StyleColor::ScrollbarGrabHovered as usize] = dim;
    style.colors[imgui::StyleColor::ScrollbarGrabActive as usize] = active;

    // CheckMark
    style.colors[imgui::StyleColor::CheckMark as usize] = active;

    // Slider
    style.colors[imgui::StyleColor::SliderGrab as usize] = active;
    style.colors[imgui::StyleColor::SliderGrabActive as usize] = active;

    // Button
    style.colors[imgui::StyleColor::Button as usize] = [0.18, 0.18, 0.18, 1.0];
    style.colors[imgui::StyleColor::ButtonHovered as usize] = active;
    style.colors[imgui::StyleColor::ButtonActive as usize] = active;

    // Header
    style.colors[imgui::StyleColor::Header as usize] = [0.15, 0.15, 0.15, 1.0];
    style.colors[imgui::StyleColor::HeaderHovered as usize] = dim;
    style.colors[imgui::StyleColor::HeaderActive as usize] = active;

    // Separator
    style.colors[imgui::StyleColor::Separator as usize] = [0.25, 0.25, 0.25, 1.0];
    style.colors[imgui::StyleColor::SeparatorHovered as usize] = dim;
    style.colors[imgui::StyleColor::SeparatorActive as usize] = active;

    // Tabs
    style.colors[imgui::StyleColor::Tab as usize] = [0.12, 0.12, 0.12, 1.0];
    style.colors[imgui::StyleColor::TabHovered as usize] = dim;
    style.colors[imgui::StyleColor::TabActive as usize] = active;
    style.colors[imgui::StyleColor::TabUnfocused as usize] = [0.12, 0.12, 0.12, 1.0];
    style.colors[imgui::StyleColor::TabUnfocusedActive as usize] = active;
}

fn draw_primary_window(ui: &Ui, state: &mut UiState, banner_texture: TextureId) {
    ui.window(APP_NAME)
        .size(WINDOW_SIZE, Condition::Always)
        .position([0.0, 0.0], Condition::Always)
        .flags(
            WindowFlags::NO_COLLAPSE
                | WindowFlags::NO_MOVE
                | WindowFlags::NO_RESIZE
                | WindowFlags::NO_SAVED_SETTINGS
                | WindowFlags::NO_TITLE_BAR,
        )
        .build(|| {
            Image::new(banner_texture, BANNER_SIZE).build(ui);
            draw_login(ui, state);
            if state.logged_in {
                poll_catalog(state);
                if let Some(catalog) = state.catalog.clone() {
                    match catalog.open_frame() {
                        Ok(surface) => {
                            draw_start_bar(ui, state);
                            let disabled = ui.begin_disabled(state.preparation.is_some());
                            draw_tabs(ui, state, &surface);
                            drop(disabled);
                        }
                        Err(error) => catalog_failure(state, &error),
                    }
                } else if state.logged_in {
                    imgui::ProgressBar::new(0.0).overlay_text("...").build(ui);
                    if ui.button("Exit") { crate::passport::end_usage_session(); std::process::exit(0); }
                }
            } else {
                state.catalog = None;
                state.catalog_result = None;
            }
        });
}

fn catalog_failure(state: &mut UiState, error: &str) {
    state.catalog = None;
    state.catalog_result = None;
    state.preparation = None;
    state.prepared_runtime = None;
    state.start_requested = false;
    state.ml.close_window();
    if let Some(shared) = &state.runner_shared {
        if let Ok(mut control) = shared.lock() { control.active = false; control.exit = true; }
    }
    if let Some(handle) = state.runner_thread.take() { let _ = handle.join(); }
    state.runner_shared = None;
    state.logged_in = false;
    state.auth_notify = error.to_owned();
    crate::passport::end_usage_session();
    show_error(error);
}

fn poll_catalog(state: &mut UiState) {
    let result = state.catalog_result.as_ref().and_then(|receiver| match receiver.try_recv() {
        Ok(result) => Some(result),
        Err(TryRecvError::Empty) => None,
        Err(TryRecvError::Disconnected) => Some(Err("Interface preparation stopped. Log in again and check verux_apb.log.".into())),
    });
    if let Some(result) = result {
        state.catalog_result = None;
        match result {
            Ok(catalog) => state.catalog = Some(catalog),
            Err(error) => catalog_failure(state, &error),
        }
    }
}

fn draw_login(ui: &Ui, state: &mut UiState) {
    if !state.logged_in {
        if ui.button("Login") {
            state.login_notify.clear();
            ui.open_popup("login_popup");
        }
        if ui.button("Exit") {
            crate::passport::end_usage_session();
            std::process::exit(0);
        }
        if !state.auth_notify.is_empty() {
            ui.text(&state.auth_notify);
        }
        ui.modal_popup_config("login_popup")
            .always_auto_resize(true)
            .build(|| {
                if let Some(receiver) = &state.login_result {
                    match receiver.try_recv() {
                        Ok(res) => {
                            state.login_result = None;
                            if res.valid {
                                match crate::passport::begin_usage_session(&state.license_key, &res) {
                                    Ok(()) => {
                                        if let Err(error) = crate::config::save_passport_key(&state.license_key) {
                                            crate::log_err!("Unable to save verified license key: {}", error);
                                            show_error("Login succeeded, but the key could not be saved. Check storage permissions.");
                                        }
                                        state.logged_in = true;
                                        let (sender, receiver) = mpsc::channel();
                                        state.catalog_result = Some(receiver);
                                        state.catalog = None;
                                        std::thread::spawn(move || { let _ = sender.send(crate::petal_text::receive()); });
                                        state.heartbeat_result = None;
                                        state.next_heartbeat_poll = Instant::now();
                                        state.login_notify.clear();
                                        state.auth_notify = crate::passport::format_user_info_for_ui(&res);
                                        ui.close_current_popup();
                                    }
                                    Err(error) => state.login_notify = format!("Session error: {}", error),
                                }
                            } else {
                                state.login_notify = res.message;
                            }
                        }
                        Err(TryRecvError::Empty) => (),
                        Err(TryRecvError::Disconnected) => {
                            state.login_result = None;
                            state.login_notify = "Login could not finish. Please try again.".into();
                            crate::log_err!("Login worker ended without returning a result.");
                        }
                    }
                }
                let disabled = ui.begin_disabled(state.login_result.is_some() || state.logged_in);
                ui.set_next_item_width(220.0);
                ui.input_text("License Key", &mut state.license_key).build();
                if !state.login_notify.is_empty() {
                    ui.text(&state.login_notify);
                }
                if ui.button("Submit") {
                    state.login_notify.clear();
                    let key = state.license_key.clone();
                    let (sender, receiver) = mpsc::channel();
                    state.login_result = Some(receiver);
                    std::thread::spawn(move || {
                        let result = crate::passport::verify_passport_key(&key);
                        let _ = sender.send(result);
                    });
                }
                drop(disabled);
            });
    }
}

fn poll_ui_heartbeat(state: &mut UiState) -> Option<crate::passport::VerifyResponse> {
    if let Some(receiver) = &state.heartbeat_result {
        match receiver.try_recv() {
            Ok(result) => {
                state.heartbeat_result = None;
                return result;
            }
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => {
                state.heartbeat_result = None;
                crate::log_err!("UI heartbeat worker ended without returning a result.");
                return Some(crate::passport::VerifyResponse {
                    valid: false,
                    message: "License verification could not finish. Please log in again.".into(),
                    status: "worker_error".into(),
                    http_status: 0,
                    user: None,
                    usage: None,
                    session_token: String::new(),
                    heartbeat_interval_seconds: 0,
                    session_ttl_seconds: 0,
                    session_hash: String::new(),
                    lease_deadline: None,
                });
            }
        }
    }
    if Instant::now() >= state.next_heartbeat_poll {
        state.next_heartbeat_poll = Instant::now() + Duration::from_secs(1);
        let (sender, receiver) = mpsc::channel();
        state.heartbeat_result = Some(receiver);
        std::thread::spawn(move || {
            let _ = sender.send(crate::passport::heartbeat_usage_session(false));
        });
    }
    None
}

fn draw_start_bar(ui: &Ui, state: &mut UiState) {
    if let Some(task) = &state.preparation {
        let result = match task.result.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Disconnected) => Some(Err("Preparation stopped. Retry Start and check verux_apb.log.".into())),
            Err(TryRecvError::Empty) => None,
        };
        if let Some(result) = result {
            state.preparation = None;
            match result {
                Ok(runtime) => {
                    state.prepared_runtime = runtime;
                    state.start_requested = true;
                }
                Err(error) => {
                    crate::log_err!("Start preparation failed: {error}");
                    show_error(&error);
                }
            }
        }
    }
    let label = format!("Verux.software [APB.RELOADED {APP_VERSION}]");
    TabBar::new("maku").build(ui, || {
        TabItem::new(&label).build(ui, || {
            let runner_busy = state.runner_thread.as_ref().is_some_and(|handle| !handle.is_finished());
            let disabled = ui.begin_disabled(state.ml.busy() || state.preparation.is_some() || state.start_requested || runner_busy);
            if ui.button("Start") {
                let (sender, result) = mpsc::channel();
                let progress = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
                let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                state.preparation = Some(Preparation { result, progress: progress.clone(), cancelled: cancelled.clone() });
                let preparation_config = state.config.effective_config();
                let needs_algorithm = preparation_config.unicorn;
                std::thread::spawn(move || {
                    let result = preparation_config.validate_eligibility().and_then(|_| {
                        if needs_algorithm {
                            crate::algorithm_runtime::AlgorithmRuntime::prepare_for_config(&preparation_config,&progress, &cancelled).map(Some)
                        } else {
                            crate::passport::algorithm_session().map(|_| None)
                        }
                    });
                    if !cancelled.load(std::sync::atomic::Ordering::Acquire) {
                        if result.is_ok() { progress.store(100, std::sync::atomic::Ordering::Release); }
                        let _ = sender.send(result);
                    }
                });
            }
            drop(disabled);
            if let Some(task) = &state.preparation {
                let percent = task.progress.load(std::sync::atomic::Ordering::Acquire).min(100);
                imgui::ProgressBar::new(percent as f32 / 100.0).overlay_text(format!("{percent}%")).build(ui);
                if ui.button("Cancel") { state.preparation = None; }
            }
            if state.start_requested { imgui::ProgressBar::new(1.0).overlay_text("100%").build(ui); }
            if let Some(notice) = state.runner_shared.as_ref().and_then(|shared| {
                shared.lock().ok().and_then(|control| {
                    control.thread_running.then(|| control.trigger_notice.clone()).flatten()
                })
            }) {
                ui.text_colored([1.0, 0.8, 0.2, 1.0], notice);
            }
            if ui.button("Exit") {
                state.preparation = None;
                state.prepared_runtime = None;
                state.ml.close_window();
                if let Some(ref shared) = state.runner_shared {
                    if let Ok(mut lock) = shared.lock() {
                        lock.exit = true;
                    }
                }
                if let Some(handle) = state.runner_thread.take() {
                    let _ = handle.join();
                }
                crate::passport::end_usage_session();
                std::process::exit(0);
            }
        });
    });
}

fn draw_tabs(ui: &Ui, state: &mut UiState, surface: &crate::petal_text::Frame) {
    let eligibility = state.config.eligibility();
    if eligibility.memory { poll_memory_status(state); }
    if let Some(receiver) = &state.echo_service_result {
        match receiver.try_recv() {
            Ok(result) => {
                state.echo_service_result = None;
                state.next_memory_status_check = Instant::now();
                match result {
                    Ok(_) => state.echo_service_status = "Ready".into(),
                    Err(error) => {
                        state.echo_service_status = error.clone();
                        show_error(&error);
                    }
                }
            }
            Err(TryRecvError::Disconnected) => {
                state.echo_service_result = None;
                state.echo_service_status = "Start failed; retry Start EchoDriver.".into();
                state.next_memory_status_check = Instant::now();
                show_error("EchoDriver service task ended without reporting a result. Retry Start EchoDriver.");
            }
            Err(TryRecvError::Empty) => {}
        }
    }
    TabBar::new(surface.text("u112")).build(ui, || {
        TabItem::new(surface.text("u082")).build(ui, || draw_unicorn(ui, state, surface));
        if eligibility.aim {
            TabItem::new(surface.text("u113")).build(ui, || draw_humanize(ui, state, surface));
        }
        TabItem::new(surface.text("u053")).build(ui, || draw_pegasus(ui, state, surface));
        if eligibility.color_aim || eligibility.color_trigger
        {
            TabItem::new(surface.text("u114")).build(ui, || draw_computer_vision(ui, state, surface));
        }
        if eligibility.memory
        {
            TabItem::new(surface.text("u115")).build(ui, || draw_memory(ui, state, surface));
        }
        TabItem::new(surface.text("u116")).build(ui, || draw_aurora(ui, state, surface));
        TabItem::new(surface.text("u117")).build(ui, || draw_general(ui, state, surface));
    });
    let risks: Vec<_> = crate::sb_protect::IDS.iter().map(|id| crate::sb_protect::risk(&state.config, id)).collect();
    if state.config.sb_protect && state.risk_enabled {
        if let Some(before)=&state.risk_snapshot {
            let changed: Vec<_>=crate::sb_protect::IDS.iter().zip(before.iter().zip(&risks)).filter_map(|(id,(a,b))| {
                (*a==crate::sb_protect::Risk::White && *b==crate::sb_protect::Risk::Red).then_some(*id)
            }).collect();
            if !changed.is_empty() {
                crate::log_info!("SB Protect: user configuration changed from white to red: {}.",changed.join(", "));
                ui.open_popup(surface.text("u147"));
            }
        }
    }
    state.risk_enabled = state.config.sb_protect;
    state.risk_snapshot = Some(risks);
    ui.modal_popup_config(surface.text("u147")).always_auto_resize(true).build(|| {
        ui.text(surface.text("u148"));
        if ui.button(surface.text("u149")) { crate::log_info!("SB Protect warning acknowledged."); ui.close_current_popup(); }
    });
}

fn binding_button(ui: &Ui, state: &mut UiState, target: BindingTarget, label: &str) {
    let name = match target {
        BindingTarget::Pause => &state.config.pause_key_name,
        BindingTarget::Block => &state.config.block_key_name,
        BindingTarget::Unicorn => &state.config.unicorn_key_name,
        BindingTarget::Pegasus => &state.config.pegasus_key_name,
        BindingTarget::Aurora => &state.config.aurora_key_name,
        BindingTarget::Lanterns => &state.config.lantern_key_name,
    };
    let value = if state.binding_target == Some(target) {
        "..."
    } else {
        name.as_str()
    };
    if ui.button(format!("{label}: {value}")) {
        state.binding_target = Some(target);
        state.awaiting_release = true;
    }
}

fn show_error(message: &str) {
    crate::log_err!("{message}");
    let text: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
            None,
            windows::core::PCWSTR(text.as_ptr()),
            windows::core::w!("Verux Error"),
            windows::Win32::UI::WindowsAndMessaging::MB_OK
                | windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
        );
    }
}

fn draw_computer_vision(ui: &Ui, state: &mut UiState, surface: &crate::petal_text::Frame) {
    input_vec2(ui, surface.text("u017"), &mut state.config.monitor_resolution);
    input_vec2(ui, surface.text("u110"), &mut state.config.grab_resolution);
    let mut fps = state.config.fps as i32;
    if ui.input_int(surface.text("u111"), &mut fps).build() {
        state.config.fps = fps.max(1) as u32;
    }
}

fn poll_memory_status(state: &mut UiState) {
    if let Some(receiver) = &state.memory_status_result {
        match receiver.try_recv() {
            Ok((apb, echo)) => {
                state.memory_status_result = None;
                state.apb_pid = apb.as_ref().ok().copied();
                let apb_status = match apb {
                    Ok(pid) => format!("Running (PID {pid})"),
                    Err(error) => error,
                };
                let echo_status = echo.unwrap_or_else(|error| error);
                if state.apb_process_status != apb_status {
                    crate::log_info!("APB status: {apb_status}");
                    state.apb_process_status = apb_status;
                }
                if state.echo_service_status != echo_status {
                    crate::log_info!("EchoDriver status: {echo_status}");
                    state.echo_service_status = echo_status;
                }
            }
            Err(TryRecvError::Disconnected) => {
                state.memory_status_result = None;
                state.apb_pid = None;
                state.apb_process_status = "Status check failed; refresh to retry.".into();
                state.echo_service_status = state.apb_process_status.clone();
                crate::log_err!("Memory status worker ended without reporting a result.");
            }
            Err(TryRecvError::Empty) => {}
        }
    }
    if state.memory_status_result.is_none()
        && state.echo_service_result.is_none()
        && Instant::now() >= state.next_memory_status_check
    {
        state.next_memory_status_check = Instant::now() + Duration::from_secs(3);
        let (sender, receiver) = mpsc::channel();
        state.memory_status_result = Some(receiver);
        std::thread::spawn(move || {
            let apb = crate::apb_process::find_running_apb_pid();
            let echo = crate::echo_courier::echodriver_status();
            let _ = sender.send((apb, echo));
        });
    }
}

fn draw_memory(ui: &Ui, state: &mut UiState, surface: &crate::petal_text::Frame) {
    let starting = state.echo_service_result.is_some();
    ui.disabled(starting, || {
        if ui.button(surface.text("u107")) {
            let (sender, receiver) = mpsc::channel();
            state.echo_service_result = Some(receiver);
            std::thread::spawn(move || {
                crate::log_info!("EchoDriver service start requested from Memory.");
                let result = crate::echo_courier::start_echodriver();
                match &result {
                    Ok(message) => {
                        crate::log_info!("{message}");
                    }
                    Err(error) => {
                        crate::log_err!("EchoDriver service start failed: {error}");
                    }
                }
                let _ = sender.send(result);
            });
        }
    });
    ui.same_line();
    if ui.button(surface.text("u108")) {
        state.next_memory_status_check = Instant::now();
    }
    ui.separator();
    ui.text_wrapped(format!("EchoDriver: {}", if starting { surface.text("u109") } else { &state.echo_service_status }));
    ui.text_wrapped(format!("APB: {}", state.apb_process_status));
}

fn draw_unicorn(ui: &Ui, state: &mut UiState, surface: &crate::petal_text::Frame) {
    labeled_separator(ui, surface.text("u081"));
    crate::sb_protect::draw(
        ui,
        state.config.sb_protect,
        crate::sb_protect::risk(&state.config, "u082"),
        || ui.checkbox(surface.text("u082"), &mut state.config.unicorn),
    );
    if !state.config.unicorn {
        return;
    }
    let memory = state.config.eligibility().memory;
    crate::sb_protect::draw(
        ui,
        state.config.sb_protect,
        crate::sb_protect::risk(&state.config, "u083"),
        || {
            radio_coordinate_source(
                ui,
                surface,
                surface.text("u083"),
                &mut state.config.coordinate_source,
                memory,
                state.config.sb_protect,
            )
        },
    );
    if !state.config.eligibility().aim {
        return;
    }
    if state.config.eligibility().memory_aim {
        ui.slider(
            surface.text("u189"),
            1.0,
            4000.0,
            &mut state.config.memory_aim_fov,
        );
    }
    crate::sb_protect::draw(
        ui,
        state.config.sb_protect,
        crate::sb_protect::risk(&state.config, "u084"),
        || binding_button(ui, state, BindingTarget::Unicorn, surface.text("u084")),
    );
    crate::sb_protect::draw(
        ui,
        state.config.sb_protect,
        crate::sb_protect::risk(&state.config, "u085"),
        || {
            radio_unicorn_activation(
                ui,
                surface,
                surface.text("u085"),
                &mut state.config.unicorn_base_activation,
                state.config.sb_protect,
                state.config.unicorn_key,
            )
        },
    );

    labeled_separator(ui, surface.text("u086"));
    if state.config.eligibility().memory_aim {
        crate::sb_protect::draw(
            ui,
            state.config.sb_protect,
            crate::sb_protect::risk(&state.config, "u087"),
            || ui.checkbox(surface.text("u087"), &mut state.config.prism_only),
        );
        let mut filter = match state.config.orbit_filter {
            LanternFilter::Opponents => 0,
            LanternFilter::MissionPlayers => 1,
            LanternFilter::AllPlayers => 2,
        };
        if crate::sb_protect::draw(
            ui,
            state.config.sb_protect,
            crate::sb_protect::risk(&state.config, "u088"),
            || {
                ui.combo_simple_string(
                    surface.text("u088"),
                    &mut filter,
                    &[
                        surface.text("u045"),
                        surface.text("u046"),
                        surface.text("u047"),
                    ],
                )
            },
        ) {
            state.config.orbit_filter = match filter {
                0 => LanternFilter::Opponents,
                1 => LanternFilter::MissionPlayers,
                _ => LanternFilter::AllPlayers,
            };
        }
        let mut priority = usize::from(state.config.orbit_order == OrbitOrder::ClosestDistance);
        if ui.combo_simple_string(
            surface.text("u090"),
            &mut priority,
            &[surface.text("u091"), surface.text("u092")],
        ) {
            state.config.orbit_order = if priority == 0 {
                OrbitOrder::ClosestUnicorn
            } else {
                OrbitOrder::ClosestDistance
            };
        }
    }
    {
        use crate::config::VerticalTracking;
        let choices = [
            VerticalTracking::Current,
            VerticalTracking::NoY,
            VerticalTracking::Head,
            VerticalTracking::Neck,
            VerticalTracking::Torso,
            VerticalTracking::Feet,
            VerticalTracking::ClosestPoint,
        ];
        let labels = [
            surface.text("u137"),
            surface.text("u138"),
            surface.text("u139"),
            surface.text("u140"),
            surface.text("u141"),
            surface.text("u142"),
            surface.text("u143"),
        ];
        let preview = choices
            .iter()
            .position(|mode| *mode == state.config.vertical_tracking)
            .unwrap_or(0);
        if let Some(_combo) = crate::sb_protect::draw(
            ui,
            state.config.sb_protect,
            crate::sb_protect::risk(&state.config, "u136"),
            || ui.begin_combo(surface.text("u136"), labels[preview]),
        ) {
            for (index, mode) in choices.iter().enumerate() {
                let _disabled =
                    ui.begin_disabled(index > 1 && !state.config.eligibility().memory_aim);
                let mut preview_config = state.config.clone();
                preview_config.vertical_tracking = *mode;
                if crate::sb_protect::draw(
                    ui,
                    state.config.sb_protect,
                    crate::sb_protect::risk(&preview_config, "u136"),
                    || {
                        ui.selectable_config(labels[index])
                            .selected(*mode == state.config.vertical_tracking)
                            .build()
                    },
                ) {
                    state.config.vertical_tracking = *mode;
                }
            }
        }
        if state.config.eligibility().memory_aim
            && state.config.vertical_tracking == VerticalTracking::ClosestPoint
        {
            let _id = ui.push_id("vertical_points");
            for (index, label) in labels[2..6].iter().enumerate() {
                crate::sb_protect::draw(
                    ui,
                    state.config.sb_protect,
                    crate::sb_protect::risk(&state.config, "u136"),
                    || ui.checkbox(*label, &mut state.config.vertical_points[index]),
                );
            }
        }
    }
    ui.checkbox(surface.text("u094"), &mut state.config.unicorn_on_unicorn);
    if state.config.unicorn_on_unicorn {
        ui.input_int(surface.text("u095"), &mut state.config.unicorn_distance)
            .build();
    }
}

fn draw_humanize(ui: &Ui, state: &mut UiState, surface: &crate::petal_text::Frame) {
    // Keep the tab row accessible when the additional controls require scrolling.
    ui.child_window(surface.text("u080"))
        .size([0.0, 0.0])
        .build(|| draw_humanize_controls(ui, state, surface));
}

fn draw_humanize_controls(ui: &Ui, state: &mut UiState, surface: &crate::petal_text::Frame) {
    labeled_separator(ui, surface.text("u062"));
    match state.ml.draw(ui, &mut state.config, surface) {
        Ok(Some(kind)) => {
            // Automated product quill must not contaminate a demonstration.
            if let Some(shared) = &state.runner_shared {
                if let Ok(mut control) = shared.lock() {
                    control.exit = true;
                }
            }
            if let Some(handle) = state.runner_thread.take() {
                if handle.join().is_err() {
                    show_error(surface.text("u063"));
                    return;
                }
            }
            state.runner_shared = None;
            if let Err(error) = state.ml.open_window(kind, &state.config) {
                show_error(&error);
            }
        }
        Ok(None) => {}
        Err(error) => show_error(&error),
    }
    labeled_separator(ui, surface.text("u064"));
    crate::sb_protect::draw(
        ui,
        state.config.sb_protect,
        crate::sb_protect::risk(&state.config, "u065"),
        || ui.checkbox(surface.text("u065"), &mut state.config.beesmas),
    );
    if state.config.beesmas {
        crate::sb_protect::draw(
            ui,
            state.config.sb_protect,
            crate::sb_protect::risk(&state.config, "u066"),
            || {
                radio_beesmas_type(
                    ui,
                    surface,
                    surface.text("u066"),
                    &mut state.config.beesmas_type,
                    state.config.sb_protect,
                )
            },
        );
        crate::sb_protect::draw(
            ui,
            state.config.sb_protect,
            crate::sb_protect::risk(&state.config, "u067"),
            || {
                ui.slider(
                    surface.text("u067"),
                    1.0,
                    10.0,
                    &mut state.config.beesmas_value,
                )
            },
        );
    }
    crate::sb_protect::draw(
        ui,
        state.config.sb_protect,
        crate::sb_protect::risk(&state.config, "u068"),
        || ui.checkbox(surface.text("u068"), &mut state.config.drift_guard),
    );
    let mut tide_enabled = state.config.tide_time_ms > 0;
    if crate::sb_protect::draw(
        ui,
        state.config.sb_protect,
        crate::sb_protect::risk(&state.config, "u069"),
        || ui.checkbox(surface.text("u069"), &mut tide_enabled),
    ) {
        if tide_enabled {
            state.config.tide_time_ms = 280;
        } else {
            state.config.tide_time_ms = 0;
        }
    }
    if tide_enabled {
        crate::sb_protect::draw(
            ui,
            state.config.sb_protect,
            crate::sb_protect::risk(&state.config, "u070"),
            || {
                ui.input_scalar(surface.text("u070"), &mut state.config.tide_time_ms)
                    .build()
            },
        );
        crate::sb_protect::draw(
            ui,
            state.config.sb_protect,
            crate::sb_protect::risk(&state.config, "u071"),
            || {
                ui.input_scalar(surface.text("u071"), &mut state.config.tide_idle_reset_ms)
                    .build()
            },
        );
        radio_ramp_apply(
            ui,
            surface,
            surface.text("u072"),
            &mut state.config.tide_apply,
        );
        radio_ease(
            ui,
            surface,
            surface.text("u073"),
            &mut state.config.tide_ease,
        );
    }

    labeled_separator(ui, surface.text("u074"));
    crate::sb_protect::draw(
        ui,
        state.config.sb_protect,
        crate::sb_protect::risk(&state.config, "u075"),
        || ui.checkbox(surface.text("u075"), &mut state.config.velvet_charge),
    );
    if state.config.velvet_charge {
        ui.input_int(surface.text("u076"), &mut state.config.velvet_distance)
            .build();
        crate::sb_protect::draw(
            ui,
            state.config.sb_protect,
            crate::sb_protect::risk(&state.config, "u077"),
            || {
                ui.input_scalar(surface.text("u077"), &mut state.config.velvet_build_ms)
                    .build()
            },
        );
        crate::sb_protect::draw(
            ui,
            state.config.sb_protect,
            crate::sb_protect::risk(&state.config, "u078"),
            || {
                radio_prox_effect(
                    ui,
                    surface,
                    surface.text("u078"),
                    &mut state.config.velvet_effect,
                )
            },
        );
        radio_ease(
            ui,
            surface,
            surface.text("u079"),
            &mut state.config.velvet_ease,
        );
    }
}

fn draw_pegasus(ui: &Ui, state: &mut UiState, surface: &crate::petal_text::Frame) {
    crate::sb_protect::draw(
        ui,
        state.config.sb_protect,
        crate::sb_protect::risk(&state.config, "u053"),
        || ui.checkbox(surface.text("u053"), &mut state.config.pegasus),
    );
    if !state.config.pegasus {
        return;
    }
    let memory = state.config.eligibility().memory;
    crate::sb_protect::draw(
        ui,
        state.config.sb_protect,
        crate::sb_protect::risk(&state.config, "u054"),
        || {
            radio_coordinate_source(
                ui,
                surface,
                surface.text("u054"),
                &mut state.config.pegasus_source,
                memory,
                state.config.sb_protect,
            )
        },
    );
    if !state.config.eligibility().trigger {
        return;
    }
    crate::sb_protect::draw(
        ui,
        state.config.sb_protect,
        crate::sb_protect::risk(&state.config, "u055"),
        || binding_button(ui, state, BindingTarget::Pegasus, surface.text("u055")),
    );
    if state.config.eligibility().memory_trigger {
        if ui.checkbox(surface.text("u056"), &mut state.config.pegasus_on_red)
            && !state.config.pegasus_on_red
            && !state.config.pegasus_on_green
        {
            state.config.pegasus_on_green = true;
        }

        if ui.checkbox(surface.text("u057"), &mut state.config.pegasus_on_green)
            && !state.config.pegasus_on_red
            && !state.config.pegasus_on_green
        {
            state.config.pegasus_on_red = true;
        }
    }
    crate::sb_protect::draw(
        ui,
        state.config.sb_protect,
        crate::sb_protect::risk(&state.config, "u058"),
        || {
            radio_pegasus_activation(
                ui,
                surface,
                surface.text("u058"),
                &mut state.config.pegasus_activation,
                state.config.sb_protect,
                state.config.pegasus_key,
            )
        },
    );
    if state.config.eligibility().aim && state.config.pegasus_unicorn_supported() {
        ui.checkbox(
            surface.text("u059"),
            &mut state.config.pegasus_activates_unicorn,
        );
    }
    TabBar::new(surface.text("u060")).build(ui, || {
        TabItem::new(surface.text("u061")).build(ui, || {
            crate::sb_protect::draw(
                ui,
                state.config.sb_protect,
                crate::sb_protect::risk(&state.config, "u061"),
                || {
                    ui.slider(
                        surface.text("u061"),
                        1,
                        1000,
                        &mut state.config.pegasus_delay_ms,
                    )
                },
            );
        });
    });
}

fn draw_aurora(ui: &Ui, state: &mut UiState, surface: &crate::petal_text::Frame) {
    ui.child_window(surface.text("u052"))
        .size([0.0, 0.0])
        .build(|| draw_aurora_controls(ui, state, surface));
}

fn draw_aurora_controls(ui: &Ui, state: &mut UiState, surface: &crate::petal_text::Frame) {
    use crate::visual_style::*;
    if let Some(error) = &state.config.visual_style_error {
        ui.text_wrapped(error);
        return;
    }
    labeled_separator(ui, surface.text("u152"));
    let e = state.config.eligibility();
    labeled_separator(ui, surface.text("u150"));
    {
        crate::sb_protect::draw(
            ui,
            state.config.sb_protect,
            crate::sb_protect::risk(&state.config, "u032"),
            || ui.checkbox(surface.text("u032"), &mut state.config.aurora_enabled),
        );
        if state.config.aurora_enabled {
            binding_button(ui, state, BindingTarget::Aurora, surface.text("u033"));
            crate::sb_protect::draw(
                ui,
                state.config.sb_protect,
                crate::sb_protect::risk(&state.config, "u034"),
                || ui.checkbox(surface.text("u034"), &mut state.config.halo),
            );
            let _disabled = ui.begin_disabled(!e.aim);
            crate::sb_protect::draw(
                ui,
                state.config.sb_protect,
                crate::sb_protect::risk(&state.config, "u036"),
                || ui.checkbox(surface.text("u036"), &mut state.config.unicorn_target_box),
            );
            crate::sb_protect::draw(
                ui,
                state.config.sb_protect,
                crate::sb_protect::risk(&state.config, "u038"),
                || ui.checkbox(surface.text("u038"), &mut state.config.lines),
            );
        }
    }
    if e.memory {
        labeled_separator(ui, surface.text("u151"));
        crate::sb_protect::draw(
            ui,
            state.config.sb_protect,
            crate::sb_protect::risk(&state.config, "u096"),
            || ui.checkbox(surface.text("u096"), &mut state.config.lanterns),
        );
        if state.config.lanterns {
            binding_button(ui, state, BindingTarget::Lanterns, surface.text("u097"));
        }
        crate::sb_protect::draw(
            ui,
            state.config.sb_protect,
            crate::sb_protect::risk(&state.config, "u133"),
            || ui.checkbox(surface.text("u133"), &mut state.config.jade),
        );
        crate::sb_protect::draw(
            ui,
            state.config.sb_protect,
            crate::sb_protect::risk(&state.config, "u134"),
            || ui.checkbox(surface.text("u134"), &mut state.config.wisps),
        );
        labeled_separator(ui, surface.text("u043"));
        crate::sb_protect::draw(
            ui,
            state.config.sb_protect,
            crate::sb_protect::risk(&state.config, "u043"),
            || ui.checkbox(surface.text("u043"), &mut state.config.comet),
        );
        if state.config.comet {
            let _id = ui.push_id("comet_jade");
            crate::sb_protect::draw(
                ui,
                state.config.sb_protect,
                crate::sb_protect::risk(&state.config, "v_radar_objects"),
                || {
                    ui.checkbox(surface.text("u133"), &mut state.config.comet_jade);
                },
            );
        }
    }
    let enabled = [
        state.config.aurora_enabled && state.config.halo,
        e.aim && state.config.aurora_enabled && state.config.unicorn_target_box,
        e.aim && state.config.aurora_enabled && state.config.lines,
        e.memory && state.config.lanterns,
        e.memory && state.config.jade,
        e.memory && state.config.wisps,
        e.memory && state.config.comet,
    ];
    let names = ["u034", "u036", "u038", "u096", "u133", "u134", "u043"];
    let Some(first) = enabled.iter().position(|value| *value) else {
        return;
    };
    if !enabled[state.visual_selection] {
        state.visual_selection = first;
    }
    labeled_separator(ui, surface.text("u153"));
    let _width = ui.push_item_width(210.0);
    if let Some(_combo) = ui.begin_combo(
        surface.text("u154"),
        surface.text(names[state.visual_selection]),
    ) {
        for (i, active) in enabled.iter().enumerate() {
            if *active
                && ui
                    .selectable_config(surface.text(names[i]))
                    .selected(state.visual_selection == i)
                    .build()
            {
                state.visual_selection = i;
            }
        }
    }
    let selected = state.visual_selection;
    if let Some(_combo) = ui.begin_combo(surface.text("u188"), "") {
        for (i, active) in enabled.iter().enumerate() {
            if *active && i != selected && ui.selectable(surface.text(names[i])) {
                let source = state.config.visual_styles[i];
                let target = &mut state.config.visual_styles[selected];
                // Copy appearance only; preserve geometry and source-specific settings.
                target.color = source.color;
                target.mode = source.mode;
                target.opacity = source.opacity;
                target.width = source.width;
                target.outline = source.outline;
                target.outline_width = source.outline_width;
                target.outline_color = source.outline_color;
                target.fill = source.fill;
                target.fill_opacity = source.fill_opacity;
                target.speed = source.speed;
                target.saturation = source.saturation;
                target.brightness = source.brightness;
                target.palette = source.palette;
                target.minimum_opacity = source.minimum_opacity;
                target.role_colors = source.role_colors
                    && matches!(selected, PLAYERS | PUMPKINS | PROJECTILES | RADAR);
            }
        }
    }
    let style = &mut state.config.visual_styles[selected];
    ui.combo_simple_string(
        surface.text("u155"),
        &mut style.mode,
        &[
            surface.text("u156"),
            surface.text("u157"),
            surface.text("u158"),
        ],
    );
    if matches!(selected, PLAYERS | PUMPKINS | PROJECTILES | RADAR) && style.mode != 1 {
        let mut palette = usize::from(style.role_colors);
        if ui.combo_simple_string(
            surface.text("u181"),
            &mut palette,
            &[
                surface.text("u182"),
                surface.text(if matches!(selected, PUMPKINS | PROJECTILES) {
                    "u191"
                } else {
                    "u183"
                }),
            ],
        ) {
            style.role_colors = palette == 1;
        }
    }
    if style.mode != 1 && !style.role_colors {
        ui.color_edit3(surface.text("u159"), &mut style.color);
    }
    ui.slider(surface.text("u160"), 0.0, 1.0, &mut style.opacity);
    style.minimum_opacity = style.minimum_opacity.min(style.opacity);
    if style.mode > 0 {
        ui.slider(surface.text("u170"), 0.05, 5.0, &mut style.speed);
    }
    if style.mode == 1 {
        ui.slider(surface.text("u171"), 0.0, 1.0, &mut style.saturation);
        ui.slider(surface.text("u172"), 0.0, 1.0, &mut style.brightness);
    }
    if style.mode == 2 {
        ui.slider(
            surface.text("u173"),
            0.0,
            style.opacity,
            &mut style.minimum_opacity,
        );
    }
    ui.slider(surface.text("u161"), 1, 8, &mut style.width);
    ui.combo_simple_string(
        surface.text("u162"),
        &mut style.outline,
        &[
            surface.text("u163"),
            surface.text("u164"),
            surface.text("u165"),
        ],
    );
    if style.outline > 0 {
        ui.slider(surface.text("u166"), 1, 5, &mut style.outline_width);
        if style.outline == 2 {
            ui.color_edit3(surface.text("u167"), &mut style.outline_color);
        }
    }
    if selected != LINES {
        ui.checkbox(surface.text("u168"), &mut style.fill);
        if style.fill {
            ui.slider(surface.text("u169"), 0.0, 1.0, &mut style.fill_opacity);
        }
    }
    if matches!(selected, TARGET | PLAYERS) {
        ui.combo_simple_string(
            surface.text("u174"),
            &mut style.shape,
            &[
                surface.text("u175"),
                surface.text("u176"),
                surface.text("u177"),
            ],
        );
        if style.shape == 2 {
            ui.slider(surface.text("u178"), 0.05, 0.5, &mut style.corner_length);
        }
    }
    if matches!(selected, PLAYERS | PUMPKINS | PROJECTILES | RADAR) {
        if selected != PLAYERS || style.shape == 1 {
            ui.slider(surface.text("u179"), 1, 30, &mut style.marker_size);
        }
        if selected != RADAR {
            ui.slider(surface.text("u180"), 0.0, 2000.0, &mut style.max_distance);
        }
    }
    // Keep legacy renderer metadata synchronized with the selected appearance.
    if selected == PLAYERS {
        state.config.aurora_style = if style.shape == 1 {
            AuroraStyle::Dots
        } else {
            AuroraStyle::Boxes
        };
    }
    if selected == LINES {
        let mut origin = usize::from(state.config.unicorn_line_origin == UnicornLineOrigin::Top);
        if ui.combo_simple_string(
            surface.text("u039"),
            &mut origin,
            &[surface.text("u040"), surface.text("u041")],
        ) {
            state.config.unicorn_line_origin = if origin == 0 {
                UnicornLineOrigin::Center
            } else {
                UnicornLineOrigin::Top
            };
        }
        ui.combo_simple_string(
            surface.text("u184"),
            &mut style.endpoint,
            &[
                surface.text("u185"),
                surface.text("u186"),
                surface.text("u187"),
            ],
        );
    }
    if matches!(selected, PLAYERS | RADAR) {
        let filter = if selected == PLAYERS {
            &mut state.config.lantern_filter
        } else {
            &mut state.config.comet_filter
        };
        let mut value = match *filter {
            LanternFilter::Opponents => 0,
            LanternFilter::MissionPlayers => 1,
            LanternFilter::AllPlayers => 2,
        };
        if ui.combo_simple_string(
            surface.text("u044"),
            &mut value,
            &[
                surface.text("u045"),
                surface.text("u046"),
                surface.text("u047"),
            ],
        ) {
            *filter = [
                LanternFilter::Opponents,
                LanternFilter::MissionPlayers,
                LanternFilter::AllPlayers,
            ][value];
        }
        if style.role_colors && style.mode != 1 {
            for (index, id) in ["u103", "u104", "u105", "u106"].iter().enumerate() {
                ui.color_edit3(surface.text(id), &mut style.palette[index]);
            }
        }
    }
    if matches!(selected, TARGET | PLAYERS | PUMPKINS | PROJECTILES) {
        ui.checkbox(surface.text("u101"), &mut style.show_distance);
    }
    if selected == RADAR {
        ui.combo_simple_string(
            surface.text("u048"),
            &mut state.config.comet_placement,
            &crate::comet::PLACEMENTS,
        );
        ui.slider(
            surface.text("u049"),
            crate::comet::MIN_SCALE,
            crate::comet::MAX_SCALE,
            &mut state.config.comet_scale,
        );
        ui.slider(
            surface.text("u190"),
            10.0,
            1000.0,
            &mut state.config.radar_range,
        );
        if state.config.comet_placement == crate::comet::CUSTOM_PLACEMENT {
            ui.slider(
                surface.text("u050"),
                0.0,
                100.0,
                &mut state.config.comet_x_percent,
            );
            ui.slider(
                surface.text("u051"),
                0.0,
                100.0,
                &mut state.config.comet_y_percent,
            );
        }
    }
}

fn draw_general(ui: &Ui, state: &mut UiState, surface: &crate::petal_text::Frame) {
    ui.checkbox(surface.text("u146"), &mut state.config.sb_protect);
    {
        let _id = ui.push_id("memory_features");
        let _disabled = ui.begin_disabled(!crate::edition::memory_available());
        let mut enabled = state.config.memory_features && crate::edition::memory_available();
        if crate::sb_protect::draw(ui, state.config.sb_protect, crate::sb_protect::risk(&state.config, "u115"), || ui.checkbox(surface.text("u115"), &mut enabled)) { state.config.memory_features = enabled; }
    }
    #[cfg(feature = "memory")]
    if state.config.eligibility().memory {
        ui.checkbox(surface.text("u144"), &mut state.config.hit_sound);
        if state.config.eligibility().hitsound {
            if let Some(_combo) = ui.begin_combo(surface.text("u145"), &state.config.hit_sound_name) {
                for (name, _) in crate::hit_sound::SOUNDS {
                    if ui.selectable_config(*name).selected(state.config.hit_sound_name == *name).build() {
                        state.config.hit_sound_name = (*name).into();
                    }
                }
            }
            ui.same_line();
            if ui.button(surface.text("u128")) {
                if let Err(error) = crate::hit_sound::sound(&state.config.hit_sound_name)
                    .and_then(crate::hit_sound::play)
                {
                    crate::log_err!("Hit sound preview failed: {error}");
                    show_error(&error);
                }
            }
        }
    }
    binding_button(ui, state, BindingTarget::Pause, surface.text("u018"));
    binding_button(ui, state, BindingTarget::Block, surface.text("u019"));
    labeled_separator(ui, surface.text("u020"));
    let ports: Vec<&str> = state.com_ports.iter().map(String::as_str).collect();
    if ui.combo_simple_string(surface.text("u021"), &mut state.selected_com_port, &ports) {
        if let Some(port) = state.com_ports.get(state.selected_com_port) {
            state.config.com_port = port.clone();
        }
    }
    if ui.button(surface.text("u022")) {
        match serialport::available_ports() {
            Ok(ports) => {
                state.com_ports = ports.into_iter().map(|port| port.port_name).collect();
                state.selected_com_port = state
                    .com_ports
                    .iter()
                    .position(|port| port.eq_ignore_ascii_case(&state.config.com_port))
                    .unwrap_or(0);
                if let Some(port) = state.com_ports.get(state.selected_com_port) {
                    state.config.com_port = port.clone();
                }
                if state.com_ports.is_empty() {
                    show_error("No COM ports detected. Connect the Arduino and refresh COM ports.");
                }
            }
            Err(error) => show_error(&format!("Refresh COM ports failed: {error}")),
        }
    }
    let rates = [surface.text("u023"), surface.text("u024"), surface.text("u025"), surface.text("u026")];
    let mut selected = match state.config.mouse_polling_rate {
        125 => 0,
        250 => 1,
        1000 => 3,
        _ => 2,
    };
    if ui.combo_simple_string(surface.text("u027"), &mut selected, &rates) {
        state.config.mouse_polling_rate = [125, 250, 500, 1000][selected];
    }
    labeled_separator(ui, surface.text("u028"));
    if ui.button(surface.text("u029")) {
        state.config.lines_color = float_to_rgb(state.lines_color);
        state.config.halo_color = float_to_rgb(state.halo_color);
        state.config.ui_color = float_to_rgb(state.ui_color);
        match crate::config::save_profile(state.current_save_entry_name.trim(), &state.config) {
            Ok(()) => {
                match crate::config::get_profile_names() {
                    Ok(profiles) => state.profiles = profiles,
                    Err(error) => show_error(&format!("Read profiles failed: {error}")),
                }
            }
            Err(error) => show_error(&format!("Save config failed: {error}")),
        }
    }
    ui.color_edit3(surface.text("u030"), &mut state.ui_color);
    ui.input_text("APB Target Folder", &mut state.config.apb_path)
        .build();
    if ui.button("Apply verux.config") {
        match crate::config::deploy_verux_config(state.config.apb_path.trim()) {
            Ok(()) => {
                crate::log_info!("Verux config package updated.");
            }
            Err(error) => show_error(&format!("Apply verux.config failed: {error}")),
        }
    }
    labeled_separator(ui, surface.text("u031"));
    if !state.auth_notify.is_empty() {
        ui.text(&state.auth_notify);
    }
}

fn input_vec2(ui: &Ui, label: &str, value: &mut Vec2i) {
    let mut data = [value.x, value.y];
    if ui.input_int2(label, &mut data).build() {
        value.x = data[0];
        value.y = data[1];
    }
}

fn radio_unicorn_activation(
    ui: &Ui,
    surface: &crate::petal_text::Frame,
    label: &str,
    value: &mut UnicornActivation,
    enabled: bool,
    key: u32,
) {
    ui.text(label);
    crate::sb_protect::draw(
        ui,
        enabled,
        if key == 1 || key == 2 || true {
            crate::sb_protect::Risk::White
        } else {
            crate::sb_protect::Risk::Yellow
        },
        || ui.radio_button(surface.text("u013"), value, UnicornActivation::Toggle),
    );
    ui.same_line();
    crate::sb_protect::draw(
        ui,
        enabled,
        if key == 1 || key == 2 || false {
            crate::sb_protect::Risk::White
        } else {
            crate::sb_protect::Risk::Yellow
        },
        || ui.radio_button(surface.text("u014"), value, UnicornActivation::Hold),
    );
    ui.same_line();
    crate::sb_protect::draw(
        ui,
        enabled,
        if key == 1 || key == 2 || false {
            crate::sb_protect::Risk::White
        } else {
            crate::sb_protect::Risk::Yellow
        },
        || ui.radio_button(surface.text("u016"), value, UnicornActivation::Timed),
    );
}

fn radio_pegasus_activation(
    ui: &Ui,
    surface: &crate::petal_text::Frame,
    label: &str,
    value: &mut PegasusActivation,
    enabled: bool,
    key: u32,
) {
    ui.text(label);
    crate::sb_protect::draw(
        ui,
        enabled,
        if false {
            crate::sb_protect::Risk::White
        } else {
            crate::sb_protect::Risk::Yellow
        },
        || ui.radio_button(surface.text("u013"), value, PegasusActivation::Toggle),
    );
    ui.same_line();
    crate::sb_protect::draw(
        ui,
        enabled,
        if key == 2 {
            crate::sb_protect::Risk::White
        } else {
            crate::sb_protect::Risk::Yellow
        },
        || ui.radio_button(surface.text("u014"), value, PegasusActivation::Hold),
    );
    ui.same_line();
    crate::sb_protect::draw(
        ui,
        enabled,
        if true {
            crate::sb_protect::Risk::White
        } else {
            crate::sb_protect::Risk::Yellow
        },
        || ui.radio_button(surface.text("u015"), value, PegasusActivation::DoubleCrouch),
    );
}

fn radio_coordinate_source(
    ui: &Ui,
    surface: &crate::petal_text::Frame,
    label: &str,
    value: &mut CoordinateSource,
    memory: bool,
    enabled: bool,
) {
    ui.text(label);
    crate::sb_protect::draw(ui, enabled, crate::sb_protect::Risk::White, || {
        ui.radio_button(
            format!("{}##{label}", surface.text("u114")),
            value,
            CoordinateSource::ColorDetection,
        )
    });
    ui.same_line();
    let _disabled = ui.begin_disabled(!memory);
    crate::sb_protect::draw(ui, enabled, crate::sb_protect::Risk::Red, || {
        ui.radio_button(
            format!("{}##{label}", surface.text("u115")),
            value,
            CoordinateSource::Memory,
        )
    });
}

fn radio_beesmas_type(
    ui: &Ui,
    surface: &crate::petal_text::Frame,
    label: &str,
    value: &mut BeesmasType,
    enabled: bool,
) {
    ui.text(label);
    crate::sb_protect::draw(ui, enabled, crate::sb_protect::Risk::Red, || {
        ui.radio_button(surface.text("u009"), value, BeesmasType::Linear)
    });
    ui.same_line();
    crate::sb_protect::draw(ui, enabled, crate::sb_protect::Risk::Red, || {
        ui.radio_button(surface.text("u010"), value, BeesmasType::Logarithmic)
    });
    ui.same_line();
    crate::sb_protect::draw(ui, enabled, crate::sb_protect::Risk::Yellow, || {
        ui.radio_button(surface.text("u011"), value, BeesmasType::Hyperbolic)
    });
    ui.same_line();
    crate::sb_protect::draw(ui, enabled, crate::sb_protect::Risk::White, || {
        ui.radio_button(surface.text("u012"), value, BeesmasType::Bezier)
    });
}

fn radio_ramp_apply(ui: &Ui, surface: &crate::petal_text::Frame, label: &str, value: &mut TideApply) {
    ui.text(label);
    ui.radio_button(surface.text("u008"), value, TideApply::Delta);
    ui.same_line();
    ui.radio_button(surface.text("u002"), value, TideApply::BeesmasValue);
}

fn radio_ease(ui: &Ui, surface: &crate::petal_text::Frame, label: &str, value: &mut ArcCurve) {
    ui.text(label);
    ui.radio_button(surface.text("u003"), value, ArcCurve::Linear);
    ui.same_line();
    ui.radio_button(surface.text("u004"), value, ArcCurve::Beesmasstep);
    ui.same_line();
    ui.radio_button(surface.text("u005"), value, ArcCurve::Beesmascurve);
    ui.same_line();
    ui.radio_button(surface.text("u006"), value, ArcCurve::Quad);
    ui.same_line();
    ui.radio_button(surface.text("u007"), value, ArcCurve::Cubic);
}

fn radio_prox_effect(ui: &Ui, surface: &crate::petal_text::Frame, label: &str, value: &mut VelvetEffect) {
    ui.text(label);
    ui.radio_button(surface.text("u001"), value, VelvetEffect::Strength);
    ui.same_line();
    ui.radio_button(surface.text("u002"), value, VelvetEffect::BeesmasValue);
}

fn float_to_rgb(col: [f32; 3]) -> Rgb {
    Rgb::new(
        (col[0] * 255.0).round() as u8,
        (col[1] * 255.0).round() as u8,
        (col[2] * 255.0).round() as u8,
    )
}

fn rgb_to_float(rgb: Rgb) -> [f32; 3] {
    [
        rgb.r as f32 / 255.0,
        rgb.g as f32 / 255.0,
        rgb.b as f32 / 255.0,
    ]
}

fn labeled_separator(ui: &Ui, label: &str) {
    ui.separator();
    ui.text(label);
}

fn get_vk_name(vk: u32) -> String {
    match vk {
        1 => "VK_LBUTTON".to_string(),
        2 => "VK_RBUTTON".to_string(),
        3 => "VK_CANCEL".to_string(),
        4 => "VK_MBUTTON".to_string(),
        5 => "VK_XBUTTON1".to_string(),
        6 => "VK_XBUTTON2".to_string(),
        8 => "VK_BACK".to_string(),
        9 => "VK_TAB".to_string(),
        12 => "VK_CLEAR".to_string(),
        13 => "VK_RETURN".to_string(),
        16 => "VK_SHIFT".to_string(),
        17 => "VK_CONTROL".to_string(),
        18 => "VK_MENU".to_string(),
        19 => "VK_PAUSE".to_string(),
        20 => "VK_CAPITAL".to_string(),
        21 => "VK_KANA".to_string(),
        23 => "VK_JUNJA".to_string(),
        24 => "VK_FINAL".to_string(),
        25 => "VK_HANJA".to_string(),
        27 => "VK_ESCAPE".to_string(),
        28 => "VK_CONVERT".to_string(),
        29 => "VK_NONCONVERT".to_string(),
        30 => "VK_ACCEPT".to_string(),
        31 => "VK_MODECHANGE".to_string(),
        32 => "VK_SPACE".to_string(),
        33 => "VK_PRIOR".to_string(),
        34 => "VK_NEXT".to_string(),
        35 => "VK_END".to_string(),
        36 => "VK_HOME".to_string(),
        37 => "VK_LEFT".to_string(),
        38 => "VK_UP".to_string(),
        39 => "VK_RIGHT".to_string(),
        40 => "VK_DOWN".to_string(),
        41 => "VK_SELECT".to_string(),
        42 => "VK_PRINT".to_string(),
        43 => "VK_EXECUTE".to_string(),
        44 => "VK_SNAPSHOT".to_string(),
        45 => "VK_INSERT".to_string(),
        46 => "VK_DELETE".to_string(),
        47 => "VK_HELP".to_string(),
        48..=57 => (vk as u8 as char).to_string(),
        65..=90 => (vk as u8 as char).to_string(),
        91 => "VK_LWIN".to_string(),
        92 => "VK_RWIN".to_string(),
        93 => "VK_APPS".to_string(),
        95 => "VK_SLEEP".to_string(),
        96..=105 => format!("VK_NUMPAD{}", vk - 96),
        106 => "VK_MULTIPLY".to_string(),
        107 => "VK_ADD".to_string(),
        108 => "VK_SEPARATOR".to_string(),
        109 => "VK_SUBTRACT".to_string(),
        110 => "VK_DECIMAL".to_string(),
        111 => "VK_DIVIDE".to_string(),
        112..=135 => format!("VK_F{}", vk - 111),
        144 => "VK_NUMLOCK".to_string(),
        160 => "VK_LSHIFT".to_string(),
        161 => "VK_RSHIFT".to_string(),
        162 => "VK_LCONTROL".to_string(),
        163 => "VK_RCONTROL".to_string(),
        164 => "VK_LMENU".to_string(),
        165 => "VK_RMENU".to_string(),
        166 => "VK_BROWSER_BACK".to_string(),
        167 => "VK_BROWSER_FORWARD".to_string(),
        168 => "VK_BROWSER_REFRESH".to_string(),
        169 => "VK_BROWSER_STOP".to_string(),
        170 => "VK_BROWSER_SEARCH".to_string(),
        171 => "VK_BROWSER_FAVORITES".to_string(),
        172 => "VK_BROWSER_HOME".to_string(),
        _ => format!("VK_{}", vk),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_vk_name() {
        assert_eq!(get_vk_name(1), "VK_LBUTTON");
        assert_eq!(get_vk_name(2), "VK_RBUTTON");
        assert_eq!(get_vk_name(32), "VK_SPACE");
        assert_eq!(get_vk_name(48), "0");
        assert_eq!(get_vk_name(57), "9");
        assert_eq!(get_vk_name(65), "A");
        assert_eq!(get_vk_name(90), "Z");
        assert_eq!(get_vk_name(112), "VK_F1");
        assert_eq!(get_vk_name(123), "VK_F12");
        assert_eq!(get_vk_name(999), "VK_999");
    }
}
