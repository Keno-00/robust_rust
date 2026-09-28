use crate::{log_err, log_info};
use glium::winit::event_loop::EventLoopProxy;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

use crate::activation::{AimActivationState, TargetPoint, TriggerActivationState};
use crate::apb_detection::{detect_name, trigger_has_red, ApbDetectionWorkspace};
use crate::apb_memory::{self, ApbMemorySource, Discovery};
use crate::apb_process::find_running_apb_pid;
use crate::capture::DxgiCapture;
use crate::config::{CoordinateSource, ProfileConfig};
use crate::input::ArduinoSerial;
use crate::memory_reader::MemoryReader;
use crate::modifiers::{DecayController, ProximityCharge, ProximityChargeState, RampController};
use crate::overlay::OverlayWindow;
use crate::predictive_braking::{PredictiveBrake, SentMotionHistory, MAX_FRAME_AGE};
use crate::smoothing::{smooth_delta, smooth_delta_precise, BezierSmoother};
use std::collections::VecDeque;

pub const MEMORY_SOURCE_NOTICE: &str =
    "Memory aims horizontally at projected nonlocal players, including neutral players. It has no opponent or visibility filter yet. Run Verux as administrator for EchoDriver access. Triggerbot still uses visual red detection.";

fn normalize_mouse_polling_rate(rate: u32) -> u32 {
    match rate {
        125 | 250 | 500 | 1000 => rate,
        _ => 500,
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
    pub runtime_error: Option<String>,
}

fn report_runtime_error(
    shared: &Arc<Mutex<RunnerControl>>,
    proxy: &EventLoopProxy<()>,
    message: String,
) {
    log_err!("{message}");
    if let Ok(mut state) = shared.lock() {
        state.active = false;
        state.exit = true;
        state.runtime_error = Some(message);
    }
    let _ = proxy.send_event(());
}

struct QueuedMouseMove {
    pub remaining_x: f64,
    pub remaining_y: f64,
    pub remaining_ticks: u32,
    pub expires_at: Option<Instant>,
}

struct MouseStepState {
    pub moves: VecDeque<QueuedMouseMove>,
    pub queue_overflow_reported: bool,
    pub arduino_ready: bool,
    pub shoot_requested: bool,
    pub shoot_delay: Duration,
    pub com_port: Option<String>,
    pub mouse_polling_rate: u32,
    pub exit: bool,
    pub predictive_braking: bool,
    pub sent_motion: SentMotionHistory,
}

impl MouseStepState {
    fn cancel_movement(&mut self) {
        self.moves.clear();
        self.queue_overflow_reported = false;
    }

    fn enqueue_move(&mut self, movement: QueuedMouseMove) {
        const MAX_PENDING_MOVES: usize = 8;
        if self.moves.len() >= MAX_PENDING_MOVES {
            self.moves.clear();
            if !self.queue_overflow_reported {
                log_err!(
                    "Mouse movement queue overflowed; discarded stale queued movement. Lower FPS or raise mouse polling rate."
                );
                self.queue_overflow_reported = true;
            }
        }
        self.moves.push_back(movement);
    }
}

fn mouse_sender_thread(mouse_state: Arc<Mutex<MouseStepState>>) {
    let mut arduino: Option<ArduinoSerial> = None;
    let mut last_com_port: Option<String> = None;
    let mut last_open_attempt: Option<Instant> = None;
    let mut next_tick = Instant::now();
    let mut last_polling_rate = 0;
    let mut last_shot_sent: Option<Instant> = None;

    loop {
        // 1. Read shared state parameters
        let (com_port, polling_rate, exit) = {
            let lock = match mouse_state.lock() {
                Ok(l) => l,
                Err(_) => break, // poisoned mutex
            };
            if lock.exit {
                break;
            }
            (lock.com_port.clone(), lock.mouse_polling_rate, lock.exit)
        };

        if exit {
            break;
        }

        // 2. Manage Arduino Serial Port
        if com_port != last_com_port {
            arduino = None;
            last_open_attempt = None;
            last_com_port = com_port.clone();
            if let Ok(mut lock) = mouse_state.lock() {
                lock.arduino_ready = false;
                lock.shoot_requested = false;
                lock.moves.clear();
                lock.queue_overflow_reported = false;
                lock.sent_motion.reset();
            }
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
                            if let Ok(mut lock) = mouse_state.lock() {
                                lock.arduino_ready = true;
                            }
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
        {
            let mut lock = match mouse_state.lock() {
                Ok(l) => l,
                Err(_) => break,
            };
            if lock.exit {
                break;
            }
            // Retain one pending shot through cooldown. Repeated detections only
            // set this same flag; the interval starts after an actual serial write.
            let shot_due = last_shot_sent.is_none_or(|sent| sent.elapsed() >= lock.shoot_delay);
            if lock.shoot_requested && shot_due {
                if let Some(ref mut ard) = arduino {
                    lock.shoot_requested = false;
                    if ard.shoot().is_err() {
                        write_failed = true;
                    } else {
                        last_shot_sent = Some(Instant::now());
                    }
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
            if lock.exit {
                break;
            }

            // A tail must not drain beyond its ramp deadline, even if vision stalls.
            while lock.moves.front().is_some_and(|movement| {
                movement.expires_at.is_some_and(|end| Instant::now() >= end)
            }) {
                lock.moves.pop_front();
            }

            if let Some(movement) = lock.moves.front_mut() {
                if movement.remaining_ticks > 0 {
                    let step_x = movement.remaining_x / movement.remaining_ticks as f64;
                    let step_y = movement.remaining_y / movement.remaining_ticks as f64;

                    send_x = step_x.round() as i32;
                    send_y = step_y.round() as i32;

                    movement.remaining_x -= send_x as f64;
                    movement.remaining_y -= send_y as f64;
                    movement.remaining_ticks -= 1;
                    if movement.remaining_ticks == 0 {
                        lock.moves.pop_front();
                    }
                } else {
                    lock.moves.pop_front();
                }
            }
            if lock.moves.len() < 4 {
                lock.queue_overflow_reported = false;
            }

            // Serialize dequeue/write with cancellation so an extracted delta cannot
            // be sent after the vision thread has cleared the movement queue.
            if send_x != 0 || send_y != 0 {
                if let Some(ref mut ard) = arduino {
                    if ard.move_mouse(send_x, send_y).is_err() {
                        write_failed = true;
                    } else if lock.predictive_braking {
                        lock.sent_motion.record(send_x, Instant::now());
                    }
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
            if let Ok(mut lock) = mouse_state.lock() {
                lock.arduino_ready = false;
                lock.shoot_requested = false;
                lock.moves.clear();
                lock.queue_overflow_reported = false;
                lock.sent_motion.reset();
            }
        }

        // Keep the sender on a deadline so work time does not get added to each interval.
        if polling_rate != last_polling_rate {
            last_polling_rate = polling_rate;
            next_tick = Instant::now();
        }
        let interval = Duration::from_micros(1_000_000 / u64::from(polling_rate.clamp(1, 1000)));
        next_tick += interval;
        let now = Instant::now();
        if next_tick <= now {
            // Do not send a burst to catch up after a late wakeup; queued movement remains intact.
            next_tick = now + interval;
        }
        thread::sleep(next_tick.saturating_duration_since(Instant::now()));
    }
}

struct RunnerCleanup {
    shared_state: Arc<Mutex<RunnerControl>>,
    mouse_state: Arc<Mutex<MouseStepState>>,
    sender_handle: Option<std::thread::JoinHandle<()>>,
    license_handle: Option<std::thread::JoinHandle<()>>,
    memory_refresh_stop: Option<Arc<AtomicBool>>,
    memory_refresh_handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for RunnerCleanup {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared_state.lock() {
            state.thread_running = false;
            state.active = false;
            state.exit = true;
        }
        if let Ok(mut mstate) = self.mouse_state.lock() {
            mstate.cancel_movement();
            mstate.shoot_requested = false;
            mstate.exit = true;
        }
        if let Some(stop) = self.memory_refresh_stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        if let Some(handle) = self.memory_refresh_handle.take() {
            let _ = handle.join();
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

    let mouse_polling_rate = normalize_mouse_polling_rate(config.mouse_polling_rate);
    let mouse_state = Arc::new(Mutex::new(MouseStepState {
        moves: VecDeque::new(),
        queue_overflow_reported: false,
        arduino_ready: false,
        shoot_requested: false,
        shoot_delay: Duration::from_millis(u64::from(config.trigger_delay_ms)),
        com_port: com_port.clone(),
        mouse_polling_rate,
        exit: false,
        predictive_braking: config.predictive_braking,
        sent_motion: SentMotionHistory::new(),
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
        memory_refresh_stop: None,
        memory_refresh_handle: None,
    };

    let mut memory_source = None;
    let mut memory_updates: Option<Receiver<Result<Discovery, String>>> = None;
    if config.coordinate_source == CoordinateSource::Memory {
        let source = find_running_apb_pid()
            .and_then(ApbMemorySource::connect);
        let source = match source {
            Ok(source) => source,
            Err(error) => {
                report_runtime_error(
                    &shared_state,
                    &event_proxy,
                    format!("Memory coordinate source could not start: {error}"),
                );
                return;
            }
        };
        let found = source.discovery();
        log_info!(
            "Memory APB PID {} base 0x{:X}: camera 0x{:X}, local 0x{:X}, {} other player entities, {} readable headers, {} failed headers, {} stale owners.",
            found.pid, found.base, found.camera, found.local_position,
            found.actors.len(), found.readable_headers, found.failed_headers, found.skipped_owners,
        );
        let seed = found.clone();
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let handle = thread::spawn(move || {
            let reader = match MemoryReader::connect() {
                Ok(reader) => reader,
                Err(error) => {
                    let _ = tx.send(Err(format!("Memory refresh could not connect to EchoDriver: {error}")));
                    return;
                }
            };
            let mut current = seed;
            loop {
                for _ in 0..15 {
                    if worker_stop.load(Ordering::Relaxed) {
                        return;
                    }
                    thread::sleep(Duration::from_secs(1));
                }
                if worker_stop.load(Ordering::Relaxed) {
                    return;
                }
                match apb_memory::refresh(&reader, &current) {
                    Ok(updated) => {
                        current = updated.clone();
                        if tx.send(Ok(updated)).is_err() {
                            return;
                        }
                    }
                    Err(error) => {
                        let _ = tx.send(Err(format!("Memory discovery refresh failed: {error}")));
                        return;
                    }
                }
            }
        });
        _cleanup.memory_refresh_stop = Some(stop);
        _cleanup.memory_refresh_handle = Some(handle);
        memory_updates = Some(rx);
        memory_source = Some(source);
    }

    // Setup base timers
    let start_instant = Instant::now();
    let get_time_s = move || start_instant.elapsed().as_secs_f64();

    // Initialize DXGI Capture
    let grab_w = config.grab_resolution.x;
    let grab_h = config.grab_resolution.y;
    let mut capture = match DxgiCapture::new(grab_w.max(1) as u32, grab_h.max(1) as u32) {
        Ok(c) => Some(c),
        Err(e) => {
            log_err!("Failed to initialize DXGI Capture: {e}");
            if config.predictive_braking {
                report_runtime_error(&shared_state, &event_proxy,
                    format!("Predictive braking stopped: DXGI capture could not start: {e}. Restore the display/game and restart."));
                return;
            }
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
    let mut decay_deadline = None;
    let mut prox_state = ProximityChargeState::default();
    let mut bezier = BezierSmoother::new(get_time_s());
    let mut brake = PredictiveBrake::new();
    let mut last_presented_at = None;

    let mut prev_aim_active = false;
    let mut prev_movement_allowed = false;
    let mut last_tracking_dx = 0;

    let mut visuals_active = config.visuals;
    let mut visuals_toggle_ready_at_s = 0.0;

    let mut image_buf = Vec::new();
    let mut ws = ApbDetectionWorkspace::default();
    log_info!(
        "APB detector active: aim red >= {}, trigger red >= {}, red dominance >= {}, horizontal reduction {}. Movement is X-only.",
        crate::apb_detection::AIM_RED_MIN,
        crate::apb_detection::TRIGGER_RED_MIN,
        crate::apb_detection::RED_DOMINANCE_MIN,
        crate::apb_detection::HORIZONTAL_REDUCTION
    );
    log_info!(
        "APB trigger active: original sampling strip, at most one pending shot, minimum click interval {} ms (no random jitter).",
        config.trigger_delay_ms
    );
    if config.predictive_braking {
        log_info!("Predictive braking enabled: curve {:?}, future-landing brake scaled to horizontal grab width. Initial count conversion is provisional at 0.5 px/count and updates from successful ordinary writes; no probe/hold phase. Already-sent movement newer than each image is reserved, unsent tracking commands are replaced, and target-loss tails are disabled.",
            if config.smoothbot { Some(config.smooth_type) } else { None });
    }

    let ticks_per_frame_exact = mouse_polling_rate as f64 / config.fps.max(1) as f64;
    let mut ticks_per_frame_remainder = 0.0;

    let mut next_tick = Instant::now();

    loop {
        if let Some(updates) = &memory_updates {
            match updates.try_recv() {
                Ok(Ok(discovery)) => {
                    if let Some(source) = memory_source.as_mut() {
                        log_info!(
                            "Memory discovery refreshed: {} other player entities, {} failed headers, {} stale owners.",
                            discovery.actors.len(), discovery.failed_headers, discovery.skipped_owners,
                        );
                        if let Err(error) = source.replace_discovery(discovery) {
                            report_runtime_error(&shared_state, &event_proxy, error);
                            return;
                        }
                    }
                }
                Ok(Err(error)) => {
                    report_runtime_error(&shared_state, &event_proxy, error);
                    return;
                }
                Err(TryRecvError::Disconnected) => {
                    report_runtime_error(&shared_state, &event_proxy,
                        "Memory discovery worker stopped unexpectedly.".to_string());
                    return;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
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

        // Distribute non-integer ratios across frames instead of overloading or starving the queue.
        ticks_per_frame_remainder += ticks_per_frame_exact;
        let whole_ticks = ticks_per_frame_remainder.floor() as u32;
        ticks_per_frame_remainder -= f64::from(whole_ticks);
        let ticks_per_frame = whole_ticks.max(1);

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
                if let Ok(mut mstate) = mouse_state.lock() {
                    mstate.cancel_movement();
                    mstate.shoot_requested = false;
                }
                decay.disarm();
                decay_deadline = None;
                last_tracking_dx = 0;
                if prev_movement_allowed {
                    bezier = BezierSmoother::new(now);
                    prox_state.reset();
                    brake.reset_tracking();
                    prev_movement_allowed = false;
                }
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
                if let Ok(mut mstate) = mouse_state.lock() {
                    mstate.cancel_movement();
                    mstate.shoot_requested = false;
                }
                brake.reset_tracking();
                log_err!("Capture failed: {e}. Reinitializing DXGI Capture...");
                capture = match DxgiCapture::new(grab_w.max(1) as u32, grab_h.max(1) as u32) {
                    Ok(c) => Some(c),
                    Err(re_err) => {
                        log_err!("Failed to reinitialize DXGI Capture: {re_err}");
                        if config.predictive_braking {
                            report_runtime_error(&shared_state, &event_proxy,
                                format!("Predictive braking stopped: DXGI capture could not be reinitialized: {re_err}. Restore the display/game and restart."));
                            return;
                        }
                        None
                    }
                };
                thread::sleep(Duration::from_millis(500)); // Sleep 500ms to let DXGI system settle
                continue;
            }
        }
        let fresh_presented_at = capture.as_ref().and_then(DxgiCapture::fresh_presented_at);
        if fresh_presented_at.is_some() {
            last_presented_at = fresh_presented_at;
        }

        // Triggerbot check safely
        let trigger_key_down = unsafe {
            config.trigger_key > 0
                && config.trigger_key < 255
                && GetAsyncKeyState(config.trigger_key as i32) < 0
        };
        let crouch_key_down = unsafe { GetAsyncKeyState(17) < 0 }; // VK_CONTROL = 17

        let triggerbot_enabled = trigger_state.update(trigger_key_down, crouch_key_down, now);

        let trigger_target_found =
            triggerbot_enabled && trigger_has_red(&image_buf, grab_w as usize, grab_h as usize);
        if let Ok(mut mstate) = mouse_state.lock() {
            if !triggerbot_enabled {
                mstate.shoot_requested = false;
            } else if trigger_target_found && mstate.arduino_ready {
                // Latch a brief detection even during cooldown. Never accumulate
                // a backlog of clicks; a pending request is only one boolean.
                mstate.shoot_requested = true;
            }
        }

        // Aimbot check safely
        let aim_key_down = unsafe {
            config.aim_key > 0
                && config.aim_key < 255
                && GetAsyncKeyState(config.aim_key as i32) < 0
        };

        let target = if let Some(source) = memory_source.as_mut() {
            match source.sample(grab_w, grab_h, shift_x, shift_y) {
                Ok(target) => target,
                Err(error) => {
                    report_runtime_error(&shared_state, &event_proxy,
                        format!("Memory coordinate source stopped: {error}"));
                    return;
                }
            }
        } else {
            detect_name(&image_buf, grab_w as usize, grab_h as usize, &mut ws)
        };

        let target_seen = target.is_some() && target.as_ref().unwrap().seen;
        let target_point = target.map(|t| TargetPoint::new(t.x, t.y));

        let aim_active = aim_state.update(
            aim_key_down,
            target_point,
            config.aim_shift,
            config.aim_distance,
            now,
        );

        // Discard old tracking moves on loss, then arm one bounded ramp-down.
        // Arm only on the transition; an expired tail must never restart itself.
        let movement_allowed = aim_active && target_seen;
        let motion_now = get_time_s();
        if config.predictive_braking
            && movement_allowed
            && last_presented_at.is_none_or(|at| at.elapsed() > MAX_FRAME_AGE)
        {
            if let Ok(mut mstate) = mouse_state.lock() {
                mstate.cancel_movement();
                mstate.shoot_requested = false;
            }
            brake.reset_tracking();
            thread::sleep(Duration::from_millis(1));
            continue;
        }
        if !movement_allowed {
            if let Ok(mut mstate) = mouse_state.lock() {
                mstate.cancel_movement();
            }
            if !aim_active {
                decay.disarm();
                decay_deadline = None;
            } else if prev_movement_allowed && !config.predictive_braking {
                decay.arm(last_tracking_dx, 0, motion_now);
                decay_deadline =
                    Some(Instant::now() + Duration::from_millis(u64::from(config.ramp_time_ms)));
            }
            if prev_movement_allowed {
                bezier = BezierSmoother::new(motion_now);
                prox_state.reset();
                brake.reset_tracking();
                last_tracking_dx = 0;
            }
        } else {
            if !prev_movement_allowed {
                // A newly acquired target takes ownership of movement immediately.
                if let Ok(mut mstate) = mouse_state.lock() {
                    mstate.cancel_movement();
                }
            }
            decay.disarm();
            decay_deadline = None;
        }
        prev_movement_allowed = movement_allowed;

        // Manage Ramp transition states
        if aim_active && !prev_aim_active {
            ramp.arm(motion_now);
        } else if !aim_active && prev_aim_active {
            ramp.disarm();
        }
        prev_aim_active = aim_active;

        if aim_active {
            if target_seen && !ramp.is_armed() {
                ramp.arm(motion_now);
            }
            if target_seen {
                // Idle time measures target absence, not time since the ramp was armed.
                // Refresh first so a visible target cannot periodically reset the ramp.
                ramp.note_target_seen(motion_now);
            }
            ramp.check_idle_reset(motion_now);
        }

        let mut drawn_target = None;

        if target_seen {
            let t = target.unwrap();
            let raw_dx = t.x + config.aim_shift.x;
            let raw_dy = 0;

            drawn_target = Some((t.x, t.y, t.width, t.height));

            if aim_active && (!config.predictive_braking || fresh_presented_at.is_some()) {
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
                    ramp.apply(raw_dx, raw_dy, config.smoothing, motion_now);

                let (dx_p, dy_p, eff_smooth_p) =
                    prox_state.apply(dx_r, dy_r, eff_smoothing, config.proximity_effect);
                dx_r = dx_p;
                dy_r = dy_p;
                eff_smoothing = eff_smooth_p;

                let smooth_type = if config.smoothbot {
                    Some(config.smooth_type)
                } else {
                    None
                };
                // Braking uses the same curve with precision retained until the
                // final command. Off uses the original integer path unchanged.
                let aim_dx = if config.predictive_braking {
                    let (desired, _) = smooth_delta_precise(
                        smooth_type,
                        dx_r,
                        dy_r,
                        eff_smoothing,
                        grab_w,
                        grab_h,
                        &mut bezier,
                        now,
                    );
                    let presented_at = fresh_presented_at.unwrap();
                    let motion = match mouse_state.lock() {
                        Ok(state) => {
                            if state.arduino_ready {
                                match state.sent_motion.snapshot(presented_at) {
                                    Some(snapshot) => Some(snapshot),
                                    None => {
                                        report_runtime_error(&shared_state, &event_proxy,
                                            "Predictive braking stopped: successful mouse-write history no longer covers the current image. Restart and check verux_apb.log.".into());
                                        return;
                                    }
                                }
                            } else {
                                None
                            }
                        }
                        Err(_) => {
                            report_runtime_error(&shared_state, &event_proxy,
                                "Predictive braking stopped: mouse sender state is invalid. Restart and check verux_apb.log.".into());
                            return;
                        }
                    };
                    let command = if let Some(motion) = motion {
                        match brake.command(
                            raw_dx,
                            t.y,
                            t.width,
                            t.height,
                            desired,
                            smooth_type,
                            eff_smoothing,
                            bezier.horizontal_control_ratio(dx_r, eff_smoothing),
                            grab_w,
                            presented_at,
                            Instant::now(),
                            motion,
                        ) {
                            Ok(command) => command,
                            Err(message) => {
                                if let Ok(mut mstate) = mouse_state.lock() {
                                    mstate.cancel_movement();
                                    mstate.shoot_requested = false;
                                }
                                report_runtime_error(&shared_state, &event_proxy, message);
                                return;
                            }
                        }
                    } else {
                        // A missing serial write must not be treated as a measured
                        // response to the command on the following image.
                        brake.reset_tracking();
                        0
                    };
                    bezier.limit_horizontal_velocity(command);
                    if let Ok(mut mstate) = mouse_state.lock() {
                        // Replace even a zero correction; obsolete commands must
                        // not continue after the current image reaches center.
                        mstate.cancel_movement();
                        if mstate.arduino_ready && command != 0 {
                            mstate.enqueue_move(QueuedMouseMove {
                                remaining_x: f64::from(command),
                                remaining_y: 0.0,
                                remaining_ticks: ticks_per_frame,
                                expires_at: Some(presented_at + MAX_FRAME_AGE),
                            });
                        }
                    } else {
                        report_runtime_error(&shared_state, &event_proxy,
                            "Predictive braking stopped: mouse sender state is invalid. Restart and check verux_apb.log.".into());
                        return;
                    }
                    command
                } else {
                    smooth_delta(
                        smooth_type,
                        dx_r,
                        dy_r,
                        eff_smoothing,
                        grab_w,
                        grab_h,
                        &mut bezier,
                        now,
                    )
                    .0
                };

                // Send move packet to mouse sender thread
                if aim_dx != 0 && !config.predictive_braking {
                    if let Ok(mut mstate) = mouse_state.lock() {
                        if mstate.arduino_ready {
                            mstate.enqueue_move(QueuedMouseMove {
                                remaining_x: aim_dx as f64,
                                remaining_y: 0.0,
                                remaining_ticks: ticks_per_frame,
                                expires_at: None,
                            });
                        }
                    }
                }
                last_tracking_dx = aim_dx;
            }
        }

        if aim_active && !target_seen {
            let (decay_dx, _, continuing) = decay.delta(motion_now);
            if continuing {
                if let Some(deadline) = decay_deadline.filter(|end| Instant::now() < *end) {
                    if let Ok(mut mstate) = mouse_state.lock() {
                        if mstate.arduino_ready && decay_dx != 0 {
                            // Replace each old tail segment with the current decayed
                            // output; late segments never accumulate in the queue.
                            mstate.enqueue_move(QueuedMouseMove {
                                remaining_x: f64::from(decay_dx),
                                remaining_y: 0.0,
                                remaining_ticks: ticks_per_frame,
                                expires_at: Some(deadline),
                            });
                        }
                    }
                } else {
                    decay.disarm();
                    decay_deadline = None;
                }
            } else {
                decay_deadline = None;
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
                    true,
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
