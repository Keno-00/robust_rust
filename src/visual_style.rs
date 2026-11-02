//! Shared persisted appearance for every overlay consumer.
use crate::config::{ProfileConfig, Rgb};
use serde::{Deserialize, Serialize};

pub const COUNT: usize = 7;
pub const FOV: usize = 0;
pub const TARGET: usize = 1;
pub const LINES: usize = 2;
pub const PLAYERS: usize = 3;
pub const PUMPKINS: usize = 4;
pub const PROJECTILES: usize = 5;
pub const RADAR: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualStyle {
    pub color: [f32; 3],
    pub mode: usize,
    pub opacity: f32,
    pub width: i32,
    pub outline: usize,
    pub outline_width: i32,
    pub outline_color: [f32; 3],
    pub fill: bool,
    pub fill_opacity: f32,
    pub speed: f32,
    pub saturation: f32,
    pub brightness: f32,
    pub minimum_opacity: f32,
    pub shape: usize,
    pub corner_length: f32,
    pub marker_size: i32,
    pub max_distance: f32,
    pub role_colors: bool,
    pub palette: [[f32; 3]; 4],
    pub show_distance: bool,
    pub endpoint: usize,
}
impl Default for VisualStyle {
    fn default() -> Self {
        Self {
            color: [1.0; 3],
            mode: 0,
            opacity: 1.0,
            width: 2,
            outline: 0,
            outline_width: 1,
            outline_color: [0.0; 3],
            fill: false,
            fill_opacity: 0.15,
            speed: 1.0,
            saturation: 1.0,
            brightness: 1.0,
            minimum_opacity: 0.2,
            shape: 0,
            corner_length: 0.25,
            marker_size: 5,
            max_distance: 0.0,
            role_colors: false,
            palette: [
                [1.0, 70.0 / 255.0, 70.0 / 255.0],
                [70.0 / 255.0, 150.0 / 255.0, 1.0],
                [160.0 / 255.0; 3],
                [1.0, 190.0 / 255.0, 60.0 / 255.0],
            ],
            show_distance: false,
            endpoint: 0,
        }
    }
}
impl VisualStyle {
    pub fn validate(&self) -> Result<(), String> {
        let unit = |x: f32| x.is_finite() && (0.0..=1.0).contains(&x);
        if self.mode > 2
            || self.outline > 2
            || self.shape > 2
            || self.endpoint > 2
            || !(1..=8).contains(&self.width)
            || !(1..=5).contains(&self.outline_width)
            || !(1..=30).contains(&self.marker_size)
            || !self.color.into_iter().all(unit)
            || !self.outline_color.into_iter().all(unit)
            || !self.palette.into_iter().flatten().all(unit)
            || !unit(self.opacity)
            || !unit(self.fill_opacity)
            || !unit(self.minimum_opacity)
            || self.minimum_opacity > self.opacity
            || !unit(self.saturation)
            || !unit(self.brightness)
            || !self.speed.is_finite()
            || !(0.05..=5.0).contains(&self.speed)
            || !self.corner_length.is_finite()
            || !(0.05..=0.5).contains(&self.corner_length)
            || !self.max_distance.is_finite()
            || !(0.0..=2000.0).contains(&self.max_distance)
        {
            return Err("Visual appearance is invalid. Correct Visuals settings and retry.".into());
        }
        Ok(())
    }
    pub fn role_color(&self, index: usize) -> Rgb {
        let color = self.palette[index];
        Rgb::new(
            (color[0] * 255.0).round() as u8,
            (color[1] * 255.0).round() as u8,
            (color[2] * 255.0).round() as u8,
        )
    }
    pub fn tint(&self, base: Rgb, seconds: f32) -> (Rgb, f32) {
        let color = if self.mode == 1 {
            let h = (seconds * self.speed * 0.1).rem_euclid(1.0) * 6.0;
            let c = self.brightness * self.saturation;
            let x = c * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
            let m = self.brightness - c;
            let rgb = match h as u32 {
                0 => [c, x, 0.0],
                1 => [x, c, 0.0],
                2 => [0.0, c, x],
                3 => [0.0, x, c],
                4 => [x, 0.0, c],
                _ => [c, 0.0, x],
            };
            Rgb::new(
                ((rgb[0] + m) * 255.0).round() as u8,
                ((rgb[1] + m) * 255.0).round() as u8,
                ((rgb[2] + m) * 255.0).round() as u8,
            )
        } else if self.role_colors {
            base
        } else {
            Rgb::new(
                (self.color[0] * 255.0).round() as u8,
                (self.color[1] * 255.0).round() as u8,
                (self.color[2] * 255.0).round() as u8,
            )
        };
        let opacity = if self.mode == 2 {
            self.minimum_opacity
                + (self.opacity - self.minimum_opacity)
                    * (0.5 - 0.5 * (seconds * self.speed * std::f32::consts::TAU).cos())
        } else {
            self.opacity
        };
        (color, opacity)
    }
}
pub fn migrate(c: &ProfileConfig) -> [VisualStyle; COUNT] {
    let mut styles = [VisualStyle::default(); COUNT];
    for (i, color) in [
        (FOV, c.halo_color),
        (TARGET, c.unicorn_target_color),
        (LINES, c.lines_color),
    ] {
        styles[i].color = [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
        ];
    }
    let palette = [
        c.opponent_color,
        c.teammate_color,
        c.outside_mission_color,
        c.unknown_color,
    ]
    .map(|c| [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0]);
    for style in &mut styles {
        style.palette = palette;
        style.show_distance = c.show_distance;
    }
    styles[PLAYERS].role_colors = true;
    styles[PLAYERS].shape = usize::from(c.aurora_style == crate::config::AuroraStyle::Dots);
    styles[PLAYERS].marker_size = 3;
    styles[PUMPKINS].max_distance = 80.0;
    styles[PROJECTILES].max_distance = 80.0;
    styles[PUMPKINS].role_colors = true;
    styles[PROJECTILES].role_colors = true;
    styles[PROJECTILES].shape = 1;
    styles[RADAR].role_colors = true;
    styles[RADAR].width = 1;
    styles[RADAR].marker_size = 3;
    styles
}

pub fn migrate_defaults() -> [VisualStyle; COUNT] {
    let mut styles = [VisualStyle::default(); COUNT];
    styles[PLAYERS].role_colors = true;
    styles[PLAYERS].marker_size = 3;
    styles[PUMPKINS].max_distance = 80.0;
    styles[PROJECTILES].max_distance = 80.0;
    styles[PUMPKINS].role_colors = true;
    styles[PROJECTILES].role_colors = true;
    styles[PROJECTILES].shape = 1;
    styles[RADAR].role_colors = true;
    styles[RADAR].width = 1;
    styles[RADAR].marker_size = 3;
    styles
}
