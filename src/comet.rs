//! Client-relative comet from verified pawn XYZ and the selected camera matrix.
use crate::apb_atlas::AtlasFrame;
use crate::apb_orchid::{MissionRole, APB_UNITS_PER_METER};
use crate::config::{ProfileConfig, LanternFilter, Rgb};
use windows::Win32::Foundation::COLORREF;
use windows::Win32::Graphics::Gdi::{
    CreatePen, CreateSolidBrush, DeleteObject, Ellipse, LineTo, MoveToEx, SelectObject, HDC,
    PS_SOLID,
};

pub const PLACEMENTS: [&str; 10] = [
    "Top Left",
    "Top Right",
    "Bottom Left",
    "Bottom Right",
    "Top Center",
    "Bottom Center",
    "Center Left",
    "Center Right",
    "Custom",
    "Center",
];
pub const CUSTOM_PLACEMENT: usize = 8;
pub const MIN_SCALE: f32 = 0.5;
pub const MAX_SCALE: f32 = 2.0;
const RANGE_METERS: f64 = 100.0;

pub fn includes(filter: LanternFilter, role: MissionRole) -> bool {
    filter.includes(role)
}

fn color(color: Rgb) -> COLORREF {
    COLORREF(color.r as u32 | ((color.g as u32) << 8) | ((color.b as u32) << 16))
}

unsafe fn line(hdc: HDC, a: [i32; 2], b: [i32; 2]) -> Result<(), String> {
    if !MoveToEx(hdc, a[0], a[1], None).as_bool() || !LineTo(hdc, b[0], b[1]).as_bool() {
        return Err("Draw Comet cross failed. Restart processing; see the diagnostic log.".into());
    }
    Ok(())
}

unsafe fn dot(hdc: HDC, point: [i32; 2], radius: i32, tint: Rgb) -> Result<(), String> {
    let pen = CreatePen(PS_SOLID, 1, color(tint));
    let brush = CreateSolidBrush(color(tint));
    if pen.is_invalid() || brush.is_invalid() {
        if !pen.is_invalid() {
            let _ = DeleteObject(pen);
        }
        if !brush.is_invalid() {
            let _ = DeleteObject(brush);
        }
        return Err("Allocate Comet dot objects failed. Restart processing.".into());
    }
    let old_pen = SelectObject(hdc, pen);
    let old_brush = SelectObject(hdc, brush);
    let ready = !old_pen.is_invalid() && !old_brush.is_invalid();
    let drawn = ready
        && Ellipse(
            hdc,
            point[0] - radius,
            point[1] - radius,
            point[0] + radius + 1,
            point[1] + radius + 1,
        )
        .as_bool();
    if !old_pen.is_invalid() {
        SelectObject(hdc, old_pen);
    }
    if !old_brush.is_invalid() {
        SelectObject(hdc, old_brush);
    }
    let _ = DeleteObject(pen);
    let _ = DeleteObject(brush);
    if drawn {
        Ok(())
    } else {
        Err("Draw Comet dot failed. Restart processing.".into())
    }
}

pub unsafe fn draw(
    hdc: HDC,
    width: i32,
    height: i32,
    frame: &AtlasFrame,
    config: &ProfileConfig,
    pumpkins: &[crate::apb_glow::ObjectPoint],
) -> Result<(), String> {
    let origin = frame.comet_origin.ok_or("Comet local position unavailable. Enter the district with your character and restart processing.")?;
    // Z is vertical in the verified collision layout. The row-vector camera's
    // clip-X column gives horizontal screen-right, including live yaw.
    let matrix = frame.camera.matrix;
    let right = [f64::from(matrix[0]), f64::from(matrix[4])];
    let length = right[0].hypot(right[1]);
    if !length.is_finite() || length < 1e-6 {
        return Err("Comet camera horizontal basis invalid. Restart processing and inspect the camera diagnostics.".into());
    }
    let right = [right[0] / length, right[1] / length];
    let mut forward = [-right[1], right[0]];
    let orientation = forward[0] * f64::from(matrix[3]) + forward[1] * f64::from(matrix[7]);
    if !orientation.is_finite() || orientation.abs() < 1e-6 {
        return Err("Comet camera heading unavailable at this pitch. Level the camera and restart processing.".into());
    }
    if orientation < 0.0 {
        forward = [-forward[0], -forward[1]];
    }
    if !config.comet_scale.is_finite() || !(MIN_SCALE..=MAX_SCALE).contains(&config.comet_scale) {
        return Err("Comet scale is invalid. Select a scale from 0.5 to 2.0 in Visuals.".into());
    }
    let radius = (f64::from(width.min(height)) * 0.11 * f64::from(config.comet_scale)).round().max(6.0);
    let margin = radius + 8.0;
    if f64::from(width) < margin * 2.0 || f64::from(height) < margin * 2.0 {
        return Err("Comet viewport is too small. Restore the APB window and restart processing.".into());
    }
    let anchors = [
        [0.0, 0.0],
        [1.0, 0.0],
        [0.0, 1.0],
        [1.0, 1.0],
        [0.5, 0.0],
        [0.5, 1.0],
        [0.0, 0.5],
        [1.0, 0.5],
        [0.0, 0.0], // Index 8 remains reserved for saved Custom placements.
        [0.5, 0.5],
    ];
    let anchor = if config.comet_placement == CUSTOM_PLACEMENT {
        if !config.comet_x_percent.is_finite() || !config.comet_y_percent.is_finite() {
            return Err("Comet placement must use finite X/Y percentages.".into());
        }
        [
            f64::from(config.comet_x_percent.clamp(0.0, 100.0)) / 100.0,
            f64::from(config.comet_y_percent.clamp(0.0, 100.0)) / 100.0,
        ]
    } else {
        *anchors
            .get(config.comet_placement)
            .ok_or("Comet placement is invalid. Select a position in Visuals.")?
    };
    let center = [
        (anchor[0] * f64::from(width)).clamp(margin, f64::from(width) - margin),
        (anchor[1] * f64::from(height)).clamp(margin, f64::from(height) - margin),
    ];
    let pixel = |x: f64, y: f64| {
        [
            (center[0] + x).round() as i32,
            (center[1] + y).round() as i32,
        ]
    };
    let pen = CreatePen(PS_SOLID, 1, color(Rgb::new(190, 190, 190)));
    if pen.is_invalid() {
        return Err("Allocate Comet cross pen failed. Restart processing.".into());
    }
    let old = SelectObject(hdc, pen);
    if old.is_invalid() {
        let _ = DeleteObject(pen);
        return Err("Select Comet cross pen failed.".into());
    }
    // The cross stays aligned to the screen; only pawn offsets use camera heading.
    let result = (|| {
        line(hdc, pixel(-radius, 0.0), pixel(radius, 0.0))?;
        line(hdc, pixel(0.0, -radius), pixel(0.0, radius))?;
        Ok::<(), String>(())
    })();
    SelectObject(hdc, old);
    let _ = DeleteObject(pen);
    result?;
    for pawn in &frame.comet_pawns {
        let delta = [
            f64::from(pawn.position[0]) - f64::from(origin[0]),
            f64::from(pawn.position[1]) - f64::from(origin[1]),
        ];
        let range = RANGE_METERS * APB_UNITS_PER_METER;
        if delta[0].hypot(delta[1]) > range {
            continue;
        }
        let x = (delta[0] * right[0] + delta[1] * right[1]) * (radius - 5.0) / range;
        let y = -(delta[0] * forward[0] + delta[1] * forward[1]) * (radius - 5.0) / range;
        let tint = match pawn.role {
            MissionRole::Opponent => config.opponent_color,
            MissionRole::Teammate => config.teammate_color,
            MissionRole::OutsideMission => config.outside_mission_color,
            MissionRole::Unknown => config.unknown_color,
        };
        dot(hdc, pixel(x, y), 3, tint)?;
    }
    for pumpkin in pumpkins {
        let delta = [f64::from(pumpkin.position[0] - origin[0]), f64::from(pumpkin.position[1] - origin[1])];
        let range = RANGE_METERS * APB_UNITS_PER_METER;
        if delta[0].hypot(delta[1]) > range { continue; }
        let x = (delta[0] * right[0] + delta[1] * right[1]) * (radius - 5.0) / range;
        let y = -(delta[0] * forward[0] + delta[1] * forward[1]) * (radius - 5.0) / range;
        let tint = if pumpkin.red { Rgb::new(255, 70, 70) } else { Rgb::new(255, 155, 50) };
        dot(hdc, pixel(x, y), 3, tint)?;
    }
    dot(hdc, pixel(0.0, 0.0), 4, Rgb::new(245, 245, 245))
}
