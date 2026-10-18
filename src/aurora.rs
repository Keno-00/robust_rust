use crate::apb_orchid::APB_UNITS_PER_METER;
use crate::config::{UnicornLineOrigin, AuroraStyle, Rgb};
use std::cell::Cell;
use std::thread;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreatePen, DeleteDC, DeleteObject, Ellipse,
    GetDC, GetStockObject, LineTo, MoveToEx, Rectangle, ReleaseDC, SelectObject, SetBkMode,
    SetTextColor, TextOutW, ValidateRect, DEFAULT_GUI_FONT, HBRUSH, HDC, NULL_BRUSH, PS_SOLID,
    SRCCOPY, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetSystemMetrics,
    RegisterClassW, SetLayeredWindowAttributes, SetWindowPos, ShowWindow, TranslateMessage,
    CS_HREDRAW, CS_VREDRAW, HMENU, HWND_TOPMOST, LWA_COLORKEY, MSG, SM_CXSCREEN, SM_CYSCREEN,
    SWP_NOACTIVATE, SW_HIDE, SW_SHOW, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
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
    pub aurora_style: AuroraStyle,
    pub esp: &'a [AuroraMarker],
    pub comet: Option<(&'a crate::apb_atlas::AtlasFrame, &'a crate::config::ProfileConfig, &'a [crate::apb_glow::ObjectPoint])>,
    pub show_distance: bool,
}

fn color_ref(color: Rgb) -> COLORREF {
    COLORREF(color.r as u32 | ((color.g as u32) << 8) | ((color.b as u32) << 16))
}

unsafe fn outline(hdc: HDC, bounds: [i32; 4], color: Rgb, dot: bool) -> Result<(), String> {
    let pen = CreatePen(PS_SOLID, 2, color_ref(color));
    if pen.is_invalid() {
        return Err("Allocate visuals outline pen failed.".into());
    }
    let previous = SelectObject(hdc, pen);
    if previous.is_invalid() {
        let _ = DeleteObject(pen);
        return Err("Select visuals outline pen failed.".into());
    }
    let drawn = if dot {
        Ellipse(hdc, bounds[0], bounds[1], bounds[2], bounds[3])
    } else {
        Rectangle(hdc, bounds[0], bounds[1], bounds[2], bounds[3])
    };
    SelectObject(hdc, previous);
    let _ = DeleteObject(pen);
    if drawn.as_bool() {
        Ok(())
    } else {
        Err("Draw visuals outline failed.".into())
    }
}

unsafe fn distance_label(
    hdc: HDC,
    bounds: [i32; 4],
    distance: f64,
    color: Rgb,
    width: i32,
    height: i32,
) -> Result<(), String> {
    let meters = distance / APB_UNITS_PER_METER;
    let label: Vec<u16> = format!("{meters:.1} m").encode_utf16().collect();
    if SetTextColor(hdc, color_ref(color)).0 == u32::MAX {
        return Err("Set distance label color failed.".into());
    }
    let x = bounds[0].clamp(0, (width - label.len() as i32 * 9).max(0));
    let y = if bounds[3] < height - 22 {
        bounds[3] + 4
    } else {
        bounds[1] - 18
    };
    if TextOutW(hdc, x, y.clamp(0, (height - 18).max(0)), &label).as_bool() {
        Ok(())
    } else {
        Err("Draw distance label failed.".into())
    }
}

pub struct AuroraWindow {
    hwnd: HWND,
    width: i32,
    height: i32,
    placement: (i32, i32, i32, i32),
    visible: Cell<bool>,
}

impl AuroraWindow {
    pub fn new() -> Result<Self, String> {
        let (tx, rx) = std::sync::mpsc::channel();

        // Register class and create window in a separate thread so its message pump
        // doesn't block winit's main GUI loop.
        thread::spawn(move || {
            unsafe {
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

                // Set transparency color key: black (RGB 0,0,0) becomes transparent
                if let Err(error) = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_COLORKEY)
                {
                    let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
                    let _ = tx.send(Err(format!("Set Aurora transparency failed: {error}")));
                    return;
                }

                let _ = tx.send(Ok((hwnd, width, height)));

                let mut msg = MSG::default();
                while GetMessageW(&mut msg, HWND(0), 0, 0).as_bool() {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        });

        let (hwnd, width, height) = rx
            .recv()
            .map_err(|e| format!("Aurora thread failed: {}", e))??;

        Ok(Self {
            hwnd,
            width,
            height,
            placement: (0, 0, width, height),
            visible: Cell::new(false),
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
        self.width = width;
        self.height = height;
        self.placement = placement;
        Ok(())
    }

    pub fn draw_frame(&self, frame: AuroraFrame<'_>) -> Result<(), String> {
        unsafe {
            let hdc_screen = GetDC(self.hwnd);
            if hdc_screen.is_invalid() {
                return Err(
                    "Aurora drawing context unavailable. Restart Verux processing.".into(),
                );
            }

            let hdc_mem = CreateCompatibleDC(hdc_screen);
            let hbmp_mem = CreateCompatibleBitmap(hdc_screen, self.width, self.height);
            if hdc_mem.is_invalid() || hbmp_mem.is_invalid() {
                if !hbmp_mem.is_invalid() {
                    let _ = DeleteObject(hbmp_mem);
                }
                if !hdc_mem.is_invalid() {
                    let _ = DeleteDC(hdc_mem);
                }
                ReleaseDC(self.hwnd, hdc_screen);
                return Err(
                    "Aurora drawing buffer allocation failed. Restart Verux processing.".into(),
                );
            }
            let hold_bmp = SelectObject(hdc_mem, hbmp_mem);

            // 1. Clear background to black (RGB 0,0,0) - our transparency key
            let brush_black = HBRUSH(GetStockObject(windows::Win32::Graphics::Gdi::BLACK_BRUSH).0);
            let rect_screen = RECT {
                left: 0,
                top: 0,
                right: self.width,
                bottom: self.height,
            };
            let background_ready =
                windows::Win32::Graphics::Gdi::FillRect(hdc_mem, &rect_screen, brush_black) != 0;

            // Select hollow brush for outline rendering
            let brush_null = GetStockObject(NULL_BRUSH);
            let hold_brush = SelectObject(hdc_mem, brush_null);

            let hold_font = SelectObject(hdc_mem, GetStockObject(DEFAULT_GUI_FONT));
            let drawing = (|| -> Result<(), String> {
                if !background_ready
                    || hold_bmp.is_invalid()
                    || hold_brush.is_invalid()
                    || hold_font.is_invalid()
                {
                    return Err("Prepare visuals drawing buffer/objects failed.".into());
                }
                if SetBkMode(hdc_mem, TRANSPARENT) == 0 {
                    return Err("Set distance label background failed.".into());
                }
                if frame.draw_fov {
                    let left = frame.center[0] - frame.grab_size[0] / 2 + frame.grab_shift[0];
                    let top = frame.center[1] - frame.grab_size[1] / 2 + frame.grab_shift[1];
                    outline(
                        hdc_mem,
                        [
                            left,
                            top,
                            left + frame.grab_size[0],
                            top + frame.grab_size[1],
                        ],
                        frame.halo_color,
                        false,
                    )?;
                }
                if let Some((memory, config, pumpkins)) = frame.comet {
                    crate::comet::draw(hdc_mem, self.width, self.height, memory, config, pumpkins)?;
                }
                for marker in frame.esp {
                    let bounds = if marker.object_kind.is_some() {
                        [marker.point[0]-5, marker.point[1]-5, marker.point[0]+6, marker.point[1]+6]
                    } else { match frame.aurora_style {
                        AuroraStyle::Boxes => marker
                            .bounds
                            .ok_or("Aurora box supplied without verified body bounds.")?,
                        AuroraStyle::Dots => [
                            marker.point[0] - 3,
                            marker.point[1] - 3,
                            marker.point[0] + 4,
                            marker.point[1] + 4,
                        ],
                    }};
                    outline(
                        hdc_mem,
                        bounds,
                        marker.color,
                        match marker.object_kind {
                            Some(crate::apb_glow::ObjectKind::Wisp) => true,
                            Some(crate::apb_glow::ObjectKind::Jade) => false,
                            None => frame.aurora_style == AuroraStyle::Dots,
                        },
                    )?;
                    if frame.show_distance {
                        if let Some(distance) = marker.distance_units {
                            distance_label(
                                hdc_mem,
                                bounds,
                                distance,
                                marker.color,
                                self.width,
                                self.height,
                            )?;
                        }
                    }
                }
                if let Some(target) = frame.target {
                    if frame.draw_target_box {
                        outline(hdc_mem, target.bounds, frame.target_color, false)?;
                    }
                    if frame.draw_lines {
                        let pen = CreatePen(PS_SOLID, 2, color_ref(frame.lines_color));
                        if pen.is_invalid() {
                            return Err("Allocate unicorn line pen failed.".into());
                        }
                        let old_pen = SelectObject(hdc_mem, pen);
                        if old_pen.is_invalid() {
                            let _ = DeleteObject(pen);
                            return Err("Select unicorn line pen failed.".into());
                        }
                        let origin = match frame.line_origin {
                            UnicornLineOrigin::Center => frame.center,
                            UnicornLineOrigin::Top => [self.width / 2, 0],
                        };
                        let moved = MoveToEx(hdc_mem, origin[0], origin[1], None).as_bool();
                        let drawn = LineTo(
                            hdc_mem,
                            target.bounds[0]
                                + (target.bounds[2] - target.bounds[0]) / 2
                                + frame.unicorn_shift_x,
                            target.bounds[1] + (target.bounds[3] - target.bounds[1]) / 2,
                        )
                        .as_bool();
                        SelectObject(hdc_mem, old_pen);
                        let _ = DeleteObject(pen);
                        if !moved || !drawn {
                            return Err("Draw unicorn line failed.".into());
                        }
                    }
                    if frame.draw_target_distance {
                        if let Some(distance) = target.distance_units {
                            distance_label(
                                hdc_mem,
                                target.bounds,
                                distance,
                                frame.target_color,
                                self.width,
                                self.height,
                            )?;
                        }
                    }
                }
                Ok(())
            })();
            // Copy double-buffered draw frame to screen
            let result = BitBlt(
                hdc_screen,
                0,
                0,
                self.width,
                self.height,
                hdc_mem,
                0,
                0,
                SRCCOPY,
            );

            // Cleanup
            SelectObject(hdc_mem, hold_font);
            SelectObject(hdc_mem, hold_brush);
            SelectObject(hdc_mem, hold_bmp);
            let _ = DeleteObject(hbmp_mem);
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(self.hwnd, hdc_screen);
            drawing?;
            result.map_err(|error| format!("Render existing visuals Aurora failed: {error}"))?;
        }
        Ok(())
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
