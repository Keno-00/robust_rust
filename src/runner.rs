use crate::{log_err, log_info};
use glium::winit::event_loop::EventLoopProxy;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

use crate::activation::{AimActivationState, TargetPoint, TriggerActivationState};
use crate::capture::DxgiCapture;
use crate::config::ProfileConfig;
use crate::detection::{check_trigger_color, detect_target_merged, partition_aim_image};
use crate::input::ArduinoSerial;
use crate::modifiers::{DecayController, ProximityCharge, ProximityChargeState, RampController};
use crate::overlay::OverlayWindow;
use crate::smoothing::{smooth_delta, BezierSmoother};

struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.state >> 32) as u32
    }

    fn next_f64(&mut self) -> f64 {
        f64::from(self.next_u32()) / f64::from(u32::MAX)
    }
}

#[derive(Debug, Clone)]
pub struct RunnerControl {
    pub active: bool,
    pub config: ProfileConfig,
    pub com_port: Option<String>,
    pub exit: bool,
    pub thread_running: bool,
    pub license_error: Option<String>,
}

struct MouseStepState {
    pub remaining_x: f64,
    pub remaining_y: f64,
    pub remaining_ticks: u32,
    pub shoot_requested: bool,
    pub com_port: Option<String>,
    pub mouse_polling_rate: u32,
    pub exit: bool,
}

fn mouse_sender_thread(mouse_state: Arc<Mutex<MouseStepState>>) {
    let mut arduino: Option<ArduinoSerial> = None;
    let mut last_com_port: Option<String> = None;
    let mut last_open_attempt: Option<Instant> = None;

    loop {
        // 1. Read shared state parameters
        let (shoot_requested, com_port, polling_rate, exit) = {
            let mut lock = match mouse_state.lock() {
                Ok(l) => l,
                Err(_) => break, // poisoned mutex
            };
            if lock.exit {
                break;
            }
            let shoot_requested = lock.shoot_requested;
            lock.shoot_requested = false;

            (
                shoot_requested,
                lock.com_port.clone(),
                lock.mouse_polling_rate,
                lock.exit,
            )
        };

        if exit {
            break;
        }

        // 2. Manage Arduino Serial Port
        if com_port != last_com_port {
            arduino = None;
            last_open_attempt = None;
            last_com_port = com_port.clone();
        }

        if arduino.is_none() && last_com_port.is_some() {
            let should_attempt = match last_open_attempt {
                None => true,
                Some(inst) => inst.elapsed() >= Duration::from_secs(2),
            };

            if should_attempt {
                last_open_attempt = Some(Instant::now());
                if let Some(ref port) = last_com_port {
                    match ArduinoSerial::open(port) {
                        Ok(s) => {
                            log_info!("Successfully opened Arduino serial on {}", port);
                            arduino = Some(s);
                        }
                        Err(e) => {
                            log_err!("Failed to open Arduino serial on {}: {}", port, e);
                        }
                    }
                }
            }
        }

        // 3. Process Shoot Command
        let mut write_failed = false;
        if shoot_requested {
            if let Some(ref mut ard) = arduino {
                if ard.shoot().is_err() {
                    write_failed = true;
                }
            }
        }

        // 4. Process Move Delta
        let mut send_x = 0;
        let mut send_y = 0;

        {
            let mut lock = match mouse_state.lock() {
                Ok(l) => l,
                Err(_) => break,
            };

            if lock.remaining_ticks > 0 {
                let step_x = lock.remaining_x / lock.remaining_ticks as f64;
                let step_y = lock.remaining_y / lock.remaining_ticks as f64;

                send_x = step_x.round() as i32;
                send_y = step_y.round() as i32;

                lock.remaining_x -= send_x as f64;
                lock.remaining_y -= send_y as f64;
                lock.remaining_ticks -= 1;
            }
        }

        // 5. Send Move command to Arduino
        if send_x != 0 || send_y != 0 {
            if let Some(ref mut ard) = arduino {
                if ard.move_mouse(send_x, send_y).is_err() {
                    write_failed = true;
                }
            }
        }

        if write_failed {
            log_err!(
                "Serial write failed on {:?}. Disconnect detected, attempting reconnect...",
                last_com_port
            );
            arduino = None;
            last_open_attempt = None; // Reset to force immediate retry
        }

        // Sleep dynamically based on polling rate (125Hz=8ms, 250Hz=4ms, 500Hz=2ms, 1000Hz=1ms)
        let sleep_ms = match polling_rate {
            125 => 8,
            250 => 4,
            500 => 2,
            1000 => 1,
            _ => 2,
        };
        thread::sleep(Duration::from_millis(sleep_ms));
    }
}

struct RunnerCleanup {
    shared_state: Arc<Mutex<RunnerControl>>,
    mouse_state: Arc<Mutex<MouseStepState>>,
    sender_handle: Option<std::thread::JoinHandle<()>>,
    license_handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for RunnerCleanup {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared_state.lock() {
            state.thread_running = false;
            state.active = false;
            state.exit = true;
        }
        if let Ok(mut mstate) = self.mouse_state.lock() {
            mstate.exit = true;
        }
        if let Some(handle) = self.sender_handle.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.license_handle.take() {
            let _ = handle.join();
        }
    }
}

pub fn run_action_loop(shared_state: Arc<Mutex<RunnerControl>>, event_proxy: EventLoopProxy<()>) {
    // 1. Get initial config and com_port
    let (config, com_port) = {
        let state = match shared_state.lock() {
            Ok(s) => s,
            Err(_) => return, // poisoned mutex, exit thread
        };
        (state.config.clone(), state.com_port.clone())
    };

    let mouse_state = Arc::new(Mutex::new(MouseStepState {
        remaining_x: 0.0,
        remaining_y: 0.0,
        remaining_ticks: 0,
        shoot_requested: false,
        com_port: com_port.clone(),
        mouse_polling_rate: config.mouse_polling_rate,
        exit: false,
    }));

    let mouse_state_for_thread = mouse_state.clone();
    let sender_handle = std::thread::spawn(move || {
        mouse_sender_thread(mouse_state_for_thread);
    });

    let license_state = shared_state.clone();
    let license_proxy = event_proxy.clone();
    let license_handle = std::thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(1));
        let should_exit = license_state.lock().map(|state| state.exit).unwrap_or(true);
        if should_exit {
            break;
        }
        if let Some(result) = crate::license::heartbeat_usage_session(false) {
            if !result.valid {
                if let Ok(mut state) = license_state.lock() {
                    state.active = false;
                    state.exit = true;
                    state.license_error = Some(result.message);
                }
                let _ = license_proxy.send_event(());
                break;
            }
        }
    });

    let mut _cleanup = RunnerCleanup {
        shared_state: shared_state.clone(),
        mouse_state: mouse_state.clone(),
        sender_handle: Some(sender_handle),
        license_handle: Some(license_handle),
    };

    // Setup base timers
    let start_instant = Instant::now();
    let get_time_s = move || start_instant.elapsed().as_secs_f64();
    let mut lcg = Lcg::new(start_instant.elapsed().as_nanos() as u64);

    // Initialize DXGI Capture
    let grab_w = config.grab_resolution.x;
    let grab_h = config.grab_resolution.y;
    let mut capture = match DxgiCapture::new(grab_w.max(1) as u32, grab_h.max(1) as u32) {
        Ok(c) => Some(c),
        Err(e) => {
            log_err!("Failed to initialize DXGI Capture: {e}");
            None
        }
    };

    // Initialize Overlay Window if visuals enabled
    let overlay = if config.visuals {
        match OverlayWindow::new() {
            Ok(ov) => {
                ov.show();
                Some(ov)
            }
            Err(e) => {
                log_err!("Failed to initialize overlay: {e}");
                None
            }
        }
    } else {
        None
    };

    // Aimbot / triggerbot state trackers
    let mut aim_state =
        AimActivationState::new(config.aimbot, config.aim_base_activation, config.aim_on_aim);
    let mut trigger_state =
        TriggerActivationState::new(config.triggerbot, config.trigger_activation);
    let mut ramp = RampController::new(
        config.ramp_time_ms,
        config.ramp_idle_reset_ms,
        config.ramp_apply,
        config.ramp_ease,
    );
    let mut decay = DecayController::new(config.ramp_time_ms, config.ramp_ease);
    let mut prox_state = ProximityChargeState::default();
    let mut bezier = BezierSmoother::new(get_time_s());

    let mut last_shoot_time = Instant::now();
    let mut current_shoot_delay = Duration::from_millis(0);

    let mut prev_aim_active = false;
    let mut last_sent_dx = 0;
    let mut last_sent_dy = 0;

    let mut visuals_active = config.visuals;
    let mut visuals_toggle_ready_at_s = 0.0;

    let mut image_buf = Vec::new();
    let mut ws = crate::detection::DetectionWorkspace::new();

    let ticks_per_frame = (config.mouse_polling_rate as f64 / config.fps as f64)
        .round()
        .max(1.0) as u32;

    let mut next_tick = Instant::now();

    loop {
        // Check exit flag
        let exit = {
            if let Ok(state) = shared_state.lock() {
                state.exit
            } else {
                true // poisoned mutex, exit thread
            }
        };

        if exit {
            break;
        }

        // Check Pause hotkey safely
        let now = get_time_s();
        unsafe {
            if config.pause_key > 0
                && config.pause_key < 255
                && GetAsyncKeyState(config.pause_key as i32) < 0
            {
                // Signal pause/show winit window and exit the runner thread
                let _ = event_proxy.send_event(());
                break;
            }

            // Check Block key safely
            if config.block_key > 0
                && config.block_key < 255
                && GetAsyncKeyState(config.block_key as i32) < 0
            {
                thread::sleep(Duration::from_millis(10));
                continue;
            }

            // Check Visuals key safely (only if visuals are configured)
            if config.visuals
                && config.visuals_key > 0
                && config.visuals_key < 255
                && GetAsyncKeyState(config.visuals_key as i32) < 0
                && now >= visuals_toggle_ready_at_s
            {
                visuals_active = !visuals_active;
                visuals_toggle_ready_at_s = now + 0.5;
                if let Some(ref ov) = overlay {
                    if visuals_active {
                        ov.show();
                    } else {
                        ov.hide();
                    }
                }
            }
        }

        let monitor_w = config.monitor_resolution.x;
        let monitor_h = config.monitor_resolution.y;
        let shift_x = config.grab_shift.x;
        let shift_y = config.grab_shift.y;

        let center_x = monitor_w / 2;
        let center_y = monitor_h / 2;

        let region_left = center_x - grab_w / 2 + shift_x;
        let region_top = center_y - grab_h / 2 + shift_y;

        // Screen Capture
        if let Some(ref mut cap) = capture {
            if let Err(e) = cap.capture_region(region_left, region_top, &mut image_buf) {
                log_err!("Capture failed: {e}. Reinitializing DXGI Capture...");
                capture = match DxgiCapture::new(grab_w.max(1) as u32, grab_h.max(1) as u32) {
                    Ok(c) => Some(c),
                    Err(re_err) => {
                        log_err!("Failed to reinitialize DXGI Capture: {re_err}");
                        None
                    }
                };
                thread::sleep(Duration::from_millis(500)); // Sleep 500ms to let DXGI system settle
                continue;
            }
        }

        // Triggerbot check safely
        let trigger_key_down = unsafe {
            config.trigger_key > 0
                && config.trigger_key < 255
                && GetAsyncKeyState(config.trigger_key as i32) < 0
        };
        let crouch_key_down = unsafe { GetAsyncKeyState(17) < 0 }; // VK_CONTROL = 17

        let triggerbot_enabled = trigger_state.update(trigger_key_down, crouch_key_down, now);

        if triggerbot_enabled {
            let trigger_target_found =
                check_trigger_color(&image_buf, grab_w as usize, grab_h as usize, &config);
            if trigger_target_found {
                if last_shoot_time.elapsed() >= current_shoot_delay {
                    if let Ok(mut mstate) = mouse_state.lock() {
                        mstate.shoot_requested = true;
                    }

                    // Reset shoot timer with base delay + random jitter matching Python
                    let base_delay = config.trigger_delay_ms as f64;
                    let jitter = 40.0 + lcg.next_f64() * 40.0;
                    let sign = if lcg.next_f64() < 0.5 { -1.0 } else { 1.0 };
                    let randomized_delay_ms = (base_delay + sign * jitter).max(0.0);

                    current_shoot_delay = Duration::from_millis(randomized_delay_ms as u64);
                    last_shoot_time = Instant::now();
                }
            }
        }

        // Aimbot check safely
        let aim_key_down = unsafe {
            config.aim_key > 0
                && config.aim_key < 255
                && GetAsyncKeyState(config.aim_key as i32) < 0
        };

        // Partition captured screen frame (removes crosshair vertical column and bottom half)
        let (partitioned_img, part_w, part_h) =
            partition_aim_image(&image_buf, grab_w as usize, grab_h as usize);

        // Run target detection on partitioned image
        let target = detect_target_merged(&partitioned_img, part_w, part_h, &config, &mut ws);

        let target_seen = target.is_some() && target.as_ref().unwrap().seen;
        let target_point = target.map(|t| TargetPoint::new(t.x, t.y));

        let aim_active = aim_state.update(
            aim_key_down,
            target_point,
            config.aim_shift,
            config.aim_distance,
            now,
        );

        // Manage Ramp transition states
        if aim_active && !prev_aim_active {
            ramp.arm(now);
        } else if !aim_active && prev_aim_active {
            ramp.disarm();
            decay.arm(last_sent_dx, last_sent_dy, now);
        }
        prev_aim_active = aim_active;

        if aim_active {
            ramp.check_idle_reset(now);
            if target_seen && !ramp.is_armed() {
                ramp.arm(now);
            }
        }

        let mut drawn_target = None;

        if target_seen {
            let t = target.unwrap();
            let raw_dx = t.x + config.aim_shift.x;
            let raw_dy = if config.x_only {
                0
            } else {
                t.y + config.aim_shift.y
            };

            drawn_target = Some((t.x, t.y, t.width, t.height));

            if aim_active {
                // Proximity Charge modifier
                let prox_config = ProximityCharge {
                    enabled: config.proximity_charge,
                    distance: config.proximity_distance,
                    build_ms: config.proximity_build_ms,
                    effect: config.proximity_effect,
                    ease: config.proximity_ease,
                };
                prox_state.update(prox_config, aim_active, raw_dx, raw_dy, now);

                // Ramp modifiers
                let (mut dx_r, mut dy_r, mut eff_smoothing) =
                    ramp.apply(raw_dx, raw_dy, config.smoothing, now);

                let (dx_p, dy_p, eff_smooth_p) =
                    prox_state.apply(dx_r, dy_r, eff_smoothing, config.proximity_effect);
                dx_r = dx_p;
                dy_r = dy_p;
                eff_smoothing = eff_smooth_p;

                // Aim smoothing path selection
                let (aim_dx, aim_dy) = smooth_delta(
                    if config.smoothbot {
                        Some(config.smooth_type)
                    } else {
                        None
                    },
                    dx_r,
                    dy_r,
                    eff_smoothing,
                    grab_w,
                    grab_h,
                    &mut bezier,
                    now,
                );

                // Send move packet to mouse sender thread
                if let Ok(mut mstate) = mouse_state.lock() {
                    mstate.remaining_x = aim_dx as f64;
                    mstate.remaining_y = aim_dy as f64;
                    mstate.remaining_ticks = ticks_per_frame;
                }

                last_sent_dx = aim_dx;
                last_sent_dy = aim_dy;
            }
        } else {
            // Target lost: run tail ease-down to zero
            if aim_active {
                if !decay.is_active() {
                    decay.arm(last_sent_dx, last_sent_dy, now);
                }
                let (decay_dx, decay_dy, cont) = decay.delta(now);
                if cont {
                    if let Ok(mut mstate) = mouse_state.lock() {
                        mstate.remaining_x = decay_dx as f64;
                        mstate.remaining_y = decay_dy as f64;
                        mstate.remaining_ticks = ticks_per_frame;
                    }
                    last_sent_dx = decay_dx;
                    last_sent_dy = decay_dy;
                } else {
                    last_sent_dx = 0;
                    last_sent_dy = 0;
                }
            } else {
                decay.disarm();
                last_sent_dx = 0;
                last_sent_dy = 0;
            }
        }

        // Update HUD Graphics Overlay
        if visuals_active {
            if let Some(ref ov) = overlay {
                ov.draw_frame(
                    center_x,
                    center_y,
                    shift_x,
                    shift_y,
                    grab_w,
                    grab_h,
                    drawn_target,
                    (config.aim_shift.x, config.aim_shift.y),
                    config.lines_color,
                    config.fov_color,
                    config.lines,
                    config.fov,
                    config.x_only,
                );
            }
        }

        // Precise loop pacing
        let frame_duration = Duration::from_secs_f64(1.0 / (config.fps as f64).max(1.0));
        next_tick += frame_duration;
        let now_inst = Instant::now();
        if now_inst > next_tick {
            next_tick = now_inst;
        } else {
            let mut remaining = next_tick.duration_since(now_inst);
            while remaining > Duration::from_micros(300) {
                if remaining > Duration::from_micros(1300) {
                    thread::sleep(remaining - Duration::from_micros(1000));
                } else {
                    std::hint::spin_loop();
                }
                remaining = next_tick.duration_since(Instant::now());
            }
            while Instant::now() < next_tick {
                std::hint::spin_loop();
            }
        }
    }
}
