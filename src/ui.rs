use crate::config::{
    AimActivation, CoordinateSource, EaseCurve, ProfileConfig, ProximityEffect, RampApply, Rgb,
    SmoothType, TriggerActivation, Vec2i,
};
use crate::input::InputBackend;
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
use std::time::Instant;

pub const APP_VERSION: &str = "2.1";
pub const APP_NAME: &str = "Verux.APB";
pub const WINDOW_SIZE: [f32; 2] = [500.0, 500.0];
pub const BANNER_SIZE: [f32; 2] = [480.0, 50.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingTarget {
    Pause,
    Block,
    Aimbot,
    Triggerbot,
    Visuals,
}

#[derive(Debug)]
pub struct UiState {
    pub config: ProfileConfig,
    pub logged_in: bool,
    pub license_key: String,
    pub login_notify: String,
    pub auth_notify: String,
    pub status: String,
    pub current_save_entry_name: String,
    pub selected_com_port: usize,
    pub com_ports: Vec<String>,
    pub apb_pid: Option<u32>,
    pub apb_process_status: String,
    pub input_backend: InputBackend,
    pub trigger_bounds: [i32; 2],
    pub color_picker_tolerance: i32,
    pub show_targeting_tab: bool,
    pub show_visuals_tab: bool,
    pub lines_color: [f32; 3],
    pub fov_color: [f32; 3],
    pub ui_color: [f32; 3],
    pub profiles: Vec<String>,
    pub selected_profile: usize,
    pub start_requested: bool,
    pub binding_target: Option<BindingTarget>,
    pub awaiting_release: bool,
    pub runner_shared: Option<std::sync::Arc<std::sync::Mutex<crate::runner::RunnerControl>>>,
    pub runner_thread: Option<std::thread::JoinHandle<()>>,
}

impl UiState {
    pub fn new(com_ports: Vec<String>) -> Self {
        let profiles = crate::config::get_profile_names();
        let selected_profile = profiles.iter().position(|p| p == "default").unwrap_or(0);
        let config = crate::config::load_profile(&profiles[selected_profile]).unwrap_or_default();
        let license_key = crate::config::load_license_key();
        let selected_com_port = com_ports
            .iter()
            .position(|port| port.eq_ignore_ascii_case(&config.com_port))
            .or_else(|| {
                com_ports
                    .iter()
                    .position(|port| port.eq_ignore_ascii_case("COM1"))
            })
            .unwrap_or(0);
        Self {
            lines_color: rgb_to_float(config.lines_color),
            fov_color: rgb_to_float(config.fov_color),
            ui_color: rgb_to_float(config.ui_color),
            config,
            logged_in: false,
            license_key,
            login_notify: String::new(),
            auth_notify: String::new(),
            status: String::new(),
            current_save_entry_name: profiles[selected_profile].clone(),
            selected_com_port,
            com_ports,
            apb_pid: None,
            apb_process_status: String::new(),
            input_backend: InputBackend::Arduino,
            trigger_bounds: [10, 10],
            color_picker_tolerance: 0,
            show_targeting_tab: false,
            show_visuals_tab: false,
            profiles,
            selected_profile,
            start_requested: false,
            binding_target: None,
            awaiting_release: false,
            runner_shared: None,
            runner_thread: None,
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self::new(
            serialport::available_ports()
                .map(|ports| ports.into_iter().map(|port| port.port_name).collect())
                .unwrap_or_default(),
        )
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
    let mut state = UiState::default();

    apply_theme(&mut imgui, state.config.ui_color);

    let proxy = event_loop.create_proxy();

    #[allow(deprecated)]
    event_loop.run(move |event, window_target| match event {
        glium::winit::event::Event::UserEvent(()) => {
            window.set_visible(true);
            let license_error = state.runner_shared.as_ref().and_then(|shared| {
                shared
                    .lock()
                    .ok()
                    .and_then(|mut control| control.license_error.take())
            });
            if let Some(message) = license_error {
                state.logged_in = false;
                state.start_requested = false;
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
                state.auth_notify = message.clone();
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
                if let Some(res) = crate::license::heartbeat_usage_session(false) {
                    if !res.valid {
                        state.logged_in = false;
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
                                    BindingTarget::Aimbot => {
                                        state.config.aim_key = vk_code;
                                        state.config.aim_key_name = name;
                                    }
                                    BindingTarget::Triggerbot => {
                                        state.config.trigger_key = vk_code;
                                        state.config.trigger_key_name = name;
                                    }
                                    BindingTarget::Visuals => {
                                        state.config.visuals_key = vk_code;
                                        state.config.visuals_key_name = name;
                                    }
                                }
                                let _ = crate::config::save_profile(
                                    &state.current_save_entry_name,
                                    &state.config,
                                );
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
                let _ = crate::config::save_profile(&state.current_save_entry_name, &state.config);
                window.set_visible(false);
                let config = state.config.clone();
                let com_port = if state.com_ports.is_empty() {
                    Some("COM1".to_string())
                } else {
                    Some(state.com_ports[state.selected_com_port].clone())
                };
                state.config.com_port = com_port.clone().unwrap_or_else(|| "COM1".to_string());

                let need_spawn = if let Some(ref shared) = state.runner_shared {
                    if let Ok(lock) = shared.lock() {
                        !lock.thread_running
                    } else {
                        true
                    }
                } else {
                    true
                };

                if need_spawn {
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
                            license_error: None,
                            runtime_error: None,
                        }));
                    state.runner_shared = Some(shared.clone());
                    let thread_proxy = proxy.clone();
                    let handle = std::thread::spawn(move || {
                        crate::runner::run_action_loop(shared, thread_proxy);
                    });
                    state.runner_thread = Some(handle);
                } else {
                    if let Some(ref shared) = state.runner_shared {
                        if let Ok(mut lock) = shared.lock() {
                            lock.config = config;
                            lock.com_port = com_port;
                            lock.active = true;
                        }
                    }
                }
            }
        }
        glium::winit::event::Event::WindowEvent {
            event: glium::winit::event::WindowEvent::CloseRequested,
            ..
        } => {
            if let Some(ref shared) = state.runner_shared {
                if let Ok(mut lock) = shared.lock() {
                    lock.exit = true;
                }
            }
            if let Some(handle) = state.runner_thread.take() {
                let _ = handle.join();
            }
            crate::license::end_usage_session();
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
                draw_start_bar(ui, state);
                draw_tabs(ui, state);
            }
        });
}

fn draw_login(ui: &Ui, state: &mut UiState) {
    if !state.logged_in {
        if ui.button("Login") {
            state.login_notify.clear();
            ui.open_popup("login_popup");
        }
        if ui.button("Exit") {
            crate::license::end_usage_session();
            std::process::exit(0);
        }
        if !state.auth_notify.is_empty() {
            ui.text(&state.auth_notify);
        }
        ui.modal_popup_config("login_popup")
            .always_auto_resize(true)
            .build(|| {
                ui.set_next_item_width(220.0);
                ui.input_text("License Key", &mut state.license_key).build();
                if !state.login_notify.is_empty() {
                    ui.text(&state.login_notify);
                }
                if ui.button("Submit") {
                    let res = crate::license::verify_license_key(&state.license_key);
                    if res.valid {
                        match crate::license::begin_usage_session(&state.license_key, &res) {
                            Ok(_) => {
                                let _ = crate::config::save_license_key(&state.license_key);
                                state.logged_in = true;
                                state.login_notify.clear();
                                state.auth_notify = crate::license::format_user_info_for_ui(&res);
                                ui.close_current_popup();
                            }
                            Err(e) => {
                                state.login_notify = format!("Session error: {}", e);
                            }
                        }
                    } else {
                        state.login_notify = res.message.clone();
                    }
                }
            });
    }
}

fn draw_start_bar(ui: &Ui, state: &mut UiState) {
    let label = format!("Verux.software [APB.RELOADED {APP_VERSION}]");
    TabBar::new("maku").build(ui, || {
        TabItem::new(&label).build(ui, || {
            if ui.button("Start") {
                state.start_requested = true;
            }
            if state.config.coordinate_source == CoordinateSource::Memory {
                ui.text_wrapped(crate::runner::MEMORY_SOURCE_NOTICE);
            }
            if ui.button("Exit") {
                if let Some(ref shared) = state.runner_shared {
                    if let Ok(mut lock) = shared.lock() {
                        lock.exit = true;
                    }
                }
                if let Some(handle) = state.runner_thread.take() {
                    let _ = handle.join();
                }
                crate::license::end_usage_session();
                std::process::exit(0);
            }
        });
    });
}

fn draw_tabs(ui: &Ui, state: &mut UiState) {
    TabBar::new("tabs").build(ui, || {
        TabItem::new("Basic Settings").build(ui, || draw_basic_settings(ui, state));
        if state.show_targeting_tab {
            TabItem::new("Targetting").build(ui, || draw_targeting(ui, state));
        }
        TabItem::new("Aimbot").build(ui, || draw_aimbot(ui, state));
        TabItem::new("Triggerbot").build(ui, || draw_triggerbot(ui, state));
        // Visuals tab is disabled functionally, config driven only.
        // TabItem::new("Visuals").build(ui, || draw_visuals(ui, state));
        TabItem::new("Misc").build(ui, || draw_misc(ui, state));
    });
}

fn draw_basic_settings(ui: &Ui, state: &mut UiState) {
    input_vec2(
        ui,
        "Monitor Resolution",
        &mut state.config.monitor_resolution,
    );
    input_vec2(ui, "Grab Resolution", &mut state.config.grab_resolution);
    let mut fps = state.config.fps as i32;
    if ui.input_int("FPS", &mut fps).build() {
        state.config.fps = fps.max(1) as u32;
    }
    radio_coordinate_source(ui, "Coordinate Source", &mut state.config.coordinate_source);

    let pause_label = if state.binding_target == Some(BindingTarget::Pause) {
        "Pause Keybind: Waiting for input...".to_string()
    } else {
        format!("Pause Keybind:{}", state.config.pause_key_name)
    };
    if ui.button(pause_label) {
        state.binding_target = Some(BindingTarget::Pause);
        state.awaiting_release = true;
    }

    let block_label = if state.binding_target == Some(BindingTarget::Block) {
        "Block Keybind: Waiting for input...".to_string()
    } else {
        format!("Block Keybind:{}", state.config.block_key_name)
    };
    if ui.button(block_label) {
        state.binding_target = Some(BindingTarget::Block);
        state.awaiting_release = true;
    }

    let aim_label = if state.binding_target == Some(BindingTarget::Aimbot) {
        "Aimbot Keybind: Waiting for input...".to_string()
    } else {
        format!("Aimbot Keybind:{}", state.config.aim_key_name)
    };
    if ui.button(aim_label) {
        state.binding_target = Some(BindingTarget::Aimbot);
        state.awaiting_release = true;
    }

    let trig_label = if state.binding_target == Some(BindingTarget::Triggerbot) {
        "Triggerbot Keybind: Waiting for input...".to_string()
    } else {
        format!("Triggerbot Keybind:{}", state.config.trigger_key_name)
    };
    if ui.button(trig_label) {
        state.binding_target = Some(BindingTarget::Triggerbot);
        state.awaiting_release = true;
    }

    TabBar::new("apbconfig").build(ui, || {
        TabItem::new("Verux Config").build(ui, || {
            ui.text("Copy the path to the [APB Reloaded] folder and paste it below \nSample:\nD:\\SteamLibrary\\steamapps\\common\\APB Reloaded");
            ui.input_text("Target Folder", &mut state.config.apb_path)
                .build();
            if ui.button("Apply verux.config") {
                let folder = state.config.apb_path.trim();
                if folder.is_empty() {
                    state.status = "Please enter a target folder path.".to_string();
                } else {
                    match crate::config::deploy_verux_config(folder) {
                        Ok(_) => state.status = "Config package updated successfully!".to_string(),
                        Err(e) => state.status = format!("Failed to update config: {e}"),
                    }
                }
            }
            // ImGui hides controls by not submitting them to the current frame.
            const SHOW_EXPERIMENTAL_CONFIG: bool = false;
            if SHOW_EXPERIMENTAL_CONFIG {
                if ui.button("Experimental config") {
                    let folder = state.config.apb_path.trim();
                    if folder.is_empty() {
                        state.status = "Please enter a target folder path.".to_string();
                    } else {
                        match crate::config::deploy_experimental_config(folder) {
                            Ok(_) => state.status = "Config package updated successfully!".to_string(),
                            Err(e) => state.status = format!("Failed to update config: {e}"),
                        }
                    }
                }
                ui.text("WARNING: EXPERIMENTAL CONFIG WILL TAKE 1HR TO COMPILE ON FIRST USE");
            }
            if !state.status.is_empty() {
                ui.text(&state.status);
            }
        });
    });
}

fn draw_targeting(ui: &Ui, state: &mut UiState) {
    ui.text("APB aiming moves horizontally.");
    ui.input_int("Aim Shift X", &mut state.config.aim_shift.x)
        .build();
}

fn draw_aimbot(ui: &Ui, state: &mut UiState) {
    labeled_separator(ui, "Activation");
    ui.checkbox("Aimbot", &mut state.config.aimbot);
    radio_aim_activation(ui, "Base Activation", &mut state.config.aim_base_activation);

    labeled_separator(ui, "Target Conditions");
    ui.checkbox("On Aim", &mut state.config.aim_on_aim);
    if state.config.aim_on_aim {
        ui.input_int("Aim Distance", &mut state.config.aim_distance)
            .build();
    }

    labeled_separator(ui, "Behavior");
    ui.checkbox("Smoothing", &mut state.config.smoothbot);
    if state.config.smoothbot {
        radio_smooth_type(ui, "Smooth Type", &mut state.config.smooth_type);
        ui.slider("Smoothing Value", 1.0, 10.0, &mut state.config.smoothing);
    }
    ui.checkbox("Predictive Braking", &mut state.config.predictive_braking);
    let mut ramp_enabled = state.config.ramp_time_ms > 0;
    if ui.checkbox("Ramp", &mut ramp_enabled) {
        if ramp_enabled {
            state.config.ramp_time_ms = 280;
        } else {
            state.config.ramp_time_ms = 0;
        }
    }
    if ramp_enabled {
        ui.input_scalar("Ramp Time MS", &mut state.config.ramp_time_ms)
            .build();
        ui.input_scalar("Ramp Idle Reset MS", &mut state.config.ramp_idle_reset_ms)
            .build();
        radio_ramp_apply(ui, "Ramp Apply", &mut state.config.ramp_apply);
        radio_ease(ui, "Ramp Ease", &mut state.config.ramp_ease);
    }

    labeled_separator(ui, "Modifiers");
    ui.checkbox("Sticky", &mut state.config.proximity_charge);
    if state.config.proximity_charge {
        ui.input_int("Range", &mut state.config.proximity_distance)
            .build();
        ui.input_scalar("Build MS", &mut state.config.proximity_build_ms)
            .build();
        radio_prox_effect(ui, "Effect", &mut state.config.proximity_effect);
        radio_ease(ui, "Curve", &mut state.config.proximity_ease);
    }
}

fn draw_triggerbot(ui: &Ui, state: &mut UiState) {
    ui.checkbox("Triggerbot", &mut state.config.triggerbot);
    radio_trigger_activation(
        ui,
        "Trigger Activation",
        &mut state.config.trigger_activation,
    );
    TabBar::new("delays").build(ui, || {
        TabItem::new("Delay").build(ui, || {
            ui.slider("Delay", 1, 1000, &mut state.config.trigger_delay_ms);
        });
    });
}

#[allow(dead_code)]
fn draw_visuals(ui: &Ui, state: &mut UiState) {
    ui.checkbox("Visuals", &mut state.config.visuals);
    let vis_label = if state.binding_target == Some(BindingTarget::Visuals) {
        "Visuals Keybind: Waiting for input...".to_string()
    } else {
        format!("Visuals Keybind:{}", state.config.visuals_key_name)
    };
    if ui.button(vis_label) {
        state.binding_target = Some(BindingTarget::Visuals);
        state.awaiting_release = true;
    }
    ui.text("Will not work on fullscreen applications");
    ui.checkbox("Lines", &mut state.config.lines);
    ui.color_edit3("Linescol", &mut state.lines_color);
    ui.checkbox("FOV", &mut state.config.fov);
    ui.color_edit3("FoVcol", &mut state.fov_color);
}

fn draw_misc(ui: &Ui, state: &mut UiState) {
    TabBar::new("profile").build(ui, || {
        TabItem::new("Profile").build(ui, || {
            if !state.auth_notify.is_empty() {
                ui.text(&state.auth_notify);
            }
        });
    });
    TabBar::new("mouse").build(ui, || {
        TabItem::new("Mouse Method (Arduino Only)").build(ui, || {
            if state.com_ports.is_empty() {
                ui.text("No COM ports detected");
            } else {
                let ports: Vec<&str> = state.com_ports.iter().map(String::as_str).collect();
                if ui.combo_simple_string("Arduino COM Port", &mut state.selected_com_port, &ports)
                {
                    if let Some(port) = state.com_ports.get(state.selected_com_port) {
                        state.config.com_port = port.clone();
                        let _ = crate::config::save_profile(
                            &state.current_save_entry_name,
                            &state.config,
                        );
                    }
                }
            }
            if ui.button("Refresh COM ports") {
                let preferred = state.config.com_port.clone();
                state.com_ports = serialport::available_ports()
                    .map(|ports| ports.into_iter().map(|port| port.port_name).collect())
                    .unwrap_or_default();
                state.selected_com_port = state
                    .com_ports
                    .iter()
                    .position(|port| port.eq_ignore_ascii_case(&preferred))
                    .or_else(|| {
                        state
                            .com_ports
                            .iter()
                            .position(|port| port.eq_ignore_ascii_case("COM1"))
                    })
                    .unwrap_or(0);
                if let Some(port) = state.com_ports.get(state.selected_com_port) {
                    state.config.com_port = port.clone();
                    let _ =
                        crate::config::save_profile(&state.current_save_entry_name, &state.config);
                }
            }
            ui.same_line();
            if ui.button("Refresh APB PID") {
                match crate::apb_process::find_running_apb_pid() {
                    Ok(pid) => {
                        state.apb_pid = Some(pid);
                        state.apb_process_status.clear();
                        crate::log_info!("Verified APB.exe process handle for PID {pid}");
                    }
                    Err(error) => {
                        state.apb_pid = None;
                        state.apb_process_status = error;
                        crate::log_err!("APB process refresh failed: {}", state.apb_process_status);
                    }
                }
            }
            if let Some(pid) = state.apb_pid {
                ui.text(format!("APB PID: {pid} (verified on refresh)"));
            } else if !state.apb_process_status.is_empty() {
                ui.text_wrapped(&state.apb_process_status);
            } else {
                ui.text("APB PID: not checked");
            }

            let rates = vec!["125 Hz", "250 Hz", "500 Hz", "1000 Hz"];
            let mut selected_rate_idx = match state.config.mouse_polling_rate {
                125 => 0,
                250 => 1,
                500 => 2,
                1000 => 3,
                _ => 2, // default 500 Hz
            };
            if ui.combo_simple_string("Mouse Polling Rate", &mut selected_rate_idx, &rates) {
                state.config.mouse_polling_rate = match selected_rate_idx {
                    0 => 125,
                    1 => 250,
                    2 => 500,
                    3 => 1000,
                    _ => 500,
                };
            }
        });
    });
    TabBar::new("savehsvtabs").build(ui, || {
        TabItem::new("Save Current Config").build(ui, || {
            ui.text("Save program settings to entry");

            if ui.button("Save") {
                state.config.lines_color = float_to_rgb(state.lines_color);
                state.config.fov_color = float_to_rgb(state.fov_color);
                state.config.ui_color = float_to_rgb(state.ui_color);

                let name = state.current_save_entry_name.clone();
                let name = name.trim();
                if !name.is_empty() {
                    if let Err(e) = crate::config::save_profile(name, &state.config) {
                        state.status = format!("Failed to save config: {}", e);
                    } else {
                        state.status = "Config saved successfully!".to_string();
                        state.profiles = crate::config::get_profile_names();
                        if let Some(pos) = state.profiles.iter().position(|p| p == name) {
                            state.selected_profile = pos;
                        }
                    }
                }
            }
        });
    });
    TabBar::new("miss").build(ui, || {
        TabItem::new("Misc").build(ui, || {
            ui.color_edit3("UI Color", &mut state.ui_color);
        });
    });
}

fn input_vec2(ui: &Ui, label: &str, value: &mut Vec2i) {
    let mut data = [value.x, value.y];
    if ui.input_int2(label, &mut data).build() {
        value.x = data[0];
        value.y = data[1];
    }
}

fn radio_aim_activation(ui: &Ui, label: &str, value: &mut AimActivation) {
    ui.text(label);
    ui.radio_button("Toggle", value, AimActivation::Toggle);
    ui.same_line();
    ui.radio_button("Hold", value, AimActivation::Hold);
    ui.same_line();
    ui.radio_button("Timed", value, AimActivation::Timed);
}

fn radio_trigger_activation(ui: &Ui, label: &str, value: &mut TriggerActivation) {
    ui.text(label);
    ui.radio_button("Toggle", value, TriggerActivation::Toggle);
    ui.same_line();
    ui.radio_button("Hold", value, TriggerActivation::Hold);
    ui.same_line();
    ui.radio_button("Double Crouch", value, TriggerActivation::DoubleCrouch);
}

fn radio_coordinate_source(ui: &Ui, label: &str, value: &mut CoordinateSource) {
    ui.text(label);
    ui.radio_button("Color Detection", value, CoordinateSource::ColorDetection);
    ui.same_line();
    ui.radio_button("Memory", value, CoordinateSource::Memory);
}

fn radio_smooth_type(ui: &Ui, label: &str, value: &mut SmoothType) {
    ui.text(label);
    ui.radio_button("Linear", value, SmoothType::Linear);
    ui.same_line();
    ui.radio_button("Logarithmic", value, SmoothType::Logarithmic);
    ui.same_line();
    ui.radio_button("Hyperbolic", value, SmoothType::Hyperbolic);
    ui.same_line();
    ui.radio_button("Bezier", value, SmoothType::Bezier);
}

fn radio_ramp_apply(ui: &Ui, label: &str, value: &mut RampApply) {
    ui.text(label);
    ui.radio_button("delta", value, RampApply::Delta);
    ui.same_line();
    ui.radio_button("smoothing", value, RampApply::Smoothing);
}

fn radio_ease(ui: &Ui, label: &str, value: &mut EaseCurve) {
    ui.text(label);
    ui.radio_button("linear", value, EaseCurve::Linear);
    ui.same_line();
    ui.radio_button("smoothstep", value, EaseCurve::Smoothstep);
    ui.same_line();
    ui.radio_button("S-curve", value, EaseCurve::Smootherstep);
    ui.same_line();
    ui.radio_button("quad", value, EaseCurve::Quad);
    ui.same_line();
    ui.radio_button("cubic", value, EaseCurve::Cubic);
}

fn radio_prox_effect(ui: &Ui, label: &str, value: &mut ProximityEffect) {
    ui.text(label);
    ui.radio_button("strength", value, ProximityEffect::Strength);
    ui.same_line();
    ui.radio_button("smoothing", value, ProximityEffect::Smoothing);
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
