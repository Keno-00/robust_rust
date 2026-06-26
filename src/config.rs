use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vec2i {
    pub x: i32,
    pub y: i32,
}

impl Vec2i {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HsvRange {
    pub lower: [u8; 3],
    pub upper: [u8; 3],
}

impl HsvRange {
    pub const fn new(lower: [u8; 3], upper: [u8; 3]) -> Self {
        Self { lower, upper }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelAcquisition {
    Contouring,
    Edging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RectCompare {
    GreaterThan,
    LessThan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AimActivation {
    Toggle,
    Hold,
    Timed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerActivation {
    Toggle,
    Hold,
    DoubleCrouch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmoothType {
    Linear,
    Logarithmic,
    Hyperbolic,
    Bezier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RampApply {
    Delta,
    Smoothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EaseCurve {
    Linear,
    Smoothstep,
    Quad,
    Cubic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProximityEffect {
    Strength,
    Smoothing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileConfig {
    pub monitor_resolution: Vec2i,
    pub grab_resolution: Vec2i,
    pub grab_shift: Vec2i,
    pub aim_shift: Vec2i,
    pub aimbot_hsv: [HsvRange; 3],
    pub triggerbot_hsv: HsvRange,
    pub aimbot: bool,
    pub aim_base_activation: AimActivation,
    pub aim_on_aim: bool,
    pub triggerbot: bool,
    pub trigger_activation: TriggerActivation,
    pub smoothbot: bool,
    pub smooth_type: SmoothType,
    pub smoothing: f32,
    pub trigger_threshold: i32,
    pub trigger_delay_ms: u32,
    pub aim_key: u32,
    pub aim_key_name: String,
    pub trigger_key: u32,
    pub trigger_key_name: String,
    pub pause_key: u32,
    pub pause_key_name: String,
    pub visuals_key: u32,
    pub visuals_key_name: String,
    pub block_key: u32,
    pub block_key_name: String,
    pub discord_rich_presence: bool,
    pub highest_pixel: bool,
    pub downscale: bool,
    pub downscale_factor: f32,
    pub visuals: bool,
    pub lines: bool,
    pub lines_color: Rgb,
    pub fov: bool,
    pub fov_color: Rgb,
    pub ui_color: Rgb,
    pub x_only: bool,
    pub fps: u32,
    pub rect_filter: bool,
    pub rect_compare: RectCompare,
    pub rect_ratio: f32,
    pub area_filter: bool,
    pub area_min: i32,
    pub area_max: i32,
    pub pixel_acquisition: PixelAcquisition,
    pub apb_path: String,
    pub merge_distance: i32,
    pub aim_distance: i32,
    pub ramp_time_ms: u32,
    pub ramp_idle_reset_ms: u32,
    pub ramp_apply: RampApply,
    pub ramp_ease: EaseCurve,
    pub proximity_charge: bool,
    pub proximity_distance: i32,
    pub proximity_build_ms: u32,
    pub proximity_effect: ProximityEffect,
    pub proximity_ease: EaseCurve,
    pub mouse_polling_rate: u32,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            monitor_resolution: Vec2i::new(1920, 1080),
            grab_resolution: Vec2i::new(500, 500),
            grab_shift: Vec2i::new(0, 0),
            aim_shift: Vec2i::new(0, 0),
            aimbot_hsv: [
                HsvRange::new([105, 214, 240], [135, 244, 255]),
                HsvRange::new([105, 227, 140], [135, 255, 170]),
                HsvRange::new([105, 228, 160], [135, 255, 190]),
            ],
            triggerbot_hsv: HsvRange::new([105, 227, 140], [135, 255, 170]),
            aimbot: true,
            aim_base_activation: AimActivation::Toggle,
            aim_on_aim: false,
            triggerbot: true,
            trigger_activation: TriggerActivation::Hold,
            smoothbot: false,
            smooth_type: SmoothType::Linear,
            smoothing: 2.5,
            trigger_threshold: 10,
            trigger_delay_ms: 30,
            aim_key: 112,
            aim_key_name: "VK_F1".to_string(),
            trigger_key: 113,
            trigger_key_name: "VK_F2".to_string(),
            pause_key: 34,
            pause_key_name: "VK_NEXT".to_string(),
            visuals_key: 45,
            visuals_key_name: "VK_INSERT".to_string(),
            block_key: 114,
            block_key_name: "VK_F3".to_string(),
            discord_rich_presence: false,
            highest_pixel: false,
            downscale: false,
            downscale_factor: 0.5,
            visuals: false,
            lines: true,
            lines_color: Rgb::new(255, 255, 255),
            fov: true,
            fov_color: Rgb::new(255, 255, 255),
            ui_color: Rgb::new(100, 100, 100),
            x_only: true,
            fps: 70,
            rect_filter: true,
            rect_compare: RectCompare::LessThan,
            rect_ratio: 0.0,
            area_filter: true,
            area_min: 15,
            area_max: 999_999,
            pixel_acquisition: PixelAcquisition::Edging,
            apb_path: String::new(),
            merge_distance: 20,
            aim_distance: 50,
            ramp_time_ms: 280,
            ramp_idle_reset_ms: 30,
            ramp_apply: RampApply::Delta,
            ramp_ease: EaseCurve::Linear,
            proximity_charge: false,
            proximity_distance: 50,
            proximity_build_ms: 700,
            proximity_effect: ProximityEffect::Strength,
            proximity_ease: EaseCurve::Smoothstep,
            mouse_polling_rate: 500,
        }
    }
}
impl ProfileConfig {
    pub fn from_map(map: &HashMap<String, String>) -> Self {
        let mut cfg = Self::default();
        
        let parse_vec2 = |s: &str| -> Option<Vec2i> {
            let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
            if parts.len() == 2 {
                let x = parts[0].parse().ok()?;
                let y = parts[1].parse().ok()?;
                Some(Vec2i::new(x, y))
            } else {
                None
            }
        };
        
        let parse_hsv = |s: &str| -> Option<[u8; 3]> {
            let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
            if parts.len() == 3 {
                let h = parts[0].parse().ok()?;
                let s = parts[1].parse().ok()?;
                let v = parts[2].parse().ok()?;
                Some([h, s, v])
            } else {
                None
            }
        };
        
        let parse_rgb = |s: &str| -> Option<Rgb> {
            let trimmed = s.trim_matches(|c| c == '(' || c == ')');
            let parts: Vec<&str> = trimmed.split(',').map(|p| p.trim()).collect();
            if parts.len() == 3 {
                let r = parts[0].parse().ok()?;
                let g = parts[1].parse().ok()?;
                let b = parts[2].parse().ok()?;
                Some(Rgb::new(r, g, b))
            } else {
                None
            }
        };
        
        let parse_bool = |s: &str| -> bool {
            let s_lower = s.to_lowercase();
            s_lower == "true" || s_lower == "1" || s_lower == "yes" || s_lower == "on"
        };
        
        if let Some(v) = map.get("monires").and_then(|s| parse_vec2(s)) { cfg.monitor_resolution = v; }
        if let Some(v) = map.get("grab_resolution").and_then(|s| parse_vec2(s)) { cfg.grab_resolution = v; }
        if let Some(v) = map.get("grab_shift").and_then(|s| parse_vec2(s)) { cfg.grab_shift = v; }
        if let Some(v) = map.get("aim_shift").and_then(|s| parse_vec2(s)) { cfg.aim_shift = v; }
        
        if let Some(v) = map.get("aimbot_hsv_lower").and_then(|s| parse_hsv(s)) { cfg.aimbot_hsv[0].lower = v; }
        if let Some(v) = map.get("aimbot_hsv_upper").and_then(|s| parse_hsv(s)) { cfg.aimbot_hsv[0].upper = v; }
        if let Some(v) = map.get("aimbot_hsv_lower_2").and_then(|s| parse_hsv(s)) { cfg.aimbot_hsv[1].lower = v; }
        if let Some(v) = map.get("aimbot_hsv_upper_2").and_then(|s| parse_hsv(s)) { cfg.aimbot_hsv[1].upper = v; }
        if let Some(v) = map.get("aimbot_hsv_lower_3").and_then(|s| parse_hsv(s)) { cfg.aimbot_hsv[2].lower = v; }
        if let Some(v) = map.get("aimbot_hsv_upper_3").and_then(|s| parse_hsv(s)) { cfg.aimbot_hsv[2].upper = v; }
        
        if let Some(v) = map.get("triggerbot_hsv_lower").and_then(|s| parse_hsv(s)) { cfg.triggerbot_hsv.lower = v; }
        if let Some(v) = map.get("triggerbot_hsv_upper").and_then(|s| parse_hsv(s)) { cfg.triggerbot_hsv.upper = v; }
        
        if let Some(v) = map.get("aimbot") { cfg.aimbot = parse_bool(v); }
        if let Some(v) = map.get("aim_base_activation") {
            cfg.aim_base_activation = match v.as_str() {
                "Toggle" => AimActivation::Toggle,
                "Hold" => AimActivation::Hold,
                "Timed" => AimActivation::Timed,
                _ => cfg.aim_base_activation,
            };
        }
        if let Some(v) = map.get("aim_on_aim") { cfg.aim_on_aim = parse_bool(v); }
        
        if let Some(v) = map.get("triggerbot") { cfg.triggerbot = parse_bool(v); }
        if let Some(v) = map.get("trigtype") {
            cfg.trigger_activation = match v.as_str() {
                "Toggle" => TriggerActivation::Toggle,
                "Hold" => TriggerActivation::Hold,
                "DoubleCrouch" => TriggerActivation::DoubleCrouch,
                _ => cfg.trigger_activation,
            };
        }
        
        if let Some(v) = map.get("Smoothbot") { cfg.smoothbot = parse_bool(v); }
        if let Some(v) = map.get("smoothtype") {
            cfg.smooth_type = match v.as_str() {
                "Linear" => SmoothType::Linear,
                "Logarithmic" => SmoothType::Logarithmic,
                "Hyperbolic" => SmoothType::Hyperbolic,
                "Bezier" => SmoothType::Bezier,
                _ => cfg.smooth_type,
            };
        }
        if let Some(v) = map.get("smoothing").and_then(|s| s.parse().ok()) { cfg.smoothing = v; }
        if let Some(v) = map.get("trigthresh").and_then(|s| s.parse().ok()) { cfg.trigger_threshold = v; }
        if let Some(v) = map.get("delay").and_then(|s| s.parse().ok()) { cfg.trigger_delay_ms = v; }
        
        if let Some(v) = map.get("aimkey").and_then(|s| s.parse().ok()) { cfg.aim_key = v; }
        if let Some(v) = map.get("aimkeyname") { cfg.aim_key_name = v.clone(); }
        if let Some(v) = map.get("trigkey").and_then(|s| s.parse().ok()) { cfg.trigger_key = v; }
        if let Some(v) = map.get("trigkeyname") { cfg.trigger_key_name = v.clone(); }
        if let Some(v) = map.get("pausekey").and_then(|s| s.parse().ok()) { cfg.pause_key = v; }
        if let Some(v) = map.get("pausekeyname") { cfg.pause_key_name = v.clone(); }
        if let Some(v) = map.get("viskey").and_then(|s| s.parse().ok()) { cfg.visuals_key = v; }
        if let Some(v) = map.get("viskeyname") { cfg.visuals_key_name = v.clone(); }
        if let Some(v) = map.get("blockkey").and_then(|s| s.parse().ok()) { cfg.block_key = v; }
        if let Some(v) = map.get("blockkeyname") { cfg.block_key_name = v.clone(); }
        
        if let Some(v) = map.get("discord_rich_presence") { cfg.discord_rich_presence = parse_bool(v); }
        if let Some(v) = map.get("highest_pixel") { cfg.highest_pixel = parse_bool(v); }
        if let Some(v) = map.get("downscale") { cfg.downscale = parse_bool(v); }
        if let Some(v) = map.get("downscale_factor").and_then(|s| s.parse().ok()) { cfg.downscale_factor = v; }
        if let Some(v) = map.get("visuals") { cfg.visuals = parse_bool(v); }
        if let Some(v) = map.get("lines") { cfg.lines = parse_bool(v); }
        if let Some(v) = map.get("lines_col").and_then(|s| parse_rgb(s)) { cfg.lines_color = v; }
        if let Some(v) = map.get("fov") { cfg.fov = parse_bool(v); }
        if let Some(v) = map.get("fov_col").and_then(|s| parse_rgb(s)) { cfg.fov_color = v; }
        if let Some(v) = map.get("ui_col").and_then(|s| parse_rgb(s)) { cfg.ui_color = v; }
        if let Some(v) = map.get("xonly") { cfg.x_only = parse_bool(v); }
        if let Some(v) = map.get("fps").and_then(|s| s.parse().ok()) { cfg.fps = v; }
        
        if let Some(v) = map.get("rect_filter") { cfg.rect_filter = parse_bool(v); }
        if let Some(v) = map.get("rect_compare") {
            cfg.rect_compare = match v.as_str() {
                "Greater than" => RectCompare::GreaterThan,
                "Less than" => RectCompare::LessThan,
                _ => cfg.rect_compare,
            };
        }
        if let Some(v) = map.get("rect_ratio").and_then(|s| s.parse().ok()) { cfg.rect_ratio = v; }
        if let Some(v) = map.get("area_filter") { cfg.area_filter = parse_bool(v); }
        if let Some(v) = map.get("area_min").and_then(|s| s.parse().ok()) { cfg.area_min = v; }
        if let Some(v) = map.get("area_max").and_then(|s| s.parse().ok()) { cfg.area_max = v; }
        
        if let Some(v) = map.get("col_mode") {
            cfg.pixel_acquisition = match v.as_str() {
                "contouring" => PixelAcquisition::Contouring,
                "edging" => PixelAcquisition::Edging,
                _ => cfg.pixel_acquisition,
            };
        }
        if let Some(v) = map.get("apbpath") { cfg.apb_path = v.clone(); }
        if let Some(v) = map.get("distance_threshold").and_then(|s| s.parse().ok()) { cfg.merge_distance = v; }
        if let Some(v) = map.get("aim_distance_threshold").and_then(|s| s.parse().ok()) { cfg.aim_distance = v; }
        
        if let Some(v) = map.get("ramp_time_ms").and_then(|s| s.parse().ok()) { cfg.ramp_time_ms = v; }
        if let Some(v) = map.get("ramp_idle_reset_ms").and_then(|s| s.parse().ok()) { cfg.ramp_idle_reset_ms = v; }
        if let Some(v) = map.get("ramp_apply") {
            cfg.ramp_apply = match v.as_str() {
                "delta" => RampApply::Delta,
                "smoothing" => RampApply::Smoothing,
                _ => cfg.ramp_apply,
            };
        }
        if let Some(v) = map.get("ramp_ease") {
            cfg.ramp_ease = match v.as_str() {
                "linear" => EaseCurve::Linear,
                "smoothstep" => EaseCurve::Smoothstep,
                "quad" => EaseCurve::Quad,
                "cubic" => EaseCurve::Cubic,
                _ => cfg.ramp_ease,
            };
        }
        
        if let Some(v) = map.get("prox_charge") { cfg.proximity_charge = parse_bool(v); }
        if let Some(v) = map.get("prox_distance").and_then(|s| s.parse().ok()) { cfg.proximity_distance = v; }
        if let Some(v) = map.get("prox_build_ms").and_then(|s| s.parse().ok()) { cfg.proximity_build_ms = v; }
        if let Some(v) = map.get("prox_effect") {
            cfg.proximity_effect = match v.as_str() {
                "strength" => ProximityEffect::Strength,
                "smoothing" => ProximityEffect::Smoothing,
                _ => cfg.proximity_effect,
            };
        }
        if let Some(v) = map.get("prox_ease") {
            cfg.proximity_ease = match v.as_str() {
                "linear" => EaseCurve::Linear,
                "smoothstep" => EaseCurve::Smoothstep,
                "quad" => EaseCurve::Quad,
                "cubic" => EaseCurve::Cubic,
                _ => cfg.proximity_ease,
            };
        }
        
        if let Some(v) = map.get("mouse_polling_rate").and_then(|s| s.parse().ok()) {
            cfg.mouse_polling_rate = v;
        }
        
        cfg
    }
    
    pub fn to_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        
        let format_vec2 = |v: Vec2i| -> String { format!("{}, {}", v.x, v.y) };
        let format_hsv = |v: [u8; 3]| -> String { format!("{}, {}, {}", v[0], v[1], v[2]) };
        let format_rgb = |v: Rgb| -> String { format!("({},{},{})", v.r, v.g, v.b) };
        let format_bool = |v: bool| -> String { if v { "True".to_string() } else { "False".to_string() } };
        
        map.insert("monires".to_string(), format_vec2(self.monitor_resolution));
        map.insert("grab_resolution".to_string(), format_vec2(self.grab_resolution));
        map.insert("grab_shift".to_string(), format_vec2(self.grab_shift));
        map.insert("aim_shift".to_string(), format_vec2(self.aim_shift));
        
        map.insert("aimbot_hsv_lower".to_string(), format_hsv(self.aimbot_hsv[0].lower));
        map.insert("aimbot_hsv_upper".to_string(), format_hsv(self.aimbot_hsv[0].upper));
        map.insert("aimbot_hsv_lower_2".to_string(), format_hsv(self.aimbot_hsv[1].lower));
        map.insert("aimbot_hsv_upper_2".to_string(), format_hsv(self.aimbot_hsv[1].upper));
        map.insert("aimbot_hsv_lower_3".to_string(), format_hsv(self.aimbot_hsv[2].lower));
        map.insert("aimbot_hsv_upper_3".to_string(), format_hsv(self.aimbot_hsv[2].upper));
        
        map.insert("triggerbot_hsv_lower".to_string(), format_hsv(self.triggerbot_hsv.lower));
        map.insert("triggerbot_hsv_upper".to_string(), format_hsv(self.triggerbot_hsv.upper));
        
        map.insert("aimbot".to_string(), format_bool(self.aimbot));
        map.insert("aim_base_activation".to_string(), match self.aim_base_activation {
            AimActivation::Toggle => "Toggle".to_string(),
            AimActivation::Hold => "Hold".to_string(),
            AimActivation::Timed => "Timed".to_string(),
        });
        map.insert("aimtype".to_string(), match self.aim_base_activation {
            AimActivation::Toggle => "Toggle".to_string(),
            AimActivation::Hold => "Hold".to_string(),
            AimActivation::Timed => "Timed".to_string(),
        });
        map.insert("aim_on_aim".to_string(), format_bool(self.aim_on_aim));
        
        map.insert("triggerbot".to_string(), format_bool(self.triggerbot));
        map.insert("trigtype".to_string(), match self.trigger_activation {
            TriggerActivation::Toggle => "Toggle".to_string(),
            TriggerActivation::Hold => "Hold".to_string(),
            TriggerActivation::DoubleCrouch => "DoubleCrouch".to_string(),
        });
        
        map.insert("Smoothbot".to_string(), format_bool(self.smoothbot));
        map.insert("smoothtype".to_string(), match self.smooth_type {
            SmoothType::Linear => "Linear".to_string(),
            SmoothType::Logarithmic => "Logarithmic".to_string(),
            SmoothType::Hyperbolic => "Hyperbolic".to_string(),
            SmoothType::Bezier => "Bezier".to_string(),
        });
        map.insert("smoothing".to_string(), self.smoothing.to_string());
        map.insert("trigthresh".to_string(), self.trigger_threshold.to_string());
        map.insert("delay".to_string(), self.trigger_delay_ms.to_string());
        
        map.insert("aimkey".to_string(), self.aim_key.to_string());
        map.insert("aimkeyname".to_string(), self.aim_key_name.clone());
        map.insert("trigkey".to_string(), self.trigger_key.to_string());
        map.insert("trigkeyname".to_string(), self.trigger_key_name.clone());
        map.insert("pausekey".to_string(), self.pause_key.to_string());
        map.insert("pausekeyname".to_string(), self.pause_key_name.clone());
        map.insert("viskey".to_string(), self.visuals_key.to_string());
        map.insert("viskeyname".to_string(), self.visuals_key_name.clone());
        map.insert("blockkey".to_string(), self.block_key.to_string());
        map.insert("blockkeyname".to_string(), self.block_key_name.clone());
        
        map.insert("discord_rich_presence".to_string(), format_bool(self.discord_rich_presence));
        map.insert("highest_pixel".to_string(), format_bool(self.highest_pixel));
        map.insert("downscale".to_string(), format_bool(self.downscale));
        map.insert("downscale_factor".to_string(), self.downscale_factor.to_string());
        map.insert("visuals".to_string(), format_bool(self.visuals));
        map.insert("lines".to_string(), format_bool(self.lines));
        map.insert("lines_col".to_string(), format_rgb(self.lines_color));
        map.insert("fov".to_string(), format_bool(self.fov));
        map.insert("fov_col".to_string(), format_rgb(self.fov_color));
        map.insert("ui_col".to_string(), format_rgb(self.ui_color));
        map.insert("xonly".to_string(), format_bool(self.x_only));
        map.insert("fps".to_string(), self.fps.to_string());
        
        map.insert("rect_filter".to_string(), format_bool(self.rect_filter));
        map.insert("rect_compare".to_string(), match self.rect_compare {
            RectCompare::GreaterThan => "Greater than".to_string(),
            RectCompare::LessThan => "Less than".to_string(),
        });
        map.insert("rect_ratio".to_string(), self.rect_ratio.to_string());
        map.insert("area_filter".to_string(), format_bool(self.area_filter));
        map.insert("area_min".to_string(), self.area_min.to_string());
        map.insert("area_max".to_string(), self.area_max.to_string());
        
        map.insert("col_mode".to_string(), match self.pixel_acquisition {
            PixelAcquisition::Contouring => "contouring".to_string(),
            PixelAcquisition::Edging => "edging".to_string(),
        });
        map.insert("apbpath".to_string(), self.apb_path.clone());
        map.insert("distance_threshold".to_string(), self.merge_distance.to_string());
        map.insert("aim_distance_threshold".to_string(), self.aim_distance.to_string());
        
        map.insert("ramp_time_ms".to_string(), self.ramp_time_ms.to_string());
        map.insert("ramp_idle_reset_ms".to_string(), self.ramp_idle_reset_ms.to_string());
        map.insert("ramp_apply".to_string(), match self.ramp_apply {
            RampApply::Delta => "delta".to_string(),
            RampApply::Smoothing => "smoothing".to_string(),
        });
        map.insert("ramp_ease".to_string(), match self.ramp_ease {
            EaseCurve::Linear => "linear".to_string(),
            EaseCurve::Smoothstep => "smoothstep".to_string(),
            EaseCurve::Quad => "quad".to_string(),
            EaseCurve::Cubic => "cubic".to_string(),
        });
        
        map.insert("prox_charge".to_string(), format_bool(self.proximity_charge));
        map.insert("prox_distance".to_string(), self.proximity_distance.to_string());
        map.insert("prox_build_ms".to_string(), self.proximity_build_ms.to_string());
        map.insert("prox_effect".to_string(), match self.proximity_effect {
            ProximityEffect::Strength => "strength".to_string(),
            ProximityEffect::Smoothing => "smoothing".to_string(),
        });
        map.insert("prox_ease".to_string(), match self.proximity_ease {
            EaseCurve::Linear => "linear".to_string(),
            EaseCurve::Smoothstep => "smoothstep".to_string(),
            EaseCurve::Quad => "quad".to_string(),
            EaseCurve::Cubic => "cubic".to_string(),
        });
        
        map.insert("mouse_polling_rate".to_string(), self.mouse_polling_rate.to_string());
        
        map
    }
}

pub struct IniFile {
    pub sections: HashMap<String, HashMap<String, String>>,
}

impl IniFile {
    pub fn parse(content: &str) -> Self {
        let mut sections = HashMap::new();
        let mut current_section = "default".to_string();
        
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len()-1].trim().to_string();
                continue;
            }
            if let Some(pos) = line.find('=') {
                let key = line[..pos].trim().to_string();
                let value = line[pos+1..].trim().to_string();
                sections.entry(current_section.clone())
                    .or_insert_with(HashMap::new)
                    .insert(key, value);
            }
        }
        Self { sections }
    }
    
    pub fn serialize(&self) -> String {
        let mut out = String::new();
        let mut sections: Vec<&String> = self.sections.keys().collect();
        sections.sort();
        for section in sections {
            out.push_str(&format!("[{}]\n", section));
            if let Some(map) = self.sections.get(section) {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                for key in keys {
                    out.push_str(&format!("{} = {}\n", key, map.get(key).unwrap()));
                }
            }
            out.push('\n');
        }
        out
    }
}

pub fn get_config_path() -> PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\".to_string());
    PathBuf::from(home).join("Documents").join("VeruxAPB").join("config.ini")
}

pub fn load_profile(name: &str) -> Option<ProfileConfig> {
    let path = get_config_path();
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(&path).ok()?;
    let ini = IniFile::parse(&content);
    let map = ini.sections.get(name)?;
    Some(ProfileConfig::from_map(map))
}

pub fn save_profile(name: &str, config: &ProfileConfig) -> std::io::Result<()> {
    let path = get_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    let content = if path.exists() {
        fs::read_to_string(&path).unwrap_or_default()
    } else {
        String::new()
    };
    
    let mut ini = IniFile::parse(&content);
    ini.sections.insert(name.to_string(), config.to_map());
    
    let serialized = ini.serialize();
    fs::write(&path, serialized)?;
    Ok(())
}

pub fn get_profile_names() -> Vec<String> {
    let path = get_config_path();
    if !path.exists() {
        return vec!["default".to_string()];
    }
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec!["default".to_string()],
    };
    let ini = IniFile::parse(&content);
    let mut names: Vec<String> = ini.sections.keys().cloned().collect();
    if !names.contains(&"default".to_string()) {
        names.push("default".to_string());
    }
    names.sort();
    names
}

pub fn get_login_path() -> PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\".to_string());
    PathBuf::from(home).join("Documents").join("VeruxAPB").join("login.ini")
}

pub fn load_license_key() -> String {
    let path = get_login_path();
    if !path.exists() {
        return String::new();
    }
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    let ini = IniFile::parse(&content);
    if let Some(section) = ini.sections.get("login") {
        if let Some(key) = section.get("license_key") {
            return key.trim().to_string();
        }
    }
    String::new()
}

pub fn save_license_key(key: &str) -> std::io::Result<()> {
    let path = get_login_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = if path.exists() {
        fs::read_to_string(&path).unwrap_or_default()
    } else {
        String::new()
    };
    let mut ini = IniFile::parse(&content);
    let mut login_section = ini.sections.remove("login").unwrap_or_default();
    login_section.insert("license_key".to_string(), key.trim().to_string());
    ini.sections.insert("login".to_string(), login_section);
    let serialized = ini.serialize();
    fs::write(&path, serialized)?;
    Ok(())
}

const DEFAULT_UI_INI: &[u8] = include_bytes!("../assets/src_ini/DefaultUI.ini");
const BASE_UI_INI: &[u8] = include_bytes!("../assets/src_ini/BaseUI.ini");
const APB_UBER_SHADERS: &[u8] = include_bytes!("../assets/src_ini/APBUberPostProcessBlendPixelShader.usf");
const BASE_PASS_SHADERS: &[u8] = include_bytes!("../assets/src_ini/BasePassPixelShader.usf");
const GPU_SKIN_SHADERS: &[u8] = include_bytes!("../assets/src_ini/GpuSkinVertexFactory.usf");

pub fn deploy_verux_config(target_dir: &str) -> std::io::Result<()> {
    let base_path = PathBuf::from(target_dir);
    
    let apb_config_dir = base_path.join("APBGame").join("Config");
    let engine_config_dir = base_path.join("Engine").join("Config");
    let engine_shaders_dir = base_path.join("Engine").join("Shaders");

    fs::create_dir_all(&apb_config_dir)?;
    fs::create_dir_all(&engine_config_dir)?;
    fs::create_dir_all(&engine_shaders_dir)?;

    fs::write(apb_config_dir.join("DefaultUI.ini"), DEFAULT_UI_INI)?;
    fs::write(engine_config_dir.join("BaseUI.ini"), BASE_UI_INI)?;
    fs::write(engine_shaders_dir.join("APBUberPostProcessBlendPixelShader.usf"), APB_UBER_SHADERS)?;

    Ok(())
}

pub fn deploy_experimental_config(target_dir: &str) -> std::io::Result<()> {
    let base_path = PathBuf::from(target_dir);
    
    let apb_config_dir = base_path.join("APBGame").join("Config");
    let engine_config_dir = base_path.join("Engine").join("Config");
    let engine_shaders_dir = base_path.join("Engine").join("Shaders");

    fs::create_dir_all(&apb_config_dir)?;
    fs::create_dir_all(&engine_config_dir)?;
    fs::create_dir_all(&engine_shaders_dir)?;

    fs::write(apb_config_dir.join("DefaultUI.ini"), DEFAULT_UI_INI)?;
    fs::write(engine_config_dir.join("BaseUI.ini"), BASE_UI_INI)?;
    fs::write(engine_shaders_dir.join("APBUberPostProcessBlendPixelShader.usf"), APB_UBER_SHADERS)?;
    fs::write(engine_shaders_dir.join("BasePassPixelShader.usf"), BASE_PASS_SHADERS)?;
    fs::write(engine_shaders_dir.join("GpuSkinVertexFactory.usf"), GPU_SKIN_SHADERS)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_python_profile() {
        let config = ProfileConfig::default();

        assert_eq!(config.monitor_resolution, Vec2i::new(1920, 1080));
        assert_eq!(config.grab_resolution, Vec2i::new(500, 500));
        assert_eq!(
            config.aimbot_hsv[0],
            HsvRange::new([105, 214, 240], [135, 244, 255])
        );
        assert_eq!(
            config.triggerbot_hsv,
            HsvRange::new([105, 227, 140], [135, 255, 170])
        );
        assert_eq!(config.aim_base_activation, AimActivation::Toggle);
        assert_eq!(config.trigger_activation, TriggerActivation::Hold);
        assert_eq!(config.smoothing, 2.5);
        assert_eq!(config.ramp_time_ms, 280);
        assert_eq!(config.ramp_idle_reset_ms, 30);
        assert_eq!(config.proximity_ease, EaseCurve::Smoothstep);
    }

    #[test]
    fn test_ini_serialization_roundtrip() {
        let mut config = ProfileConfig::default();
        config.smoothing = 4.2;
        config.aim_key_name = "VK_XBUTTON1".to_string();
        config.lines_color = Rgb::new(10, 20, 30);
        
        let map = config.to_map();
        let parsed = ProfileConfig::from_map(&map);
        
        assert_eq!(parsed.smoothing, 4.2);
        assert_eq!(parsed.aim_key_name, "VK_XBUTTON1");
        assert_eq!(parsed.lines_color, Rgb::new(10, 20, 30));
        assert_eq!(parsed.monitor_resolution, Vec2i::new(1920, 1080));
    }

    #[test]
    fn test_ini_file_parsing() {
        let content = "
[default]
smoothing = 3.5
aimkeyname = VK_F5

[custom_profile]
smoothing = 5.0
aimkeyname = VK_F6
";
        let ini = IniFile::parse(content);
        assert_eq!(ini.sections.len(), 2);
        
        let def = ini.sections.get("default").unwrap();
        assert_eq!(def.get("smoothing").unwrap(), "3.5");
        assert_eq!(def.get("aimkeyname").unwrap(), "VK_F5");
        
        let cust = ini.sections.get("custom_profile").unwrap();
        assert_eq!(cust.get("smoothing").unwrap(), "5.0");
        assert_eq!(cust.get("aimkeyname").unwrap(), "VK_F6");
    }
}
