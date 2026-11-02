use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, SRCCOPY, HBITMAP, HDC,
};

use windows::core::ComInterface;
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIOutput1, IDXGIOutputDuplication,
    DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
};
use std::time::{Duration, Instant};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};

// --- Legacy GDI Capture Backend (kept for performance benchmarks) ---

pub struct GdiCanvas {
    hdc_screen: HDC,
    hdc_mem: HDC,
    hbmp: HBITMAP,
    h_old_bmp: windows::Win32::Graphics::Gdi::HGDIOBJ,
    width: i32,
    height: i32,
}

impl GdiCanvas {
    pub fn new(width: i32, height: i32) -> Result<Self, String> {
        if width <= 0 || height <= 0 {
            return Err("Width and height must be positive".to_string());
        }

        unsafe {
            let hdc_screen = GetDC(HWND(0));
            if hdc_screen.is_invalid() {
                return Err("Failed to get screen DC".to_string());
            }

            let hdc_mem = CreateCompatibleDC(hdc_screen);
            if hdc_mem.is_invalid() {
                ReleaseDC(HWND(0), hdc_screen);
                return Err("Failed to create compatible DC".to_string());
            }

            let hbmp = CreateCompatibleBitmap(hdc_screen, width, height);
            if hbmp.is_invalid() {
                DeleteDC(hdc_mem);
                ReleaseDC(HWND(0), hdc_screen);
                return Err("Failed to create compatible bitmap".to_string());
            }

            let h_old_bmp = SelectObject(hdc_mem, hbmp);

            Ok(Self {
                hdc_screen,
                hdc_mem,
                hbmp,
                h_old_bmp,
                width,
                height,
            })
        }
    }

    pub fn canvas_region(
        &self,
        left: i32,
        top: i32,
        buf: &mut Vec<u8>,
    ) -> Result<(), String> {
        unsafe {
            if BitBlt(
                self.hdc_mem,
                0,
                0,
                self.width,
                self.height,
                self.hdc_screen,
                left,
                top,
                SRCCOPY,
            )
            .is_err()
            {
                return Err("BitBlt failed".to_string());
            }

            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: self.width,
                    biHeight: -self.height, // Negative height for top-down DIB
                    biPlanes: 1,
                    biBitCount: 24, // 24-bit BGR
                    biCompression: 0, // BI_RGB
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: Default::default(),
            };

            // DIB scanlines must be aligned to 4 bytes
            let row_pitch = ((self.width * 3 + 3) / 4) * 4;
            let data_size = row_pitch * self.height;
            buf.resize(data_size as usize, 0);

            let lines_copied = GetDIBits(
                self.hdc_screen,
                self.hbmp,
                0,
                self.height as u32,
                Some(buf.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            if lines_copied == 0 {
                return Err("GetDIBits failed".to_string());
            }

            // Pack buffer to remove row padding bytes
            let bgr_row_len = (self.width * 3) as usize;
            if row_pitch as usize > bgr_row_len {
                let mut write_idx = 0;
                for row in 0..(self.height as usize) {
                    let read_idx = row * (row_pitch as usize);
                    buf.copy_within(read_idx..(read_idx + bgr_row_len), write_idx);
                    write_idx += bgr_row_len;
                }
                buf.truncate(self.width as usize * self.height as usize * 3);
            } else {
                buf.truncate(self.width as usize * self.height as usize * 3);
            }

            Ok(())
        }
    }
}

impl Drop for GdiCanvas {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.hdc_mem, self.h_old_bmp);
            DeleteObject(self.hbmp);
            DeleteDC(self.hdc_mem);
            ReleaseDC(HWND(0), self.hdc_screen);
        }
    }
}


struct FrameRelease<'a> {
    dup: &'a IDXGIOutputDuplication,
    acquired: bool,
}

impl<'a> Drop for FrameRelease<'a> {
    fn drop(&mut self) {
        if self.acquired {
            unsafe {
                let _ = self.dup.ReleaseFrame();
            }
        }
    }
}

// --- DXGI Desktop Duplication Capture Backend ---

pub struct DxgiCanvas {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: Option<IDXGIOutputDuplication>,
    staging_texture: Option<ID3D11Texture2D>,
    width: u32,
    height: u32,
    performance_frequency: i64,
    last_present_counter: i64,
    fresh_presented_at: Option<Instant>,
}

impl DxgiCanvas {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        unsafe {
            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;
            let mut feature_level = D3D_FEATURE_LEVEL::default();

            D3D11CreateDevice(
                None, // default adapter
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                windows::Win32::Graphics::Direct3D11::D3D11_CREATE_DEVICE_FLAG(0),
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut feature_level),
                Some(&mut context),
            )
            .map_err(|e| format!("Failed to create D3D11 Device: {:?}", e))?;

            let device = device.ok_or_else(|| "D3D11 Device is None".to_string())?;
            let context = context.ok_or_else(|| "D3D11 Context is None".to_string())?;
            let mut performance_frequency = 0;
            QueryPerformanceFrequency(&mut performance_frequency)
                .map_err(|e| format!("Failed to read capture timing frequency: {e}"))?;
            if performance_frequency <= 0 {
                return Err("Capture timing frequency must be positive".into());
            }

            let mut canvas = Self {
                device,
                context,
                duplication: None,
                staging_texture: None,
                width,
                height,
                performance_frequency,
                last_present_counter: 0,
                fresh_presented_at: None,
            };

            // Try initial duplication setup
            let _ = canvas.ensure_duplication();

            Ok(canvas)
        }
    }

    fn ensure_duplication(&mut self) -> Result<IDXGIOutputDuplication, String> {
        if let Some(ref dup) = self.duplication {
            return Ok(dup.clone());
        }

        unsafe {
            let dxgi_device: IDXGIDevice = self
                .device
                .cast::<IDXGIDevice>()
                .map_err(|e| format!("Failed to cast to IDXGIDevice: {:?}", e))?;

            let adapter = dxgi_device
                .GetAdapter()
                .map_err(|e| format!("Failed to get DXGI Adapter: {:?}", e))?;

            let output = adapter
                .EnumOutputs(0)
                .map_err(|e| format!("Failed to enum primary output: {:?}", e))?;

            let output1: IDXGIOutput1 = output
                .cast::<IDXGIOutput1>()
                .map_err(|e| format!("Failed to cast to IDXGIOutput1: {:?}", e))?;

            let duplication = output1
                .DuplicateOutput(&self.device)
                .map_err(|e| format!("Failed to duplicate output: {:?}", e))?;

            self.duplication = Some(duplication.clone());
            Ok(duplication)
        }
    }

    pub fn canvas_region(
        &mut self,
        left: i32,
        top: i32,
        buf: &mut Vec<u8>,
    ) -> Result<(), String> {
        self.fresh_presented_at = None;
        unsafe {
            // Ensure duplication is initialized
            let dup = match self.ensure_duplication() {
                Ok(d) => d,
                Err(e) => return Err(e),
            };

            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;

            // Acquire the next frame with a 0ms timeout (immediate snapshot)
            match dup.AcquireNextFrame(0, &mut frame_info, &mut resource) {
                Ok(_) => {}
                Err(e) => {
                    let hresult = e.code();
                    if hresult == DXGI_ERROR_ACCESS_LOST {
                        // Access lost (e.g. UAC prompt or display change). Reset duplication to self-heal.
                        self.duplication = None;
                        return Err("DXGI duplication access lost, resetting...".to_string());
                    }
                    if hresult == DXGI_ERROR_WAIT_TIMEOUT {
                        // Legacy callers may reuse the buffer; predictive braking
                        // sees no new presentation and does not identify another sample.
                        return Ok(());
                    }
                    return Err(format!("AcquireNextFrame failed: {e}"));
                }
            }

            // Frame is acquired successfully. Initialize RAII release guard.
            let _release_guard = FrameRelease {
                dup: &dup,
                acquired: true,
            };

            let resource = resource.ok_or_else(|| "AcquireNextFrame succeeded but resource is None".to_string())?;
            let frame_texture: ID3D11Texture2D = resource
                .cast::<ID3D11Texture2D>()
                .map_err(|e| format!("Failed to cast resource to texture: {:?}", e))?;

            let mut desc = D3D11_TEXTURE2D_DESC::default();
            frame_texture.GetDesc(&mut desc);

            if self.width > desc.Width || self.height > desc.Height {
                return Err(format!("Capture region {}x{} exceeds the current desktop {}x{}. Reduce the capture region or restore the display resolution and restart processing.", self.width, self.height, desc.Width, desc.Height));
            }

            if let Some(staging) = &self.staging_texture {
                let mut staging_desc = D3D11_TEXTURE2D_DESC::default();
                staging.GetDesc(&mut staging_desc);
                if staging_desc.Width != desc.Width || staging_desc.Height != desc.Height
                    || staging_desc.Format != desc.Format {
                    self.staging_texture = None;
                }
            }

            // Lazy create staging texture if dimensions mismatch or missing
            if self.staging_texture.is_none() {
                let staging_desc = D3D11_TEXTURE2D_DESC {
                    Width: desc.Width,
                    Height: desc.Height,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM, // DXGI outputs 32-bit BGRA
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_STAGING,
                    BindFlags: 0,
                    CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                    MiscFlags: 0,
                };

                let mut tex = None;
                self.device
                    .CreateTexture2D(&staging_desc, None, Some(&mut tex))
                    .map_err(|e| format!("Failed to create staging texture: {:?}", e))?;
                self.staging_texture = Some(tex.ok_or("DXGI staging texture creation returned no texture. Restart processing.")?);
            }

            let staging = self.staging_texture.as_ref().unwrap();

            // Copy frame texture to staging texture
            self.context.CopyResource(staging, &frame_texture);

            // Map staging texture for reading
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|e| format!("Failed to map staging texture: {:?}", e))?;

            let src_ptr = mapped.pData as *const u8;
            let row_pitch = mapped.RowPitch as usize;

            // Crop region and convert BGRA -> BGR in a single pass
            let grab_w = self.width as usize;
            let grab_h = self.height as usize;
            buf.resize(grab_w * grab_h * 3, 0);

            // Ensure bounding region fits inside the captured desktop resolution
            let desktop_w = desc.Width as i32;
            let desktop_h = desc.Height as i32;

            let start_x = left.clamp(0, desktop_w - self.width as i32) as usize;
            let start_y = top.clamp(0, desktop_h - self.height as i32) as usize;

            for r in 0..grab_h {
                let src_row_offset = (start_y + r) * row_pitch + start_x * 4;
                let src_row = src_ptr.add(src_row_offset);
                let dst_row_offset = r * grab_w * 3;

                for c in 0..grab_w {
                    let pixel_src = src_row.add(c * 4);
                    let dst_pixel = dst_row_offset + c * 3;

                    buf[dst_pixel]     = *pixel_src;       // B
                    buf[dst_pixel + 1] = *pixel_src.add(1); // G
                    buf[dst_pixel + 2] = *pixel_src.add(2); // R
                }
            }

            self.context.Unmap(staging, 0);

            // Pointer-only updates have LastPresentTime=0 and are not new game
            // measurements. Timestamp the image, rather than the end of its copy.
            if frame_info.LastPresentTime > 0
                && frame_info.LastPresentTime != self.last_present_counter
            {
                let mut current_counter = 0;
                QueryPerformanceCounter(&mut current_counter)
                    .map_err(|e| format!("Failed to read capture presentation timing: {e}"))?;
                let age_ticks = current_counter.checked_sub(frame_info.LastPresentTime)
                    .filter(|ticks| *ticks >= 0)
                    .ok_or_else(|| "DXGI presentation timestamp is later than the capture clock".to_string())?;
                let age_s = age_ticks as f64 / self.performance_frequency as f64;
                self.fresh_presented_at = Instant::now().checked_sub(Duration::from_secs_f64(age_s));
                self.last_present_counter = frame_info.LastPresentTime;
            }

            Ok(())
        }
    }

    pub fn fresh_presented_at(&self) -> Option<Instant> {
        self.fresh_presented_at
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_desktop_dimensions() {
        if let Ok(canvas) = GdiCanvas::new(100, 100) {
            let mut buf = Vec::new();
            let res = canvas.canvas_region(0, 0, &mut buf);
            match res {
                Ok(_) => {
                    assert_eq!(buf.len(), 100 * 100 * 3);
                }
                Err(e) => {
                    println!("GDI Capture skipped (expected in headless environments): {}", e);
                }
            }
        }
    }

    #[test]
    fn test_dxgi_capture_desktop_dimensions() {
        if let Ok(mut canvas) = DxgiCanvas::new(100, 100) {
            let mut buf = Vec::new();
            let res = canvas.canvas_region(0, 0, &mut buf);
            match res {
                Ok(_) => {
                    assert_eq!(buf.len(), 100 * 100 * 3);
                }
                Err(e) => {
                    println!("DXGI Capture skipped (expected in headless/RDP environments): {}", e);
                }
            }
        }
    }
}
