use crate::config::{Rgb, UnicornLineOrigin};
use std::cell::Cell;
use std::thread;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ValidateRect;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetSystemMetrics,
    RegisterClassW, SetWindowPos, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW, HMENU,
    HWND_TOPMOST, MSG, SM_CXSCREEN, SM_CYSCREEN, SWP_NOACTIVATE, SW_HIDE, SW_SHOW, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

const WINDOW_CLASS_NAME: PCWSTR = w!("VeruxOverlayClass");

pub struct AuroraMarker {
    pub object_kind: Option<crate::apb_glow::ObjectKind>,
    pub point: [i32; 2],
    pub bounds: Option<[i32; 4]>,
    pub color: Rgb,
    pub distance_units: Option<f64>,
}

pub struct UnicornVisualTarget {
    pub bounds: [i32; 4],
    pub distance_units: Option<f64>,
}

pub struct AuroraFrame<'a> {
    pub config: &'a crate::config::ProfileConfig,
    pub seconds: f32,
    pub center: [i32; 2],
    pub grab_shift: [i32; 2],
    pub grab_size: [i32; 2],
    pub target: Option<UnicornVisualTarget>,
    pub unicorn_shift_x: i32,
    pub lines_color: Rgb,
    pub halo_color: Rgb,
    pub target_color: Rgb,
    pub draw_lines: bool,
    pub draw_fov: bool,
    pub draw_target_box: bool,
    pub draw_target_distance: bool,
    pub line_origin: UnicornLineOrigin,
    pub esp: &'a [AuroraMarker],
    pub comet: Option<(
        &'a crate::apb_atlas::AtlasFrame,
        &'a crate::config::ProfileConfig,
        &'a [crate::apb_glow::ObjectPoint],
    )>,
}

pub struct AuroraWindow {
    hwnd: HWND,
    width: i32,
    height: i32,
    placement: (i32, i32, i32, i32),
    visible: Cell<bool>,
    painter: crate::visual_painter::Painter,
}

impl AuroraWindow {
    pub fn new() -> Result<Self, String> {
        let (tx, rx) = std::sync::mpsc::channel();

        // Register class and create window in a separate thread so its message pump
        // doesn't block winit's main GUI loop.
        thread::spawn(move || unsafe {
            let instance = match windows::Win32::System::LibraryLoader::GetModuleHandleW(None) {
                Ok(module) => HINSTANCE(module.0),
                Err(error) => {
                    let _ = tx.send(Err(format!("Aurora module handle failed: {error}")));
                    return;
                }
            };

            let wnd_class = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: WINDOW_CLASS_NAME,
                ..Default::default()
            };

            RegisterClassW(&wnd_class);

            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);

            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                WINDOW_CLASS_NAME,
                w!("VeruxOverlay"),
                WS_POPUP,
                0,
                0,
                width,
                height,
                HWND(0),
                HMENU(0),
                instance,
                None,
            );

            if hwnd.0 == 0 {
                let _ = tx.send(Err("Failed to create Aurora window".to_string()));
                return;
            }

            let _ = tx.send(Ok((hwnd, width, height)));

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND(0), 0, 0).as_bool() {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });

        let (hwnd, width, height) = rx
            .recv()
            .map_err(|e| format!("Aurora thread failed: {}", e))??;

        let painter = match crate::visual_painter::Painter::new(width, height) {
            Ok(painter) => painter,
            Err(error) => {
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                        hwnd,
                        windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                        WPARAM(0),
                        LPARAM(0),
                    );
                }
                return Err(error);
            }
        };
        Ok(Self {
            hwnd,
            width,
            height,
            placement: (0, 0, width, height),
            visible: Cell::new(false),
            painter,
        })
    }

    pub fn show(&self) {
        if self.visible.replace(true) {
            return;
        }
        unsafe {
            ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    pub fn hide(&self) {
        if !self.visible.replace(false) {
            return;
        }
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn set_client_rect(&mut self, rect: RECT) -> Result<(), String> {
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            return Err("Aurora client rectangle is invalid.".into());
        }
        let placement = (rect.left, rect.top, width, height);
        if self.placement == placement {
            return Ok(());
        }
        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                rect.left,
                rect.top,
                width,
                height,
                SWP_NOACTIVATE,
            )
        }
        .map_err(|error| format!("Place existing visuals Aurora failed: {error}"))?;
        if self.width != width || self.height != height {
            self.painter = crate::visual_painter::Painter::new(width, height)?;
        }
        self.width = width;
        self.height = height;
        self.placement = placement;
        Ok(())
    }

    pub fn draw_frame(&mut self, frame: AuroraFrame<'_>) -> Result<(), String> {
        use crate::visual_style::*;
        let styles = &frame.config.visual_styles;
        self.painter.clear(frame.seconds);
        if frame.draw_fov {
            if frame.config.coordinate_source == crate::config::CoordinateSource::Memory {
                let radius = frame.config.memory_aim_fov.round() as i32;
                self.painter.shape(
                    [
                        frame.center[0] - radius,
                        frame.center[1] - radius,
                        frame.center[0] + radius,
                        frame.center[1] + radius,
                    ],
                    &styles[FOV],
                    frame.halo_color,
                    true,
                )?;
            } else {
                let left = frame.center[0] - frame.grab_size[0] / 2 + frame.grab_shift[0];
                let top = frame.center[1] - frame.grab_size[1] / 2 + frame.grab_shift[1];
                self.painter.shape(
                    [
                        left,
                        top,
                        left + frame.grab_size[0],
                        top + frame.grab_size[1],
                    ],
                    &styles[FOV],
                    frame.halo_color,
                    false,
                )?;
            }
        }
        if let Some((memory, config, pumpkins)) = frame.comet {
            crate::comet::draw(
                &mut self.painter,
                self.width,
                self.height,
                memory,
                config,
                pumpkins,
            )?;
        }
        for marker in frame.esp {
            let index = match marker.object_kind {
                Some(crate::apb_glow::ObjectKind::Jade) => PUMPKINS,
                Some(crate::apb_glow::ObjectKind::Wisp) => PROJECTILES,
                None => PLAYERS,
            };
            let style = &styles[index];
            if style.max_distance > 0.0
                && !marker.distance_units.is_some_and(|d| {
                    d / crate::apb_orchid::APB_UNITS_PER_METER <= f64::from(style.max_distance)
                })
            {
                continue;
            }
            let circle = style.shape == 1;
            let bounds = if marker.object_kind.is_some() || circle {
                let r = style.marker_size;
                [
                    marker.point[0] - r,
                    marker.point[1] - r,
                    marker.point[0] + r + 1,
                    marker.point[1] + r + 1,
                ]
            } else {
                marker
                    .bounds
                    .ok_or("Player box supplied without verified body bounds.")?
            };
            self.painter.shape(bounds, style, marker.color, circle)?;
            if style.show_distance {
                if let Some(distance) = marker.distance_units {
                    self.painter
                        .distance(bounds, distance, style, marker.color)?;
                }
            }
        }
        if let Some(target) = frame.target {
            if frame.draw_target_box {
                let style = &styles[TARGET];
                self.painter
                    .shape(target.bounds, style, frame.target_color, style.shape == 1)?;
            }
            if frame.draw_lines {
                let style = &styles[LINES];
                let origin = match frame.line_origin {
                    UnicornLineOrigin::Center => frame.center,
                    UnicornLineOrigin::Top => [self.width / 2, 0],
                };
                let y = match style.endpoint {
                    1 => target.bounds[1],
                    2 => target.bounds[3],
                    _ => (target.bounds[1] + target.bounds[3]) / 2,
                };
                self.painter.line(
                    origin,
                    [
                        (target.bounds[0] + target.bounds[2]) / 2 + frame.unicorn_shift_x,
                        y,
                    ],
                    style,
                    frame.lines_color,
                )?;
            }
            if frame.draw_target_distance {
                if let Some(distance) = target.distance_units {
                    self.painter.distance(
                        target.bounds,
                        distance,
                        &styles[TARGET],
                        frame.target_color,
                    )?;
                }
            }
        }
        self.painter
            .present(self.hwnd, self.placement.0, self.placement.1)
    }
}

impl Drop for AuroraWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                self.hwnd,
                windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        windows::Win32::UI::WindowsAndMessaging::WM_PAINT => {
            ValidateRect(hwnd, None);
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_ERASEBKGND => LRESULT(1),
        windows::Win32::UI::WindowsAndMessaging::WM_DESTROY => {
            windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
