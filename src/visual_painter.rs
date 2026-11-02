//! GDI rasterization into bounded layers, composited with per-pixel alpha.
use crate::{config::Rgb, visual_style::VisualStyle};
use windows::Win32::{
    Foundation::{COLORREF, HWND, POINT, SIZE},
    Graphics::Gdi::*,
    UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA},
};

struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    pixels: *mut u32,
    width: i32,
    height: i32,
}
impl Surface {
    unsafe fn new(width: i32, height: i32) -> Result<Self, String> {
        if width <= 0 || height <= 0 || width.checked_mul(height).is_none() {
            return Err(
                "Visual viewport size is invalid. Restore the game window and restart processing."
                    .into(),
            );
        }
        let dc = CreateCompatibleDC(HDC(0));
        if dc.is_invalid() {
            return Err("Allocate visual drawing context failed.".into());
        }
        let mut info = BITMAPINFO::default();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB.0;
        let mut bits = std::ptr::null_mut();
        let bitmap = match CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(bitmap) => bitmap,
            Err(error) => {
                let _ = DeleteDC(dc);
                return Err(format!("Allocate visual alpha buffer failed: {error}"));
            }
        };
        let old = SelectObject(dc, bitmap);
        if old.is_invalid() || bits.is_null() {
            let _ = DeleteObject(bitmap);
            let _ = DeleteDC(dc);
            return Err("Select visual alpha buffer failed.".into());
        }
        Ok(Self {
            dc,
            bitmap,
            old,
            pixels: bits.cast(),
            width,
            height,
        })
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(self.bitmap);
            let _ = DeleteDC(self.dc);
        }
    }
}

pub struct Painter {
    output: Surface,
    scratch: Surface,
    pub seconds: f32,
}
fn color(c: Rgb) -> COLORREF {
    // Zero marks untouched scratch pixels; near-black preserves visible black outlines.
    let value = u32::from(c.r) | (u32::from(c.g) << 8) | (u32::from(c.b) << 16);
    COLORREF(if value == 0 { 0x010101 } else { value })
}
impl Painter {
    pub fn new(width: i32, height: i32) -> Result<Self, String> {
        unsafe {
            Ok(Self {
                output: Surface::new(width, height)?,
                scratch: Surface::new(width, height)?,
                seconds: 0.0,
            })
        }
    }
    pub fn clear(&mut self, seconds: f32) {
        self.seconds = seconds;
        unsafe {
            std::ptr::write_bytes(
                self.output.pixels,
                0,
                (self.output.width * self.output.height) as usize,
            );
        }
    }
    pub fn present(&self, hwnd: HWND, left: i32, top: i32) -> Result<(), String> {
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        unsafe {
            UpdateLayeredWindow(
                hwnd,
                HDC(0),
                Some(&POINT { x: left, y: top }),
                Some(&SIZE {
                    cx: self.output.width,
                    cy: self.output.height,
                }),
                self.output.dc,
                Some(&POINT { x: 0, y: 0 }),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
        }
        .map_err(|e| format!("Present visual alpha buffer failed: {e}. Restart processing."))
    }
    fn layer(
        &mut self,
        bounds: [i32; 4],
        opacity: f32,
        draw: impl FnOnce(HDC) -> Result<(), String>,
    ) -> Result<(), String> {
        let [l, t, r, b] = [
            bounds[0].clamp(0, self.output.width),
            bounds[1].clamp(0, self.output.height),
            bounds[2].clamp(0, self.output.width),
            bounds[3].clamp(0, self.output.height),
        ];
        if l >= r || t >= b || opacity <= 0.0 {
            return Ok(());
        }
        unsafe {
            for y in t..b {
                std::ptr::write_bytes(
                    self.scratch
                        .pixels
                        .add((y * self.output.width + l) as usize),
                    0,
                    (r - l) as usize,
                );
            }
            let saved = SaveDC(self.scratch.dc);
            if saved == 0 {
                return Err("Save visual drawing state failed.".into());
            }
            let clipped = IntersectClipRect(self.scratch.dc, l, t, r, b);
            let result = if clipped.0 == 0 {
                Err("Clip visual drawing layer failed.".into())
            } else {
                draw(self.scratch.dc)
            };
            let flushed = GdiFlush().as_bool();
            let restored = RestoreDC(self.scratch.dc, saved).as_bool();
            result?;
            if !flushed || !restored {
                return Err("Complete visual drawing layer failed.".into());
            }
            let alpha = (opacity * 255.0).round() as u32;
            for y in t..b {
                for x in l..r {
                    let i = (y * self.output.width + x) as usize;
                    let src = *self.scratch.pixels.add(i);
                    if src & 0x00ffffff == 0 {
                        continue;
                    }
                    let dst = &mut *self.output.pixels.add(i);
                    let inverse = 255 - alpha;
                    let channel = |shift: u32| {
                        (((src >> shift) & 255) * alpha + ((*dst >> shift) & 255) * inverse + 127)
                            / 255
                    };
                    let a = alpha + (((*dst >> 24) & 255) * inverse + 127) / 255;
                    *dst = (a << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0);
                }
            }
        }
        Ok(())
    }
    pub fn shape(
        &mut self,
        bounds: [i32; 4],
        style: &VisualStyle,
        base: Rgb,
        circle: bool,
    ) -> Result<(), String> {
        let (tint, alpha) = style.tint(base, self.seconds);
        if style.fill {
            self.layer(bounds, alpha * style.fill_opacity, |dc| unsafe {
                let brush = CreateSolidBrush(color(tint));
                if brush.is_invalid() {
                    return Err("Allocate visual fill failed.".into());
                }
                let old = SelectObject(dc, brush);
                let pen = SelectObject(dc, GetStockObject(NULL_PEN));
                if old.is_invalid() || pen.is_invalid() {
                    if !old.is_invalid() {
                        SelectObject(dc, old);
                    }
                    if !pen.is_invalid() {
                        SelectObject(dc, pen);
                    }
                    let _ = DeleteObject(brush);
                    return Err("Select visual fill objects failed.".into());
                }
                let ok = if circle {
                    Ellipse(dc, bounds[0], bounds[1], bounds[2], bounds[3])
                } else {
                    Rectangle(dc, bounds[0], bounds[1], bounds[2], bounds[3])
                }
                .as_bool();
                SelectObject(dc, old);
                SelectObject(dc, pen);
                let _ = DeleteObject(brush);
                if ok {
                    Ok(())
                } else {
                    Err("Draw visual fill failed.".into())
                }
            })?;
        }
        if style.outline > 0 {
            let rgb = if style.outline == 1 {
                Rgb::new(1, 1, 1)
            } else {
                Rgb::new(
                    (style.outline_color[0] * 255.0) as u8,
                    (style.outline_color[1] * 255.0) as u8,
                    (style.outline_color[2] * 255.0) as u8,
                )
            };
            self.stroke_shape(
                bounds,
                style.width + style.outline_width * 2,
                rgb,
                alpha,
                circle,
                style,
            )?;
        }
        self.stroke_shape(bounds, style.width, tint, alpha, circle, style)
    }
    fn stroke_shape(
        &mut self,
        b: [i32; 4],
        width: i32,
        tint: Rgb,
        alpha: f32,
        circle: bool,
        style: &VisualStyle,
    ) -> Result<(), String> {
        let pad = width + 2;
        self.layer(
            [b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad],
            alpha,
            |dc| unsafe {
                let pen = CreatePen(PS_SOLID, width, color(tint));
                if pen.is_invalid() {
                    return Err("Allocate visual stroke failed.".into());
                }
                let old = SelectObject(dc, pen);
                let brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
                if old.is_invalid() || brush.is_invalid() {
                    if !old.is_invalid() {
                        SelectObject(dc, old);
                    }
                    if !brush.is_invalid() {
                        SelectObject(dc, brush);
                    }
                    let _ = DeleteObject(pen);
                    return Err("Select visual stroke objects failed.".into());
                }
                let result = if circle {
                    Ellipse(dc, b[0], b[1], b[2], b[3]).as_bool()
                } else if style.shape == 2 {
                    let dx = ((b[2] - b[0]) as f32 * style.corner_length).round() as i32;
                    let dy = ((b[3] - b[1]) as f32 * style.corner_length).round() as i32;
                    let mut ok = true;
                    for (x, y, sx, sy) in [
                        (b[0], b[1], 1, 1),
                        (b[2], b[1], -1, 1),
                        (b[0], b[3], 1, -1),
                        (b[2], b[3], -1, -1),
                    ] {
                        ok &= MoveToEx(dc, x + sx * dx, y, None).as_bool()
                            && LineTo(dc, x, y).as_bool()
                            && LineTo(dc, x, y + sy * dy).as_bool();
                    }
                    ok
                } else {
                    Rectangle(dc, b[0], b[1], b[2], b[3]).as_bool()
                };
                SelectObject(dc, brush);
                SelectObject(dc, old);
                let _ = DeleteObject(pen);
                if result {
                    Ok(())
                } else {
                    Err("Draw visual stroke failed.".into())
                }
            },
        )
    }
    pub fn line(
        &mut self,
        a: [i32; 2],
        b: [i32; 2],
        style: &VisualStyle,
        base: Rgb,
    ) -> Result<(), String> {
        let (tint, alpha) = style.tint(base, self.seconds);
        let mut strokes = Vec::with_capacity(2);
        if style.outline > 0 {
            strokes.push((
                style.width + 2 * style.outline_width,
                if style.outline == 1 {
                    Rgb::new(1, 1, 1)
                } else {
                    Rgb::new(
                        (style.outline_color[0] * 255.0) as u8,
                        (style.outline_color[1] * 255.0) as u8,
                        (style.outline_color[2] * 255.0) as u8,
                    )
                },
            ));
        }
        strokes.push((style.width, tint));
        for (width, tint) in strokes {
            self.layer(
                [
                    a[0].min(b[0]) - width,
                    a[1].min(b[1]) - width,
                    a[0].max(b[0]) + width + 1,
                    a[1].max(b[1]) + width + 1,
                ],
                alpha,
                |dc| unsafe {
                    let pen = CreatePen(PS_SOLID, width, color(tint));
                    if pen.is_invalid() {
                        return Err("Allocate visual line failed.".into());
                    }
                    let old = SelectObject(dc, pen);
                    if old.is_invalid() {
                        let _ = DeleteObject(pen);
                        return Err("Select visual line pen failed.".into());
                    }
                    let ok = MoveToEx(dc, a[0], a[1], None).as_bool()
                        && LineTo(dc, b[0], b[1]).as_bool();
                    SelectObject(dc, old);
                    let _ = DeleteObject(pen);
                    if ok {
                        Ok(())
                    } else {
                        Err("Draw visual line failed.".into())
                    }
                },
            )?;
        }
        Ok(())
    }
    pub fn distance(
        &mut self,
        bounds: [i32; 4],
        units: f64,
        style: &VisualStyle,
        base: Rgb,
    ) -> Result<(), String> {
        let label: Vec<u16> = format!("{:.1} m", units / crate::apb_orchid::APB_UNITS_PER_METER)
            .encode_utf16()
            .collect();
        let x = bounds[0].clamp(0, (self.output.width - label.len() as i32 * 10).max(0));
        let y = if bounds[3] < self.output.height - 22 {
            bounds[3] + 4
        } else {
            bounds[1] - 18
        };
        let y = y.clamp(0, (self.output.height - 20).max(0));
        let (tint, alpha) = style.tint(base, self.seconds);
        self.layer(
            [x, y, x + label.len() as i32 * 12, y + 24],
            alpha,
            |dc| unsafe {
                let old = SelectObject(dc, GetStockObject(DEFAULT_GUI_FONT));
                if old.is_invalid() {
                    return Err("Select visual distance font failed.".into());
                }
                let ok = SetBkMode(dc, TRANSPARENT) != 0
                    && SetTextColor(dc, color(tint)).0 != u32::MAX
                    && TextOutW(dc, x, y, &label).as_bool();
                SelectObject(dc, old);
                if ok {
                    Ok(())
                } else {
                    Err("Draw visual distance label failed.".into())
                }
            },
        )
    }
}
