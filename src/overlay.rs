use std::thread;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, COLORREF, RECT, HINSTANCE};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreatePen, DeleteDC, DeleteObject,
    GetDC, GetStockObject, LineTo, MoveToEx, Rectangle, ReleaseDC, SelectObject,
    SRCCOPY, NULL_BRUSH, PS_SOLID, ValidateRect, HBRUSH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
    GetSystemMetrics, RegisterClassW, SetLayeredWindowAttributes, ShowWindow, TranslateMessage,
    CS_HREDRAW, CS_VREDRAW, HMENU, LWA_COLORKEY, MSG, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOW,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_TRANSPARENT, WS_EX_TOPMOST, WS_EX_NOACTIVATE, WS_POPUP,
};
use crate::config::Rgb;

const WINDOW_CLASS_NAME: PCWSTR = w!("VeruxOverlayClass");

pub struct OverlayWindow {
    hwnd: HWND,
    width: i32,
    height: i32,
}

impl OverlayWindow {
    pub fn new() -> Result<Self, String> {
        let (tx, rx) = std::sync::mpsc::channel();

        // Register class and create window in a separate thread so its message pump
        // doesn't block winit's main GUI loop.
        thread::spawn(move || {
            unsafe {
                let instance = HINSTANCE(windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                    .unwrap_or_default().0);

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
                    let _ = tx.send(Err("Failed to create overlay window".to_string()));
                    return;
                }

                // Set transparency color key: black (RGB 0,0,0) becomes transparent
                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_COLORKEY);

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
            .map_err(|e| format!("Overlay thread failed: {}", e))??;

        Ok(Self { hwnd, width, height })
    }

    pub fn show(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    pub fn hide(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn draw_frame(
        &self,
        center_x: i32,
        center_y: i32,
        shift_x: i32,
        shift_y: i32,
        grab_w: i32,
        grab_h: i32,
        target: Option<(i32, i32, i32, i32)>, // (x, y, w, h) relative to aimbot center
        aim_shift: (i32, i32),
        lines_color: Rgb,
        fov_color: Rgb,
        draw_lines: bool,
        draw_fov: bool,
        x_only: bool,
    ) {
        unsafe {
            let hdc_screen = GetDC(self.hwnd);
            if hdc_screen.is_invalid() {
                return;
            }

            let hdc_mem = CreateCompatibleDC(hdc_screen);
            let hbmp_mem = CreateCompatibleBitmap(hdc_screen, self.width, self.height);
            let hold_bmp = SelectObject(hdc_mem, hbmp_mem);

            // 1. Clear background to black (RGB 0,0,0) - our transparency key
            let brush_black = HBRUSH(GetStockObject(windows::Win32::Graphics::Gdi::BLACK_BRUSH).0);
            let rect_screen = RECT {
                left: 0,
                top: 0,
                right: self.width,
                bottom: self.height,
            };
            windows::Win32::Graphics::Gdi::FillRect(hdc_mem, &rect_screen, brush_black);

            // Select hollow brush for outline rendering
            let brush_null = GetStockObject(NULL_BRUSH);
            let hold_brush = SelectObject(hdc_mem, brush_null);

            // 2. Draw FOV Square
            if draw_fov {
                let pen_fov = CreatePen(
                    PS_SOLID,
                    2,
                    COLORREF(
                        fov_color.r as u32
                            | ((fov_color.g as u32) << 8)
                            | ((fov_color.b as u32) << 16),
                    ),
                );
                let hold_pen = SelectObject(hdc_mem, pen_fov);

                // Draw central FOV boundary
                let fov_left = center_x - grab_w / 2 + shift_x;
                let fov_top = center_y - grab_h / 2 + shift_y;
                let _ = Rectangle(hdc_mem, fov_left, fov_top, fov_left + grab_w, fov_top + grab_h);

                // Draw target-relative bounding box if seen
                if let Some((tx, ty, tw, th)) = target {
                    let box_x = tx + center_x + shift_x;
                    let box_y = ty + center_y + shift_y;
                    let box_w_half = tw / 2;
                    let box_h_half = th / 2;
                    let _ = Rectangle(
                        hdc_mem,
                        box_x - box_w_half,
                        box_y - box_h_half,
                        box_x + box_w_half,
                        box_y + box_h_half,
                    );
                }

                SelectObject(hdc_mem, hold_pen);
                let _ = DeleteObject(pen_fov);
            }

            // 3. Draw Lines
            if draw_lines {
                let pen_lines = CreatePen(
                    PS_SOLID,
                    2,
                    COLORREF(
                        lines_color.r as u32
                            | ((lines_color.g as u32) << 8)
                            | ((lines_color.b as u32) << 16),
                    ),
                );
                let hold_pen = SelectObject(hdc_mem, pen_lines);

                if let Some((tx, ty, _, _)) = target {
                    // Line 1: Center to raw target center
                    let _ = MoveToEx(hdc_mem, center_x, center_y, None);
                    let _ = LineTo(hdc_mem, tx + center_x, ty + center_y);

                    // Line 2: Shifted center to target + aim_shift
                    let shifted_cx = center_x + shift_x;
                    let shifted_cy = center_y + shift_y;
                    let _ = MoveToEx(hdc_mem, shifted_cx, shifted_cy, None);
                    let target_y_offset = if x_only { 0 } else { ty + aim_shift.1 };
                    let _ = LineTo(
                        hdc_mem,
                        tx + center_x + shift_x + aim_shift.0,
                        target_y_offset + center_y + shift_y,
                    );
                }

                SelectObject(hdc_mem, hold_pen);
                let _ = DeleteObject(pen_lines);
            }

            // Copy double-buffered draw frame to screen
            let _ = BitBlt(
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
            SelectObject(hdc_mem, hold_brush);
            SelectObject(hdc_mem, hold_bmp);
            let _ = DeleteObject(hbmp_mem);
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(self.hwnd, hdc_screen);
        }
    }
}

impl Drop for OverlayWindow {
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
