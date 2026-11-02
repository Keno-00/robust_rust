//! Client-relative comet from verified pawn XYZ and the selected camera matrix.
use crate::apb_atlas::AtlasFrame;
use crate::apb_orchid::{MissionRole, APB_UNITS_PER_METER};
use crate::config::{LanternFilter, ProfileConfig, Rgb};
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

pub fn includes(filter: LanternFilter, role: MissionRole) -> bool {
    filter.includes(role)
}

pub fn draw(
    painter: &mut crate::visual_painter::Painter,
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
    let radius = (f64::from(width.min(height)) * 0.11 * f64::from(config.comet_scale))
        .round()
        .max(6.0);
    let margin = radius + 8.0;
    if f64::from(width) < margin * 2.0 || f64::from(height) < margin * 2.0 {
        return Err(
            "Comet viewport is too small. Restore the APB window and restart processing.".into(),
        );
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
    let style = &config.visual_styles[crate::visual_style::RADAR];
    let center_pixel = pixel(0.0, 0.0);
    let mut backdrop = *style;
    backdrop.role_colors = false;
    backdrop.color = [0.0; 3];
    backdrop.shape = 0;
    backdrop.mode = 0;
    backdrop.opacity = style.tint(Rgb::new(1, 1, 1), painter.seconds).1 * style.fill_opacity;
    backdrop.fill_opacity = 1.0;
    backdrop.outline = 0;
    if style.fill {
        painter.shape(
            [
                center_pixel[0] - radius as i32,
                center_pixel[1] - radius as i32,
                center_pixel[0] + radius as i32,
                center_pixel[1] + radius as i32,
            ],
            &backdrop,
            Rgb::new(1, 1, 1),
            true,
        )?;
    }
    painter.line(
        pixel(-radius, 0.0),
        pixel(radius, 0.0),
        style,
        Rgb::new(190, 190, 190),
    )?;
    painter.line(
        pixel(0.0, -radius),
        pixel(0.0, radius),
        style,
        Rgb::new(190, 190, 190),
    )?;
    let dot = |painter: &mut crate::visual_painter::Painter, point: [i32; 2], r: i32, tint: Rgb| {
        let mut dot_style = *style;
        dot_style.fill = true;
        dot_style.fill_opacity = 1.0;
        dot_style.shape = 1;
        painter.shape(
            [
                point[0] - r,
                point[1] - r,
                point[0] + r + 1,
                point[1] + r + 1,
            ],
            &dot_style,
            tint,
            true,
        )
    };
    for pawn in &frame.comet_pawns {
        let delta = [
            f64::from(pawn.position[0]) - f64::from(origin[0]),
            f64::from(pawn.position[1]) - f64::from(origin[1]),
        ];
        let range = f64::from(config.radar_range) * APB_UNITS_PER_METER;
        if delta[0].hypot(delta[1]) > range {
            continue;
        }
        let x = (delta[0] * right[0] + delta[1] * right[1]) * (radius - 5.0) / range;
        let y = -(delta[0] * forward[0] + delta[1] * forward[1]) * (radius - 5.0) / range;
        let tint = match pawn.role {
            MissionRole::Opponent => style.role_color(0),
            MissionRole::Teammate => style.role_color(1),
            MissionRole::OutsideMission => style.role_color(2),
            MissionRole::Unknown => style.role_color(3),
        };
        dot(painter, pixel(x, y), style.marker_size, tint)?;
    }
    for pumpkin in pumpkins {
        let delta = [
            f64::from(pumpkin.position[0] - origin[0]),
            f64::from(pumpkin.position[1] - origin[1]),
        ];
        let range = f64::from(config.radar_range) * APB_UNITS_PER_METER;
        if delta[0].hypot(delta[1]) > range {
            continue;
        }
        let x = (delta[0] * right[0] + delta[1] * right[1]) * (radius - 5.0) / range;
        let y = -(delta[0] * forward[0] + delta[1] * forward[1]) * (radius - 5.0) / range;
        let tint = if pumpkin.red {
            Rgb::new(255, 70, 70)
        } else {
            Rgb::new(255, 155, 50)
        };
        dot(painter, pixel(x, y), style.marker_size, tint)?;
    }
    dot(
        painter,
        pixel(0.0, 0.0),
        style.marker_size + 1,
        Rgb::new(245, 245, 245),
    )
}
