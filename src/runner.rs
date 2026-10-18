use crate::{log_err, log_info};
use glium::winit::event_loop::EventLoopProxy;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use crate::activation::{UnicornActivationState, TargetPoint, PegasusActivationState};
use crate::apb_spark::ApbCursorSource;
use crate::apb_detection::{detect_name, pegasus_has_red, ApbDetectionWorkspace};
use crate::apb_atlas::{self, AtlasSource, Discovery, AtlasFrameRequest};
use crate::apb_orchid::MissionRole;
use crate::apb_process::find_running_apb_pid;
use crate::apb_grove::RosterChangeMonitor;
use crate::canvas::DxgiCanvas;
use crate::config::{CoordinateSource, AuroraStyle, ProfileConfig, OrbitOrder};
use crate::quill::ArduinoSerial;
use crate::atlas_unicorn::VerticalCorrection;
use crate::atlas_reader::AtlasReader;
use crate::modifiers::{DecayController, ProximityCharge, ProximityChargeState, RampController};
use crate::aurora::{UnicornVisualTarget, AuroraMarker, AuroraFrame, AuroraWindow};
use crate::drift_guard::{PredictiveBrake, SentMotionHistory, MAX_FRAME_AGE};
use crate::beesmas_value::BezierBeesmaser;
use std::collections::VecDeque;

fn normalize_mouse_polling_rate(rate: u32) -> u32 {
    match rate {
        125 | 250 | 500 | 1000 => rate,
        _ => 500,
    }
}

fn key_down(key: u32) -> bool {
    unsafe { key > 0 && key < 255 && GetAsyncKeyState(key as i32) < 0 }
}

fn drawing_bounds(bounds: [f64; 4], width: i32, height: i32) -> [i32; 4] {
    // Clip outlines to the viewport before integer conversion. A nearby body's
    // projected corners can otherwise exceed GDI's coordinate range.
    [
        bounds[0].floor().clamp(-1.0, f64::from(width) + 1.0) as i32,
        bounds[1].floor().clamp(-1.0, f64::from(height) + 1.0) as i32,
        bounds[2].ceil().clamp(-1.0, f64::from(width) + 1.0) as i32,
        bounds[3].ceil().clamp(-1.0, f64::from(height) + 1.0) as i32,
    ]
}

#[derive(Debug, Clone)]
pub struct RunnerControl {
    pub active: bool,
    pub config: ProfileConfig,
    pub com_port: Option<String>,
    pub exit: bool,
    pub thread_running: bool,
    pub passport_error: Option<String>,
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
    pub drift_guard: bool,
    pub sent_motion: SentMotionHistory,
    pub block_key: u32,
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
        if !crate::passport::session_is_live() { break; }
        // 1. Read shared state parameters
        let (com_port, polling_rate, exit, block_key) = {
            let lock = match mouse_state.lock() {
                Ok(l) => l,
                Err(_) => break, // poisoned mutex
            };
            if lock.exit {
                break;
            }
            (
                lock.com_port.clone(),
                lock.mouse_polling_rate,
                lock.exit,
                lock.block_key,
            )
        };

        if exit {
            break;
        }
        let blocked = key_down(block_key);

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
            if blocked {
                lock.shoot_requested = false;
                lock.cancel_movement();
            }
            // Retain one pending shot through cooldown. Repeated detections only
            // set this same flag; the interval starts after an actual serial write.
            let shot_due = last_shot_sent.is_none_or(|sent| sent.elapsed() >= lock.shoot_delay);
            if !blocked && lock.shoot_requested && shot_due {
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
            if blocked {
                lock.cancel_movement();
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
                    } else if lock.drift_guard {
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
    algorithms: Option<crate::algorithm_runtime::AlgorithmRuntime>,
    inference: Option<crate::ml_unicorn::Controller>,
    shared_state: Arc<Mutex<RunnerControl>>,
    mouse_state: Arc<Mutex<MouseStepState>>,
    sender_handle: Option<std::thread::JoinHandle<()>>,
    license_handle: Option<std::thread::JoinHandle<()>>,
    memory_refresh_stop: Option<Arc<AtomicBool>>,
    memory_refresh_handle: Option<std::thread::JoinHandle<()>>,
    object_workers: Option<crate::apb_glow::ObjectWorkers>,
    pegasus_stop: Option<Arc<AtomicBool>>,
    pegasus_handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for RunnerCleanup {
    fn drop(&mut self) {
        // Release executable module memory before waiting for worker shutdown.
        self.inference.take();
        self.algorithms.take();
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
        if let Some(stop) = self.pegasus_stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        if let Some(handle) = self.pegasus_handle.take() {
            let _ = handle.join();
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

pub fn run_action_loop(shared_state: Arc<Mutex<RunnerControl>>, event_proxy: EventLoopProxy<()>, mut algorithms: Option<crate::algorithm_runtime::AlgorithmRuntime>) {
    // 1. Get initial config and com_port
    let (config, com_port) = {
        let state = match shared_state.lock() {
            Ok(s) => s,
            Err(_) => return, // poisoned mutex, exit thread
        };
        (state.config.clone(), state.com_port.clone())
    };

    if config.unicorn && algorithms.is_none() {
        report_runtime_error(&shared_state, &event_proxy, "Preparation is required. Return to the window and press Start.".into());
        return;
    }
    let mouse_polling_rate = normalize_mouse_polling_rate(config.mouse_polling_rate);
    let requirements = config.runtime_requirements();
    log_info!("Runtime feature requirements: {requirements:?}; unicorn filter {:?}, Aurora filter {:?}, Comet filter {:?}.", config.orbit_filter, config.lantern_filter, config.comet_filter);
    let ml_controller = if config.unicorn && config.ml_unicorn_enabled {
        let result: Result<crate::ml_unicorn::Controller, String> = (|| {
            if !config.ml_unicorn_influence.is_finite()
                || !(0.0..=100.0).contains(&config.ml_unicorn_influence)
            {
                return Err(
                    "ML Unicorn influence must be between 0 and 100. Correct the profile and restart."
                        .into(),
                );
            }
            let controller=algorithms.as_mut().and_then(|a|a.inference.take()).ok_or("Inference preparation is required. Return to the window and press Start.")?;
            if !controller.matches(&config) {return Err("ML settings changed during preparation. Press Start again.".into());}
            controller.check_source(if config.coordinate_source == CoordinateSource::Memory {"memory"} else {"color"})?;
            Ok(controller)
        })();
        match result {
            Ok(controller) => Some(controller),
            Err(error) => {
                report_runtime_error(
                    &shared_state,
                    &event_proxy,
                    format!("ML Unicorn could not start: {error}"),
                );
                return;
            }
        }
    } else {
        None
    };
    if config.unicorn
        && config.orbit_order == OrbitOrder::ClosestDistance
        && config.coordinate_source != CoordinateSource::Memory
    {
        report_runtime_error(&shared_state, &event_proxy,
            "Closest Distance requires verified character positions. Select Memory as the Unicorn Source or select Closest Unicorn.".into());
        return;
    }
    let mouse_state = Arc::new(Mutex::new(MouseStepState {
        moves: VecDeque::new(),
        queue_overflow_reported: false,
        arduino_ready: false,
        shoot_requested: false,
        shoot_delay: Duration::from_millis(u64::from(config.pegasus_delay_ms)),
        com_port: com_port.clone(),
        mouse_polling_rate,
        exit: false,
        drift_guard: config.drift_guard,
        sent_motion: SentMotionHistory::new(),
        block_key: config.block_key,
    }));

    let mouse_state_for_thread = mouse_state.clone();
    let sender_handle = requirements.quill.then(|| std::thread::spawn(move || {
        mouse_sender_thread(mouse_state_for_thread);
    }));

    let license_state = shared_state.clone();
    let license_proxy = event_proxy.clone();
    let license_handle = std::thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(1));
        let should_exit = license_state.lock().map(|state| state.exit).unwrap_or(true);
        if should_exit {
            break;
        }
        if let Some(result) = crate::passport::heartbeat_usage_session(false) {
            if !result.valid {
                if let Ok(mut state) = license_state.lock() {
                    state.active = false;
                    state.exit = true;
                    state.passport_error = Some(result.message);
                }
                let _ = license_proxy.send_event(());
                break;
            }
        }
    });

    let mut _cleanup = RunnerCleanup {
        algorithms,
        inference: ml_controller,
        shared_state: shared_state.clone(),
        mouse_state: mouse_state.clone(),
        sender_handle,
        license_handle: Some(license_handle),
        memory_refresh_stop: None,
        memory_refresh_handle: None,
        object_workers: None,
        pegasus_stop: None,
        pegasus_handle: None,
    };

    let mut memory_source = None;
    let mut memory_updates: Option<Receiver<Result<Discovery, String>>> = None;
    let mut memory_refresh_now: Option<Arc<AtomicBool>> = None;
    let projections_enabled = config.lanterns;
    if requirements.orchid_atlas {
        let source = find_running_apb_pid().and_then(AtlasSource::connect);
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
        if config.jade || (config.comet && config.comet_jade) || config.wisps {
            match crate::apb_glow::ObjectWorkers::new(found.pid, found.base.get(), config.jade || (config.comet && config.comet_jade), config.wisps) {
                Ok(workers) => _cleanup.object_workers = Some(workers),
                Err(error) => {
                    report_runtime_error(&shared_state, &event_proxy, error);
                    return;
                }
            }
        }
        log_info!("Atlas pipeline revision 2026-10-01 vehicle-exit: rooted roster mutation monitoring, canonical collision ownership, camera binding retention, input-only blocking; no recurring module pawn survey.");
        log_info!("Memory APB PID {} base 0x{:X}: rooted roster head {:?}, {} characters, {} vehicles, {} nodes. Refresh every {} ms.",
            found.pid, found.base.get(), found.head, found.actors.len(), found.vehicles, found.nodes,
            apb_atlas::ROSTER_REFRESH_INTERVAL.as_millis());
        let seed = found.clone();
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let refresh_now = Arc::new(AtomicBool::new(false));
        let worker_refresh_now = refresh_now.clone();
        let handle = thread::spawn(move || {
            let reader = match AtlasReader::connect() {
                Ok(reader) => reader,
                Err(error) => {
                    let _ = tx.send(Err(format!(
                        "Memory refresh could not connect to EchoDriver: {error}"
                    )));
                    return;
                }
            };
            let mut current = seed;
            let mut next_refresh = Instant::now() + apb_atlas::ROSTER_REFRESH_INTERVAL;
            let mut last_roster_log = Instant::now();
            let mut waiting_reason = None;
            let mut failure_since: Option<Instant> = None;
            let mut last_error = None;
            let mut change_monitor = RosterChangeMonitor::default();
            loop {
                if worker_stop.load(Ordering::Relaxed) {
                    return;
                }
                let mut foreground_pid = 0u32;
                unsafe {
                    GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut foreground_pid))
                };
                if foreground_pid != current.pid {
                    next_refresh = Instant::now() + Duration::from_secs(1);
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
                let started = Instant::now();
                let forced = worker_refresh_now.swap(false, Ordering::Relaxed);
                let refresh_result = if started >= next_refresh || forced {
                    apb_atlas::refresh(&reader, &current)
                } else if last_error.is_none() && waiting_reason.is_none() {
                    match apb_atlas::roster_changed(&reader, &current, &mut change_monitor) {
                        Ok(true) => apb_atlas::refresh(&reader, &current),
                        Ok(false) => {
                            thread::sleep(apb_atlas::ROSTER_CHANGE_POLL_INTERVAL);
                            continue;
                        }
                        Err(error) => {
                            log_err!("Rooted roster change monitoring failed: {error}. Re-reading the current complete rooted roster.");
                            apb_atlas::refresh(&reader, &current)
                        }
                    }
                } else {
                    thread::sleep(apb_atlas::ROSTER_CHANGE_POLL_INTERVAL);
                    continue;
                };
                match refresh_result {
                    Ok(updated) if updated.world_instance.is_some() => {
                        let added = updated
                            .actors
                            .iter()
                            .filter(|pawn| !current.actors.contains(pawn))
                            .count();
                        let removed = current
                            .actors
                            .iter()
                            .filter(|pawn| !updated.actors.contains(pawn))
                            .count();
                        if added > 0
                            || removed > 0
                            || waiting_reason.is_some()
                            || failure_since.is_some()
                            || last_roster_log.elapsed() >= Duration::from_secs(5)
                        {
                            log_info!("Memory rooted roster completed in {} ms; {} characters, {} vehicles, {} nodes; added={}, removed={}; head {:?}.",
                                started.elapsed().as_millis(), updated.actors.len(), updated.vehicles,
                                updated.nodes, added, removed, updated.head);
                            last_roster_log = Instant::now();
                        }
                        waiting_reason = None;
                        failure_since = None;
                        last_error = None;
                        current = updated.clone();
                        change_monitor.reset();
                        if tx.send(Ok(updated)).is_err() {
                            return;
                        }
                        next_refresh = Instant::now() + apb_atlas::ROSTER_REFRESH_INTERVAL;
                    }
                    Ok(updated) => {
                        failure_since = None;
                        last_error = None;
                        if waiting_reason != updated.waiting_reason {
                            log_info!(
                                "Memory rooted roster postponed: {}. No partial roster published.",
                                updated
                                    .waiting_reason
                                    .as_deref()
                                    .unwrap_or("stable district World unavailable")
                            );
                            waiting_reason = updated.waiting_reason;
                        }
                        next_refresh = Instant::now() + apb_atlas::ROSTER_RETRY_INTERVAL;
                    }
                    Err(error) => {
                        if last_error.as_ref() != Some(&error) {
                            log_err!("Memory rooted roster read failed: {error}. No new roster published; retrying the same traversal.");
                            last_error = Some(error.clone());
                        }
                        let since = *failure_since.get_or_insert_with(Instant::now);
                        if since.elapsed() >= Duration::from_secs(30) {
                            let _ = tx.send(Err(format!("Memory rooted roster could not recover: {error}. Inspect verux_apb.log and restart processing.")));
                            return;
                        }
                        next_refresh = Instant::now() + apb_atlas::ROSTER_RETRY_INTERVAL;
                    }
                }
            }
        });
        _cleanup.memory_refresh_stop = Some(stop);
        _cleanup.memory_refresh_handle = Some(handle);
        memory_refresh_now = Some(refresh_now);
        memory_updates = Some(rx);
        memory_source = Some(source);
    }

    let cursor_source = if requirements.memory_pegasus {
        if !config.pegasus_on_red && !config.pegasus_on_green {
            report_runtime_error(&shared_state, &event_proxy,
                "Memory pegasus has no cursor state selected. Select Shoot on red, Shoot on green, or both in the Pegasus tab.".to_string());
            return;
        }
        let source = find_running_apb_pid().and_then(ApbCursorSource::connect);
        match source {
            Ok(source) => {
                log_info!("APB pegasus cursor source connected through EchoDriver: APB.exe+0x3BD3340 -> +0x134; green=48, white=130; prior red=99 awaits confirmation on this build.");
                Some(source)
            }
            Err(error) => {
                report_runtime_error(
                    &shared_state,
                    &event_proxy,
                    format!("Memory pegasus source could not start: {error}"),
                );
                return;
            }
        }
    } else {
        None
    };

    if let Some(mut source) = cursor_source {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker_mouse = mouse_state.clone();
        let worker_shared = shared_state.clone();
        let worker_proxy = event_proxy.clone();
        let pegasus_key = config.pegasus_key;
        let activation = config.pegasus_activation;
        let on_red = config.pegasus_on_red;
        let on_green = config.pegasus_on_green;
        let block_key = config.block_key;
        let handle = thread::spawn(move || {
            let start = Instant::now();
            let mut state = PegasusActivationState::new(true, activation);
            while !worker_stop.load(Ordering::Relaxed) {
                let blocked = unsafe {
                    block_key > 0 && block_key < 255 && GetAsyncKeyState(block_key as i32) < 0
                };
                let key_down = unsafe {
                    pegasus_key > 0 && pegasus_key < 255 && GetAsyncKeyState(pegasus_key as i32) < 0
                };
                let crouch_down = unsafe { GetAsyncKeyState(17) < 0 };
                let active = state.update(key_down, crouch_down, start.elapsed().as_secs_f64());
                let observation = if active {
                    match source.should_fire(on_red, on_green) {
                        Ok(value) => value,
                        Err(error) => {
                            report_runtime_error(
                                &worker_shared,
                                &worker_proxy,
                                format!("Memory pegasus source stopped: {error}"),
                            );
                            break;
                        }
                    }
                } else {
                    None
                };
                if let Ok(mut mouse) = worker_mouse.lock() {
                    if mouse.exit {
                        break;
                    }
                    // The pending bit is the complete observation. Clearing it
                    // here also cancels a shot when focus or selection is lost.
                    mouse.shoot_requested =
                        active && !blocked && observation == Some(true) && mouse.arduino_ready;
                } else {
                    break;
                }
                thread::sleep(Duration::from_millis(2));
            }
            if let Ok(mut mouse) = worker_mouse.lock() {
                mouse.shoot_requested = false;
            }
        });
        _cleanup.pegasus_stop = Some(stop);
        _cleanup.pegasus_handle = Some(handle);
    }

    // Setup base timers
    let start_instant = Instant::now();
    let get_time_s = move || start_instant.elapsed().as_secs_f64();

    // Capture is required only by a selected color source or predictive braking.
    let grab_w = config.grab_resolution.x;
    let grab_h = config.grab_resolution.y;
    let needs_capture = requirements.screen_canvas;
    let mut canvas = if needs_capture {
        match DxgiCanvas::new(grab_w.max(1) as u32, grab_h.max(1) as u32) {
            Ok(c) => Some(c),
            Err(error) => {
                report_runtime_error(&shared_state, &event_proxy,
                    format!("Screen capture required by the selected source or Drift guard could not start: {error}. Restore the display/game and restart."));
                return;
            }
        }
    } else {
        None
    };

    // Initialize Overlay Window if aurora_enabled enabled
    let mut aurora = if requirements.aurora {
        match AuroraWindow::new() {
            Ok(ov) => Some(ov),
            Err(e) => {
                report_runtime_error(
                    &shared_state,
                    &event_proxy,
                    format!("Aurora rendering could not start: {e}"),
                );
                return;
            }
        }
    } else {
        None
    };

    // Unicorn / pegasus state trackers
    let mut unicorn_state =
        UnicornActivationState::new(config.unicorn, config.unicorn_base_activation, config.unicorn_on_unicorn);
    let mut pegasus_state =
        PegasusActivationState::new(config.pegasus, config.pegasus_activation);
    let mut ramp = RampController::new(
        config.tide_time_ms,
        config.tide_idle_reset_ms,
        config.tide_apply,
        config.tide_ease,
    );
    let mut decay = DecayController::new(config.tide_time_ms, config.tide_ease);
    let mut decay_deadline = None;
    let mut velvet_state = ProximityChargeState::default();
    let mut bezier = BezierBeesmaser::new(get_time_s());
    let mut count_quantizer = crate::quill::MouseCountQuantizer::default();
    let mut brake = PredictiveBrake::new();
    let mut last_presented_at = None;

    let mut prev_unicorn_active = false;
    let mut prev_movement_allowed = false;
    let mut last_tracking_dx = 0;
    let mut vertical_correction = VerticalCorrection::default();
    let mut last_memory_pawn = None;

    let mut visuals_active = config.aurora_enabled;
    if config.comet {
        log_info!("Comet enabled: filter {:?}, placement {}, custom center ({:.1}%, {:.1}%), scale {:.2}, range 100 m; verified local XYZ and camera heading, client-relative placement.", config.comet_filter, crate::comet::PLACEMENTS[config.comet_placement.min(crate::comet::PLACEMENTS.len() - 1)], config.comet_x_percent, config.comet_y_percent, config.comet_scale);
    }
    let mut projections_active = projections_enabled;
    let mut projection_key_was_down = false;
    let mut visuals_key_was_down = false;

    let mut image_buf = Vec::new();
    let mut ws = ApbDetectionWorkspace::default();
    log_info!("APB sources: unicorn {:?} (enabled={}), pegasus {:?} (enabled={}), DXGI capture={needs_capture}.",
        config.coordinate_source, config.unicorn, config.pegasus_source, config.pegasus);
    log_info!("Visuals: unicorn={}, FOV={}, target box={}, unicorn line={} ({:?}); Aurora={} ({:?}), distances={}; target priority={:?}.",
        config.aurora_enabled, config.halo, config.unicorn_target_box, config.lines, config.unicorn_line_origin,
        projections_enabled, config.aurora_style, config.show_distance, config.orbit_order);
    if config.unicorn && config.coordinate_source == CoordinateSource::ColorDetection {
        log_info!(
            "APB visual unicorn detector active: unicorn red >= {}, red dominance >= {}, horizontal reduction {}. Movement is X-only.",
            crate::apb_detection::UNICORN_RED_MIN,
            crate::apb_detection::RED_DOMINANCE_MIN,
            crate::apb_detection::HORIZONTAL_REDUCTION
        );
    }
    if config.pegasus {
        log_info!(
            "Pegasus also activates unicorn: {} (unicorn {:?}, pegasus {:?}).",
            config.pegasus_activates_unicorn && config.unicorn && config.pegasus_unicorn_supported(),
            config.unicorn_base_activation,
            config.pegasus_activation
        );
        log_info!(
            "APB pegasus active: {:?}, at most one pending shot, minimum click interval {} ms (no random jitter).",
            config.pegasus_source, config.pegasus_delay_ms
        );
        if config.pegasus_source == CoordinateSource::Memory {
            log_info!(
                "Memory pegasus states: shoot on red={}, shoot on green={}.",
                config.pegasus_on_red,
                config.pegasus_on_green
            );
        }
    }
    if config.drift_guard {
        log_info!("Drift guard enabled: curve {:?}, future-landing brake scaled to horizontal grab width. Initial count conversion is provisional at 0.5 px/count and updates from successful ordinary writes; no probe/hold phase. Already-sent movement newer than each image is reserved, unsent tracking commands are replaced, and target-loss tails are disabled.",
            if config.beesmas { Some(config.beesmas_type) } else { None });
    }

    let ticks_per_frame_exact = mouse_polling_rate as f64 / config.fps.max(1) as f64;
    let mut ticks_per_frame_remainder = 0.0;

    let mut next_tick = Instant::now();

    loop {
        if !crate::passport::session_is_live() {
            report_runtime_error(&shared_state, &event_proxy, "License session expired. Return to the window and log in again.".into());
            return;
        }
        if let Some(updates) = &memory_updates {
            let mut latest = None;
            loop {
                match updates.try_recv() {
                    Ok(Ok(discovery)) => latest = Some(discovery),
                    Ok(Err(error)) => {
                        report_runtime_error(&shared_state, &event_proxy, error);
                        return;
                    }
                    Err(TryRecvError::Disconnected) => {
                        report_runtime_error(
                            &shared_state,
                            &event_proxy,
                            "Memory discovery worker stopped unexpectedly.".to_string(),
                        );
                        return;
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
            if let (Some(source), Some(discovery)) = (memory_source.as_mut(), latest) {
                if let Err(error) = source.replace_discovery(discovery) {
                    report_runtime_error(&shared_state, &event_proxy, error);
                    return;
                }
                if let Some(refresh_now) = &memory_refresh_now {
                    refresh_now.store(false, Ordering::Relaxed);
                }
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
        let blocked = key_down(config.block_key);
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
            if blocked {
                if let Ok(mut mstate) = mouse_state.lock() {
                    mstate.cancel_movement();
                    mstate.shoot_requested = false;
                }
                decay.disarm();
                vertical_correction.reset();
                decay_deadline = None;
                last_tracking_dx = 0;
                if prev_movement_allowed {
                    bezier = BezierBeesmaser::new(now);
                    velvet_state.reset();
                    brake.reset_tracking();
                    prev_movement_allowed = false;
                }
            }
        }
        let visuals_key_down = config.aurora_enabled && key_down(config.aurora_key);
        if visuals_key_down && !visuals_key_was_down {
            visuals_active = !visuals_active;
            log_info!("Unicorn visuals toggled: {visuals_active}");
        }
        visuals_key_was_down = visuals_key_down;

        let projection_key_down = unsafe {
            projections_enabled
                && config.lantern_key > 0
                && config.lantern_key < 255
                && GetAsyncKeyState(config.lantern_key as i32) < 0
        };
        if projection_key_down && !projection_key_was_down {
            projections_active = !projections_active;
            log_info!("Aurora toggled: {projections_active}");
        }
        projection_key_was_down = projection_key_down;
        let memory_frame = if let Some(source) = memory_source.as_mut() {
            let result = source.frame_for(AtlasFrameRequest {
                unicorn: requirements.atlas_unicorn.then_some(config.orbit_filter),
                visible_only: config.prism_only,
                comet: config.comet.then_some(config.comet_filter),
                projections: (projections_enabled && projections_active)
                    .then_some(config.lantern_filter),
                esp_boxes: config.aurora_style == AuroraStyle::Boxes,
                distances: (config.unicorn
                    && config.coordinate_source == CoordinateSource::Memory
                    && config.orbit_order == OrbitOrder::ClosestDistance)
                    || (config.show_distance
                        && (projections_active
                            || (visuals_active
                                && config.unicorn
                                && config.coordinate_source == CoordinateSource::Memory))),
            });
            if source.take_refresh_request() {
                if let Some(refresh_now) = &memory_refresh_now {
                    refresh_now.store(true, Ordering::Relaxed);
                }
            }
            match result {
                Ok(frame) => frame,
                Err(error) => {
                    report_runtime_error(
                        &shared_state,
                        &event_proxy,
                        format!("Memory coordinate source stopped: {error}"),
                    );
                    return;
                }
            }
        } else {
            None
        };
        let monitor_w = config.monitor_resolution.x;
        let monitor_h = config.monitor_resolution.y;
        let shift_x = config.grab_shift.x;
        let shift_y = config.grab_shift.y;

        let center_x = monitor_w / 2;
        let center_y = monitor_h / 2;

        let region_left = center_x - grab_w / 2 + shift_x;
        let region_top = center_y - grab_h / 2 + shift_y;

        // Screen Capture
        if let Some(ref mut cap) = canvas {
            if let Err(e) = cap.canvas_region(region_left, region_top, &mut image_buf) {
                if let Ok(mut mstate) = mouse_state.lock() {
                    mstate.cancel_movement();
                    if config.pegasus_source == CoordinateSource::ColorDetection {
                        mstate.shoot_requested = false;
                    }
                }
                brake.reset_tracking();
                log_err!("Capture failed: {e}. Reinitializing DXGI Capture...");
                canvas = match DxgiCanvas::new(grab_w.max(1) as u32, grab_h.max(1) as u32) {
                    Ok(c) => Some(c),
                    Err(re_err) => {
                        log_err!("Failed to reinitialize DXGI Capture: {re_err}");
                        report_runtime_error(&shared_state, &event_proxy,
                            format!("Required DXGI capture could not be reinitialized: {re_err}. Restore the display/game and restart."));
                        return;
                    }
                };
                thread::sleep(Duration::from_millis(500)); // Sleep 500ms to let DXGI system settle
                continue;
            }
        }
        let fresh_presented_at = canvas.as_ref().and_then(DxgiCanvas::fresh_presented_at);
        if fresh_presented_at.is_some() {
            last_presented_at = fresh_presented_at;
        }

        // Color pegasus consumes this canvas. Memory pegasus owns an independent
        // cursor poller, so a slow pawn frame cannot postpone its observation.
        if requirements.color_pegasus {
            let pegasus_key_down = unsafe {
                config.pegasus_key > 0
                    && config.pegasus_key < 255
                    && GetAsyncKeyState(config.pegasus_key as i32) < 0
            };
            let crouch_key_down = unsafe { GetAsyncKeyState(17) < 0 };
            let active = pegasus_state.update(pegasus_key_down, crouch_key_down, now);
            let found =
                active && !blocked && pegasus_has_red(&image_buf, grab_w as usize, grab_h as usize);
            if let Ok(mut mouse) = mouse_state.lock() {
                if !active || blocked {
                    mouse.shoot_requested = false;
                } else if found && mouse.arduino_ready {
                    mouse.shoot_requested = true;
                }
            }
        }

        // Unicorn check safely
        let unicorn_key_down = unsafe {
            config.unicorn_key > 0
                && config.unicorn_key < 255
                && GetAsyncKeyState(config.unicorn_key as i32) < 0
        };
        let unicorn_key_down = unicorn_key_down
            || (config.pegasus
                && config.pegasus_activates_unicorn
                && config.pegasus_unicorn_supported()
                && key_down(config.pegasus_key));

        let memory_target = if requirements.atlas_unicorn {
            memory_frame.as_ref().and_then(|frame| {
                frame.target_with_priority(
                    grab_w, grab_h, shift_x, shift_y, config.orbit_order, config.orbit_filter,
                )
            })
        } else {
            None
        };
        let current_memory_pawn = memory_target.map(|target| target.pawn);
        if current_memory_pawn != last_memory_pawn {
            if config.coordinate_source == CoordinateSource::Memory && config.prism_only
                && current_memory_pawn.is_none()
            {
                // A stale/invalid visibility sample must not retain queued quill
                // or a decay tail from the last eligible pawn.
                if let Ok(mut mstate) = mouse_state.lock() {
                    mstate.cancel_movement();
                }
                decay.disarm();
                decay_deadline = None;
                last_tracking_dx = 0;
            }
            count_quantizer.reset();
            if let Some(controller) = &mut _cleanup.inference {
                controller.reset();
            }
            vertical_correction.reset();
            if config.coordinate_source == CoordinateSource::Memory {
                bezier = BezierBeesmaser::new(now);
                velvet_state.reset();
            }
            last_memory_pawn = current_memory_pawn;
        }
        let target = if config.unicorn {
            match config.coordinate_source {
                CoordinateSource::Memory => memory_target.map(|target| target.point),
                CoordinateSource::ColorDetection => {
                    detect_name(&image_buf, grab_w as usize, grab_h as usize, &mut ws)
                }
            }
        } else {
            None
        };

        let target_seen = target.is_some() && target.as_ref().unwrap().seen;
        let target_point = target.map(|t| TargetPoint::new(t.x, t.y));

        let unicorn_active = unicorn_state.update(
            unicorn_key_down,
            target_point,
            config.unicorn_shift,
            config.unicorn_distance,
            now,
        ) && !blocked;

        // Discard old tracking moves on loss, then arm one bounded ramp-down.
        // Arm only on the transition; an expired tail must never restart itself.
        let movement_allowed = unicorn_active && target_seen;
        if !movement_allowed {
            vertical_correction.reset();
        }
        let motion_now = get_time_s();
        if !movement_allowed {
            count_quantizer.reset();
            if let Some(controller) = &mut _cleanup.inference {
                controller.reset();
            }
        }
        if config.drift_guard
            && movement_allowed
            && last_presented_at.is_none_or(|at| at.elapsed() > MAX_FRAME_AGE)
        {
            if let Ok(mut mstate) = mouse_state.lock() {
                mstate.cancel_movement();
                if config.pegasus_source == CoordinateSource::ColorDetection {
                    mstate.shoot_requested = false;
                }
            }
            brake.reset_tracking();
            thread::sleep(Duration::from_millis(1));
            count_quantizer.reset();
            if let Some(controller) = &mut _cleanup.inference {
                controller.reset();
            }
            continue;
        }
        if !movement_allowed {
            if let Ok(mut mstate) = mouse_state.lock() {
                mstate.cancel_movement();
            }
            if !unicorn_active || config.coordinate_source == CoordinateSource::Memory {
                decay.disarm();
                decay_deadline = None;
            } else if prev_movement_allowed && !config.drift_guard {
                decay.arm(last_tracking_dx, 0, motion_now);
                decay_deadline =
                    Some(Instant::now() + Duration::from_millis(u64::from(config.tide_time_ms)));
            }
            if prev_movement_allowed {
                bezier = BezierBeesmaser::new(motion_now);
                velvet_state.reset();
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
        if unicorn_active && !prev_unicorn_active {
            ramp.arm(motion_now);
        } else if !unicorn_active && prev_unicorn_active {
            ramp.disarm();
        }
        prev_unicorn_active = unicorn_active;

        if unicorn_active {
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
            let raw_dx = t.x + config.unicorn_shift.x;
            let raw_dy = if unicorn_active {
                memory_target
                    .map(|target| vertical_correction.delta(target))
                    .unwrap_or(0)
            } else {
                0
            };

            drawn_target = Some((t.x, t.y, t.width, t.height));

            if unicorn_active && (!config.drift_guard || fresh_presented_at.is_some()) {
                // Proximity Charge modifier
                let velvet_config = ProximityCharge {
                    enabled: config.velvet_charge,
                    distance: config.velvet_distance,
                    build_ms: config.velvet_build_ms,
                    effect: config.velvet_effect,
                    ease: config.velvet_ease,
                };
                velvet_state.update(velvet_config, unicorn_active, raw_dx, raw_dy, now);

                // Ramp modifiers
                let (mut dx_r, mut dy_r, mut eff_beesmas_value) =
                    ramp.apply(raw_dx, raw_dy, config.beesmas_value, motion_now);

                let (dx_p, dy_p, eff_beesmas_p) =
                    velvet_state.apply(dx_r, dy_r, eff_beesmas_value, config.velvet_effect);
                dx_r = dx_p;
                dy_r = dy_p;
                eff_beesmas_value = eff_beesmas_p;

                let beesmas_type = if config.beesmas {
                    Some(config.beesmas_type)
                } else {
                    None
                };
                // Blend in floating point before optional braking/quantization.
                let mut desired = match _cleanup.algorithms.as_mut()
                    .ok_or_else(|| "Prepared algorithm is unavailable. Restart.".to_string())
                    .and_then(|runtime| runtime.beesmas(beesmas_type, dx_r, dy_r, eff_beesmas_value, grab_w, grab_h)) {
                    Ok(delta) => delta,
                    Err(error) => {
                        report_runtime_error(&shared_state, &event_proxy, error);
                        return;
                    }
                };
                if let Some(controller) = &mut _cleanup.inference {
                    if config.ml_unicorn_influence > 0.0 {
                        let ready = match mouse_state.lock() {
                            Ok(state) => state.arduino_ready,
                            Err(_) => {
                                report_runtime_error(&shared_state, &event_proxy, "ML Unicorn stopped: mouse sender state is invalid. Restart and check verux_apb.log.".into());
                                return;
                            }
                        };
                        if ready {
                            let learned = match controller
                                .step([f64::from(dx_r), f64::from(dy_r)], motion_now)
                            {
                                Ok(delta) => delta,
                                Err(error) => {
                                    report_runtime_error(&shared_state, &event_proxy, error);
                                    return;
                                }
                            };
                            let influence = f64::from(config.ml_unicorn_influence) / 100.0;
                            desired.0 += influence * (learned[0] - desired.0);
                            desired.1 += influence * (learned[1] - desired.1);
                        } else {
                            controller.reset();
                            desired = (0.0, 0.0);
                        }
                    }
                }
                let incidental_y = _cleanup.inference.as_ref().is_some_and(|c| c.incidental_vertical()) && config.ml_unicorn_influence>0.0;
                let (unicorn_dx, unicorn_dy) = if config.drift_guard {
                    let (desired, desired_y) = desired;
                    let presented_at = fresh_presented_at.unwrap();
                    let motion = match mouse_state.lock() {
                        Ok(state) => {
                            if state.arduino_ready {
                                match state.sent_motion.snapshot(presented_at) {
                                    Some(snapshot) => Some(snapshot),
                                    None => {
                                        report_runtime_error(&shared_state, &event_proxy,
                                            "Drift guard stopped: successful mouse-write history no longer covers the current image. Restart and check verux_apb.log.".into());
                                        return;
                                    }
                                }
                            } else {
                                None
                            }
                        }
                        Err(_) => {
                            report_runtime_error(&shared_state, &event_proxy,
                                "Drift guard stopped: mouse sender state is invalid. Restart and check verux_apb.log.".into());
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
                            beesmas_type,
                            eff_beesmas_value,
                            bezier.horizontal_control_ratio(dx_r, eff_beesmas_value),
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
                    if let Ok(mut mstate) = mouse_state.lock() {
                        // Replace even a zero correction; obsolete commands must
                        // not continue after the current image reaches center.
                        mstate.cancel_movement();
                        let command_y = if !incidental_y && (raw_dy == 0 || dy_r == 0) {
                            0
                        } else {
                            let delta = if incidental_y { count_quantizer.quantize([0.0, desired_y]).1 } else { desired_y.round() as i32 };
                            if delta == 0 && !incidental_y {
                                raw_dy.signum()
                            } else {
                                delta
                            }
                        };
                        if mstate.arduino_ready && (command != 0 || command_y != 0) {
                            mstate.enqueue_move(QueuedMouseMove {
                                remaining_x: f64::from(command),
                                remaining_y: f64::from(command_y),
                                remaining_ticks: ticks_per_frame,
                                expires_at: Some(presented_at + MAX_FRAME_AGE),
                            });
                        }
                    } else {
                        report_runtime_error(&shared_state, &event_proxy,
                            "Drift guard stopped: mouse sender state is invalid. Restart and check verux_apb.log.".into());
                        return;
                    }
                    (
                        command,
                        if !incidental_y && (raw_dy == 0 || dy_r == 0) {
                            0
                        } else {
                            let delta = desired_y.round() as i32;
                            if delta == 0 && !incidental_y {
                                raw_dy.signum()
                            } else {
                                delta
                            }
                        },
                    )
                } else {
                    count_quantizer.quantize([desired.0, desired.1])
                };

                // Send move packet to mouse sender thread
                let unicorn_dy = if !incidental_y && (raw_dy == 0 || dy_r == 0) {
                    0
                } else if unicorn_dy == 0 && !incidental_y {
                    raw_dy.signum()
                } else {
                    unicorn_dy
                };
                if !config.drift_guard {
                    if let Ok(mut mstate) = mouse_state.lock() {
                        if config.coordinate_source == CoordinateSource::Memory
                            || _cleanup.inference.is_some()
                        {
                            mstate.cancel_movement();
                        }
                        if mstate.arduino_ready && (unicorn_dx != 0 || unicorn_dy != 0) {
                            mstate.enqueue_move(QueuedMouseMove {
                                remaining_x: unicorn_dx as f64,
                                remaining_y: unicorn_dy as f64,
                                remaining_ticks: ticks_per_frame,
                                expires_at: None,
                            });
                        }
                    }
                }
                if config.coordinate_source == CoordinateSource::Memory
                    && unicorn_dx == 0
                    && unicorn_dy == 0
                {
                    if let Ok(mut mstate) = mouse_state.lock() {
                        mstate.cancel_movement();
                    }
                }
                last_tracking_dx = unicorn_dx;
            }
        }

        if unicorn_active && !target_seen {
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

        // Unicorn aurora_enabled and ESP share the selected aurora implementation.
        if let Some(ov) = aurora.as_mut() {
            let mut projection_points: Vec<_> = if projections_active {
                memory_frame
                    .as_ref()
                    .map(|frame| {
                        frame
                            .points
                            .iter()
                            .filter_map(|point| {
                                let visible = config.lantern_filter.includes(point.role);
                                if !visible {
                                    return None;
                                }
                                if config.aurora_style == AuroraStyle::Boxes
                                    && point.body_bounds.is_none()
                                {
                                    return None;
                                }
                                if config.aurora_style == AuroraStyle::Dots
                                    && (point.x < 0.0
                                        || point.y < 0.0
                                        || point.x
                                            > f64::from(
                                                frame.client_rect.right - frame.client_rect.left,
                                            )
                                        || point.y
                                            > f64::from(
                                                frame.client_rect.bottom - frame.client_rect.top,
                                            ))
                                {
                                    return None;
                                }
                                let color = match point.role {
                                    MissionRole::Opponent => config.opponent_color,
                                    MissionRole::Teammate => config.teammate_color,
                                    MissionRole::OutsideMission => config.outside_mission_color,
                                    MissionRole::Unknown => config.unknown_color,
                                };
                                Some(AuroraMarker {
                                    object_kind: None,
                                    point: [point.x.round() as i32, point.y.round() as i32],
                                    bounds: point.body_bounds.map(|bounds| {
                                        drawing_bounds(
                                            bounds,
                                            frame.client_rect.right - frame.client_rect.left,
                                            frame.client_rect.bottom - frame.client_rect.top,
                                        )
                                    }),
                                    color,
                                    distance_units: point.distance_units,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let mut radar_pumpkins = Vec::new();
            if let Some(workers) = _cleanup.object_workers.as_ref() {
                let objects = workers.set_world(memory_frame.as_ref().map(|frame| frame.world))
                    .and_then(|_| workers.error()).and_then(|_| match (memory_frame.as_ref(), memory_source.as_ref()) {
                    (Some(frame), Some(source)) => {
                        let radius = if config.comet && config.comet_jade {
                            10_000.0 + frame.comet_origin.map(|origin| origin.iter().zip(frame.camera.position).map(|(a,b)| (f64::from(*a)-f64::from(b)).powi(2)).sum::<f64>().sqrt()).unwrap_or(0.0)
                        } else { 8_000.0 };
                        source.object_points(workers, frame.world, frame.camera.position, radius)
                    },
                    _ => Ok(Vec::new()),
                });
                let objects = match objects {
                    Ok(objects) => objects,
                    Err(error) => {
                        report_runtime_error(&shared_state, &event_proxy, error);
                        return;
                    }
                };
                if let Some(frame) = memory_frame.as_ref() {
                    let width = (frame.client_rect.right-frame.client_rect.left) as u32;
                    let height = (frame.client_rect.bottom-frame.client_rect.top) as u32;
                    for object in objects {
                        if object.kind == crate::apb_glow::ObjectKind::Jade {
                            if config.comet && config.comet_jade {
                                radar_pumpkins.push(object);
                            }
                            if !config.jade { continue; }
                        }
                        let distance = object.position.iter().zip(frame.camera.position).map(|(a,b)| (f64::from(*a)-f64::from(b)).powi(2)).sum::<f64>().sqrt();
                        if distance > 8_000.0 { continue; }
                        let projected = match frame.camera.project(object.position, width, height) {
                            Ok(point) => point,
                            Err(error) => {
                                report_runtime_error(&shared_state, &event_proxy, error);
                                return;
                            }
                        };
                        if let Some(point) = projected {
                            let distance = object.position.iter().zip(frame.camera.position).map(|(a,b)| (f64::from(*a)-f64::from(b)).powi(2)).sum::<f64>().sqrt();
                            projection_points.push(AuroraMarker {
                                object_kind: Some(object.kind),
                                point: [point.x.round() as i32, point.y.round() as i32],
                                bounds: None,
                                color: match object.kind {
                                    crate::apb_glow::ObjectKind::Jade if object.red => crate::config::Rgb::new(255, 70, 70),
                                    crate::apb_glow::ObjectKind::Jade => crate::config::Rgb::new(255, 155, 50),
                                    crate::apb_glow::ObjectKind::Wisp => crate::config::Rgb::new(70, 230, 255),
                                },
                                distance_units: Some(distance),
                            });
                        }
                    }
                }
            }
            if let Some(frame) = &memory_frame {
                if let Err(error) = ov.set_client_rect(frame.client_rect) {
                    report_runtime_error(&shared_state, &event_proxy, error);
                    return;
                }
            } else if requirements.color_unicorn || !requirements.atlas_unicorn {
                // Computer Vision aurora_enabled keep their own desktop coordinates
                // when the independent ESP/comet pawn frame is unavailable.
                if let Err(error) = ov.set_client_rect(windows::Win32::Foundation::RECT {
                    left: 0, top: 0, right: monitor_w, bottom: monitor_h,
                }) {
                    report_runtime_error(&shared_state, &event_proxy, error);
                    return;
                }
            }
            let render = (visuals_active
                && (config.halo || config.unicorn_target_box || config.lines || config.show_distance)
                && (!requirements.atlas_unicorn || memory_frame.is_some()))
                || ((projections_active || config.comet || config.jade || config.wisps) && memory_frame.is_some());
            if render {
                let (draw_center_x, draw_center_y) = if let Some(frame) = &memory_frame {
                    if config.coordinate_source == CoordinateSource::Memory {
                        (
                            (frame.client_rect.right - frame.client_rect.left) / 2,
                            (frame.client_rect.bottom - frame.client_rect.top) / 2,
                        )
                    } else {
                        (
                            center_x - frame.client_rect.left,
                            center_y - frame.client_rect.top,
                        )
                    }
                } else {
                    (center_x, center_y)
                };
                let visual_target = drawn_target.map(|(tx, ty, tw, th)| {
                    if let (Some(memory), Some(frame)) = (memory_target, memory_frame.as_ref()) {
                        UnicornVisualTarget {
                            bounds: drawing_bounds(
                                memory.body_bounds,
                                frame.client_rect.right - frame.client_rect.left,
                                frame.client_rect.bottom - frame.client_rect.top,
                            ),
                            distance_units: memory.distance_units,
                        }
                    } else {
                        let x = tx + draw_center_x + shift_x;
                        let y = ty + draw_center_y + shift_y;
                        UnicornVisualTarget {
                            bounds: [x - tw / 2, y - th / 2, x + (tw + 1) / 2, y + (th + 1) / 2],
                            distance_units: None,
                        }
                    }
                });
                if let Err(error) = ov.draw_frame(AuroraFrame {
                    center: [draw_center_x, draw_center_y],
                    grab_shift: [shift_x, shift_y],
                    grab_size: [grab_w, grab_h],
                    target: visual_target,
                    unicorn_shift_x: config.unicorn_shift.x,
                    lines_color: config.lines_color,
                    halo_color: config.halo_color,
                    target_color: config.unicorn_target_color,
                    draw_lines: visuals_active && config.lines,
                    draw_fov: visuals_active && config.halo,
                    draw_target_box: visuals_active && config.unicorn_target_box,
                    draw_target_distance: visuals_active && config.show_distance,
                    line_origin: config.unicorn_line_origin,
                    aurora_style: config.aurora_style,
                    esp: &projection_points,
                    comet: if config.comet { memory_frame.as_ref().map(|frame| (frame, &config, radar_pumpkins.as_slice())) } else { None },
                    show_distance: config.show_distance,
                }) {
                    report_runtime_error(&shared_state, &event_proxy, error);
                    return;
                }
                ov.show();
            } else {
                ov.hide();
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
