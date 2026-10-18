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
pub enum CoordinateSource {
    ColorDetection,
    Memory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanternFilter {
    Opponents,
    MissionPlayers,
    AllPlayers,
}

impl LanternFilter {
    pub fn includes(self, role: crate::apb_orchid::MissionRole) -> bool {
        use crate::apb_orchid::MissionRole;
        match self {
            Self::Opponents => role == MissionRole::Opponent,
            Self::MissionPlayers => matches!(role, MissionRole::Opponent | MissionRole::Teammate),
            Self::AllPlayers => true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeRequirements {
    pub atlas_unicorn: bool,
    pub memory_pegasus: bool,
    pub color_unicorn: bool,
    pub color_pegasus: bool,
    pub orchid_atlas: bool,
    pub screen_canvas: bool,
    pub aurora: bool,
    pub quill: bool,
}

/// Effective capability gates shared by UI, preparation, and runtime.
#[derive(Debug, Clone, Copy)]
pub struct FeatureEligibility {
    pub memory: bool,
    pub aim: bool,
    pub trigger: bool,
    pub memory_aim: bool,
    pub color_aim: bool,
    pub memory_trigger: bool,
    pub color_trigger: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuroraStyle {
    Boxes,
    Dots,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnicornLineOrigin {
    Center,
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalTracking {
    Current, NoY, Head, Neck, Torso, Feet, ClosestPoint,
}
impl VerticalTracking {
    fn key(self) -> &'static str {
        match self { Self::Current => "current", Self::NoY => "no_y", Self::Head => "head", Self::Neck => "neck", Self::Torso => "torso", Self::Feet => "feet", Self::ClosestPoint => "closest_point" }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrbitOrder {
    ClosestUnicorn,
    ClosestDistance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RectCompare {
    GreaterThan,
    LessThan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnicornActivation {
    Toggle,
    Hold,
    Timed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PegasusActivation {
    Toggle,
    Hold,
    DoubleCrouch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeesmasType {
    Linear,
    Logarithmic,
    Hyperbolic,
    Bezier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TideApply {
    Delta,
    BeesmasValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcCurve {
    Linear,
    Beesmasstep,
    Beesmascurve,
    Quad,
    Cubic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VelvetEffect {
    Strength,
    BeesmasValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileConfig {
    pub com_port: String,
    pub monitor_resolution: Vec2i,
    pub grab_resolution: Vec2i,
    pub grab_shift: Vec2i,
    pub unicorn_shift: Vec2i,
    pub coordinate_source: CoordinateSource,
    pub memory_features: bool,
    pub pegasus_source: CoordinateSource,
    pub comet: bool,
    pub comet_jade: bool,
    pub comet_filter: LanternFilter,
    pub comet_placement: usize,
    pub comet_scale: f32,
    pub comet_x_percent: f32,
    pub comet_y_percent: f32,
    pub lanterns: bool,
    pub jade: bool,
    pub wisps: bool,
    pub lantern_key: u32,
    pub lantern_key_name: String,
    pub lantern_filter: LanternFilter,
    pub aurora_style: AuroraStyle,
    pub show_distance: bool,
    pub orbit_order: OrbitOrder,
    pub vertical_tracking: VerticalTracking,
    pub vertical_points: [bool; 4],
    pub orbit_filter: LanternFilter,
    pub prism_only: bool,
    pub opponent_color: Rgb,
    pub teammate_color: Rgb,
    pub outside_mission_color: Rgb,
    pub unknown_color: Rgb,
    pub unicorn_hsv: [HsvRange; 3],
    pub pegasus_hsv: HsvRange,
    pub unicorn: bool,
    pub unicorn_base_activation: UnicornActivation,
    pub unicorn_on_unicorn: bool,
    pub pegasus: bool,
    pub pegasus_on_red: bool,
    pub pegasus_on_green: bool,
    pub pegasus_activation: PegasusActivation,
    pub pegasus_activates_unicorn: bool,
    pub beesmas: bool,
    pub beesmas_type: BeesmasType,
    pub beesmas_value: f32,
    pub drift_guard: bool,
    pub ml_unicorn_enabled: bool,
    pub ml_unicorn_model: String,
    pub ml_unicorn_influence: f32,
    pub pegasus_threshold: i32,
    pub pegasus_delay_ms: u32,
    pub unicorn_key: u32,
    pub unicorn_key_name: String,
    pub pegasus_key: u32,
    pub pegasus_key_name: String,
    pub pause_key: u32,
    pub pause_key_name: String,
    pub aurora_key: u32,
    pub aurora_key_name: String,
    pub block_key: u32,
    pub block_key_name: String,
    pub discord_rich_presence: bool,
    pub highest_pixel: bool,
    pub downscale: bool,
    pub downscale_factor: f32,
    pub aurora_enabled: bool,
    pub lines: bool,
    pub lines_color: Rgb,
    pub unicorn_line_origin: UnicornLineOrigin,
    pub unicorn_target_box: bool,
    pub unicorn_target_color: Rgb,
    pub halo: bool,
    pub halo_color: Rgb,
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
    pub unicorn_distance: i32,
    pub tide_time_ms: u32,
    pub tide_idle_reset_ms: u32,
    pub tide_apply: TideApply,
    pub tide_ease: ArcCurve,
    pub velvet_charge: bool,
    pub velvet_distance: i32,
    pub velvet_build_ms: u32,
    pub velvet_effect: VelvetEffect,
    pub velvet_ease: ArcCurve,
    pub mouse_polling_rate: u32,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            com_port: "COM1".to_string(),
            monitor_resolution: Vec2i::new(1920, 1080),
            grab_resolution: Vec2i::new(500, 500),
            grab_shift: Vec2i::new(0, 0),
            unicorn_shift: Vec2i::new(0, 0),
            coordinate_source: CoordinateSource::ColorDetection,
            memory_features: true,
            pegasus_source: CoordinateSource::Memory,
            comet: false,
            comet_jade: false,
            comet_filter: LanternFilter::MissionPlayers,
            comet_placement: 0,
            comet_scale: 1.0,
            comet_x_percent: 50.0,
            comet_y_percent: 50.0,
            lanterns: false,
            jade: false,
            wisps: false,
            lantern_key: 121,
            lantern_key_name: "VK_F10".to_string(),
            lantern_filter: LanternFilter::Opponents,
            aurora_style: AuroraStyle::Boxes,
            show_distance: false,
            orbit_order: OrbitOrder::ClosestUnicorn,
            vertical_tracking: VerticalTracking::Current,
            vertical_points: [true; 4],
            orbit_filter: LanternFilter::Opponents,
            prism_only: true,
            opponent_color: Rgb::new(255, 70, 70),
            teammate_color: Rgb::new(70, 150, 255),
            outside_mission_color: Rgb::new(160, 160, 160),
            unknown_color: Rgb::new(255, 190, 60),
            unicorn_hsv: [
                HsvRange::new([105, 214, 240], [135, 244, 255]),
                HsvRange::new([105, 227, 140], [135, 255, 170]),
                HsvRange::new([105, 228, 160], [135, 255, 190]),
            ],
            pegasus_hsv: HsvRange::new([105, 227, 140], [135, 255, 170]),
            unicorn: true,
            unicorn_base_activation: UnicornActivation::Toggle,
            unicorn_on_unicorn: false,
            pegasus: true,
            pegasus_on_red: true,
            pegasus_on_green: false,
            pegasus_activation: PegasusActivation::Hold,
            pegasus_activates_unicorn: false,
            beesmas: false,
            beesmas_type: BeesmasType::Linear,
            beesmas_value: 2.5,
            drift_guard: false,
            ml_unicorn_enabled: false,
            ml_unicorn_model: String::new(),
            ml_unicorn_influence: 100.0,
            pegasus_threshold: 10,
            pegasus_delay_ms: 30,
            unicorn_key: 112,
            unicorn_key_name: "VK_F1".to_string(),
            pegasus_key: 113,
            pegasus_key_name: "VK_F2".to_string(),
            pause_key: 34,
            pause_key_name: "VK_NEXT".to_string(),
            aurora_key: 45,
            aurora_key_name: "VK_INSERT".to_string(),
            block_key: 114,
            block_key_name: "VK_F3".to_string(),
            discord_rich_presence: false,
            highest_pixel: false,
            downscale: false,
            downscale_factor: 0.5,
            aurora_enabled: false,
            lines: true,
            lines_color: Rgb::new(255, 255, 255),
            unicorn_line_origin: UnicornLineOrigin::Center,
            unicorn_target_box: true,
            unicorn_target_color: Rgb::new(255, 255, 255),
            halo: true,
            halo_color: Rgb::new(255, 255, 255),
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
            unicorn_distance: 50,
            tide_time_ms: 280,
            tide_idle_reset_ms: 30,
            tide_apply: TideApply::Delta,
            tide_ease: ArcCurve::Beesmascurve,
            velvet_charge: false,
            velvet_distance: 50,
            velvet_build_ms: 700,
            velvet_effect: VelvetEffect::Strength,
            velvet_ease: ArcCurve::Beesmasstep,
            mouse_polling_rate: 500,
        }
    }
}
impl ProfileConfig {
    pub fn eligibility(&self) -> FeatureEligibility {
        let memory = self.memory_features && crate::edition::memory_available();
        let memory_aim = self.unicorn && memory && self.coordinate_source == CoordinateSource::Memory;
        let color_aim = self.unicorn && self.coordinate_source == CoordinateSource::ColorDetection;
        let memory_trigger = self.pegasus && memory && self.pegasus_source == CoordinateSource::Memory;
        let color_trigger = self.pegasus && self.pegasus_source == CoordinateSource::ColorDetection;
        FeatureEligibility { memory, aim: memory_aim || color_aim, trigger: memory_trigger || color_trigger,
            memory_aim, color_aim, memory_trigger, color_trigger }
    }

    /// Preserve saved choices while removing consumers whose parents are unavailable.
    /// Sources never change implicitly: an unavailable selected source disables its consumer.
    pub fn effective_config(&self) -> Self {
        let eligible = self.eligibility();
        let mut effective = self.clone();
        effective.memory_features = eligible.memory;
        effective.unicorn = eligible.aim;
        effective.pegasus = eligible.trigger;
        if !eligible.memory {
            effective.lanterns = false;
            effective.comet = false;
            effective.jade = false;
            effective.wisps = false;
        }
        effective.comet_jade &= effective.comet;
        if !eligible.aim {
            effective.ml_unicorn_enabled = false;
            effective.drift_guard = false;
            effective.beesmas = false;
            effective.velvet_charge = false;
            effective.lines = false;
            effective.unicorn_target_box = false;
        }
        if !eligible.memory_aim {

            effective.orbit_order = OrbitOrder::ClosestUnicorn;
            effective.prism_only = false;
        }
        effective.pegasus_activates_unicorn &= eligible.aim && eligible.trigger && effective.pegasus_unicorn_supported();
        effective
    }

    pub fn validate_eligibility(&self) -> Result<(), String> {
        let eligible = self.eligibility();
        if eligible.color_aim && !matches!(self.vertical_tracking, VerticalTracking::Current | VerticalTracking::NoY) {
            return Err("Approximate body points require Memory aim. Select Current or No Y for Color aim.".into());
        }
        if eligible.memory_aim && self.vertical_tracking == VerticalTracking::ClosestPoint && !self.vertical_points.iter().any(|enabled| *enabled) {
            return Err("Closest Point requires at least one approximate point. Select Head, Neck, Torso or Feet.".into());
        }
        Ok(())
    }

    pub fn runtime_requirements(&self) -> RuntimeRequirements {
        let effective = self.effective_config();
        let eligibility = self.eligibility();
        let atlas_unicorn = eligibility.memory_aim;
        let memory_pegasus = eligibility.memory_trigger;
        let color_unicorn = eligibility.color_aim;
        let color_pegasus = eligibility.color_trigger;
        RuntimeRequirements {
            atlas_unicorn, memory_pegasus, color_unicorn, color_pegasus,
            orchid_atlas: atlas_unicorn || effective.lanterns || effective.comet || effective.jade || effective.wisps,
            screen_canvas: color_unicorn || color_pegasus || (effective.unicorn && effective.drift_guard),
            aurora: effective.aurora_enabled || effective.lanterns || effective.comet || effective.jade || effective.wisps,
            quill: effective.unicorn || effective.pegasus,
        }
    }

    pub fn pegasus_unicorn_supported(&self) -> bool {
        matches!(
            self.unicorn_base_activation,
            UnicornActivation::Hold | UnicornActivation::Timed
        ) && self.pegasus_activation == PegasusActivation::Hold
    }

    pub fn from_map(map: &HashMap<String, String>) -> Self {
        let mut cfg = Self::default();
        let parse_bool = |s: &str| -> bool {
            let s_lower = s.to_lowercase();
            s_lower == "true" || s_lower == "1" || s_lower == "yes" || s_lower == "on"
        };
        if let Some(value) = map.get("vertical_tracking") {
            cfg.vertical_tracking = match value.as_str() { "no_y" => VerticalTracking::NoY, "head" => VerticalTracking::Head, "neck" => VerticalTracking::Neck, "torso" => VerticalTracking::Torso, "feet" => VerticalTracking::Feet, "closest_point" => VerticalTracking::ClosestPoint, _ => VerticalTracking::Current };
        }
        for (index, key) in ["vertical_head", "vertical_neck", "vertical_torso", "vertical_feet"].iter().enumerate() {
            if let Some(value) = map.get(*key) { cfg.vertical_points[index] = parse_bool(value); }
        }


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



        if let Some(v) = map.get("com_port") {
            let port = v.trim();
            if !port.is_empty() {
                cfg.com_port = port.to_string();
            }
        }

        if let Some(v) = map.get("monires").and_then(|s| parse_vec2(s)) {
            cfg.monitor_resolution = v;
        }
        if let Some(v) = map.get("grab_resolution").and_then(|s| parse_vec2(s)) {
            cfg.grab_resolution = v;
        }
        if let Some(v) = map.get("grab_shift").and_then(|s| parse_vec2(s)) {
            cfg.grab_shift = v;
        }
        if let Some(v) = map.get("unicorn_shift").and_then(|s| parse_vec2(s)) {
            cfg.unicorn_shift = v;
        }
        if let Some(v) = map.get("memory_features") { cfg.memory_features = parse_bool(v); }
        if let Some(v) = map.get("coordinate_source") {
            cfg.coordinate_source = match v.as_str() {
                "memory" | "Memory" => CoordinateSource::Memory,
                "color" | "ColorDetection" | "Color Detection" => CoordinateSource::ColorDetection,
                _ => cfg.coordinate_source,
            };
        }
        if let Some(v) = map.get("pegasus_source") {
            cfg.pegasus_source = match v.as_str() {
                "memory" | "Memory" => CoordinateSource::Memory,
                "color" | "ColorDetection" | "Color Detection" => CoordinateSource::ColorDetection,
                _ => cfg.pegasus_source,
            };
        }

        if let Some(v) = map.get("comet_jade") { cfg.comet_jade = parse_bool(v); }
        if let Some(v) = map.get("comet") { cfg.comet = parse_bool(v); }
        if let Some(v) = map.get("comet_filter") {
            cfg.comet_filter = match v.as_str() {
                "opponents" => LanternFilter::Opponents,
                "all_players" => LanternFilter::AllPlayers,
                _ => LanternFilter::MissionPlayers,
            };
        }
        if let Some(v) = map.get("comet_placement").and_then(|v| v.parse::<usize>().ok()) {
            cfg.comet_placement = v.min(crate::comet::PLACEMENTS.len() - 1);
        }
        if let Some(v) = map.get("comet_scale").and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite()) {
            cfg.comet_scale = v.clamp(crate::comet::MIN_SCALE, crate::comet::MAX_SCALE);
        }
        for (key, value) in [("comet_x_percent", &mut cfg.comet_x_percent), ("comet_y_percent", &mut cfg.comet_y_percent)] {
            if let Some(v) = map.get(key).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite()) { *value = v.clamp(0.0, 100.0); }
        }
        if let Some(v) = map.get("lanterns") {
            cfg.lanterns = parse_bool(v);
        }
        if let Some(v) = map.get("jade") { cfg.jade = parse_bool(v); }
        if let Some(v) = map.get("wisps") { cfg.wisps = parse_bool(v); }
        if let Some(v) = map.get("aurora_style") {
            cfg.aurora_style = match v.as_str() {
                "dots" => AuroraStyle::Dots,
                _ => AuroraStyle::Boxes,
            };
        }
        if let Some(v) = map.get("show_distance") {
            cfg.show_distance = parse_bool(v);
        }
        if let Some(v) = map.get("orbit_order") {
            cfg.orbit_order = match v.as_str() {
                "closest_distance" => OrbitOrder::ClosestDistance,
                _ => OrbitOrder::ClosestUnicorn,
            };
        }
        if let Some(v) = map.get("orbit_filter") {
            cfg.orbit_filter = match v.as_str() {
                "mission_players" => LanternFilter::MissionPlayers,
                "all_players" => LanternFilter::AllPlayers,
                _ => LanternFilter::Opponents,
            };
        }
        if let Some(v) = map.get("prism_only") {
            cfg.prism_only = parse_bool(v);
        }
        if let Some(v) = map.get("lantern_key").and_then(|s| s.parse().ok()) {
            cfg.lantern_key = v;
        }
        if let Some(v) = map.get("lantern_key_name") {
            cfg.lantern_key_name = v.clone();
        }
        if let Some(v) = map.get("lantern_filter") {
            cfg.lantern_filter = match v.as_str() {
                "mission_players" => LanternFilter::MissionPlayers,
                "all_players" => LanternFilter::AllPlayers,
                _ => LanternFilter::Opponents,
            };
        }
        for (key, destination) in [
            ("opponent_color", &mut cfg.opponent_color),
            ("teammate_color", &mut cfg.teammate_color),
            ("outside_mission_color", &mut cfg.outside_mission_color),
            ("unknown_color", &mut cfg.unknown_color),
        ] {
            if let Some(value) = map.get(key).and_then(|s| parse_rgb(s)) {
                *destination = value;
            }
        }
        if let Some(v) = map.get("unicorn_hsv_lower").and_then(|s| parse_hsv(s)) {
            cfg.unicorn_hsv[0].lower = v;
        }
        if let Some(v) = map.get("unicorn_hsv_upper").and_then(|s| parse_hsv(s)) {
            cfg.unicorn_hsv[0].upper = v;
        }
        if let Some(v) = map.get("unicorn_hsv_lower_2").and_then(|s| parse_hsv(s)) {
            cfg.unicorn_hsv[1].lower = v;
        }
        if let Some(v) = map.get("unicorn_hsv_upper_2").and_then(|s| parse_hsv(s)) {
            cfg.unicorn_hsv[1].upper = v;
        }
        if let Some(v) = map.get("unicorn_hsv_lower_3").and_then(|s| parse_hsv(s)) {
            cfg.unicorn_hsv[2].lower = v;
        }
        if let Some(v) = map.get("unicorn_hsv_upper_3").and_then(|s| parse_hsv(s)) {
            cfg.unicorn_hsv[2].upper = v;
        }

        if let Some(v) = map.get("pegasus_hsv_lower").and_then(|s| parse_hsv(s)) {
            cfg.pegasus_hsv.lower = v;
        }
        if let Some(v) = map.get("pegasus_hsv_upper").and_then(|s| parse_hsv(s)) {
            cfg.pegasus_hsv.upper = v;
        }

        if let Some(v) = map.get("unicorn") {
            cfg.unicorn = parse_bool(v);
        }
        if let Some(v) = map.get("unicorn_base_activation") {
            cfg.unicorn_base_activation = match v.as_str() {
                "Toggle" => UnicornActivation::Toggle,
                "Hold" => UnicornActivation::Hold,
                "Timed" => UnicornActivation::Timed,
                _ => cfg.unicorn_base_activation,
            };
        }
        if let Some(v) = map.get("unicorn_on_unicorn") {
            cfg.unicorn_on_unicorn = parse_bool(v);
        }

        if let Some(v) = map.get("pegasus") {
            cfg.pegasus = parse_bool(v);
        }
        if let Some(v) = map.get("pegasus_on_red") {
            cfg.pegasus_on_red = parse_bool(v);
        }
        if let Some(v) = map.get("pegasus_on_green") {
            cfg.pegasus_on_green = parse_bool(v);
        }
        if let Some(v) = map.get("pegasus_activates_unicorn") {
            cfg.pegasus_activates_unicorn = parse_bool(v);
        }
        if let Some(v) = map.get("trigtype") {
            cfg.pegasus_activation = match v.as_str() {
                "Toggle" => PegasusActivation::Toggle,
                "Hold" => PegasusActivation::Hold,
                "DoubleCrouch" => PegasusActivation::DoubleCrouch,
                _ => cfg.pegasus_activation,
            };
        }

        if let Some(v) = map.get("Beesmas") {
            cfg.beesmas = parse_bool(v);
        }
        if let Some(v) = map.get("beesmastype") {
            cfg.beesmas_type = match v.as_str() {
                "Linear" => BeesmasType::Linear,
                "Logarithmic" => BeesmasType::Logarithmic,
                "Hyperbolic" => BeesmasType::Hyperbolic,
                "Bezier" => BeesmasType::Bezier,
                _ => cfg.beesmas_type,
            };
        }
        if let Some(v) = map.get("beesmas_value").and_then(|s| s.parse().ok()) {
            cfg.beesmas_value = v;
        }
        if let Some(v) = map.get("PredictiveBraking") {
            cfg.drift_guard = parse_bool(v);
        }
        if let Some(v) = map.get("MLUnicornEnabled") {
            cfg.ml_unicorn_enabled = parse_bool(v);
        }
        if let Some(v) = map.get("MLUnicornModel") {
            cfg.ml_unicorn_model = v.clone();
        }
        if let Some(v) = map.get("MLUnicornInfluence").and_then(|s| s.parse().ok()) {
            cfg.ml_unicorn_influence = v;
        }
        if let Some(v) = map.get("trigthresh").and_then(|s| s.parse().ok()) {
            cfg.pegasus_threshold = v;
        }
        if let Some(v) = map.get("delay").and_then(|s| s.parse().ok()) {
            cfg.pegasus_delay_ms = v;
        }

        if let Some(v) = map.get("unicornkey").and_then(|s| s.parse().ok()) {
            cfg.unicorn_key = v;
        }
        if let Some(v) = map.get("unicornkeyname") {
            cfg.unicorn_key_name = v.clone();
        }
        if let Some(v) = map.get("trigkey").and_then(|s| s.parse().ok()) {
            cfg.pegasus_key = v;
        }
        if let Some(v) = map.get("trigkeyname") {
            cfg.pegasus_key_name = v.clone();
        }
        if let Some(v) = map.get("pausekey").and_then(|s| s.parse().ok()) {
            cfg.pause_key = v;
        }
        if let Some(v) = map.get("pausekeyname") {
            cfg.pause_key_name = v.clone();
        }
        if let Some(v) = map.get("viskey").and_then(|s| s.parse().ok()) {
            cfg.aurora_key = v;
        }
        if let Some(v) = map.get("viskeyname") {
            cfg.aurora_key_name = v.clone();
        }
        if let Some(v) = map.get("blockkey").and_then(|s| s.parse().ok()) {
            cfg.block_key = v;
        }
        if let Some(v) = map.get("blockkeyname") {
            cfg.block_key_name = v.clone();
        }

        if let Some(v) = map.get("discord_rich_presence") {
            cfg.discord_rich_presence = parse_bool(v);
        }
        if let Some(v) = map.get("highest_pixel") {
            cfg.highest_pixel = parse_bool(v);
        }
        if let Some(v) = map.get("downscale") {
            cfg.downscale = parse_bool(v);
        }
        if let Some(v) = map.get("downscale_factor").and_then(|s| s.parse().ok()) {
            cfg.downscale_factor = v;
        }
        if let Some(v) = map.get("aurora_enabled") {
            cfg.aurora_enabled = parse_bool(v);
        }
        if let Some(v) = map.get("lines") {
            cfg.lines = parse_bool(v);
        }
        if let Some(v) = map.get("lines_col").and_then(|s| parse_rgb(s)) {
            cfg.lines_color = v;
        }
        if let Some(v) = map.get("halo") {
            cfg.halo = parse_bool(v);
        }
        if let Some(v) = map.get("halo_col").and_then(|s| parse_rgb(s)) {
            cfg.halo_color = v;
        }
        // Older profiles used the FOV flag/color for the target box too.
        cfg.unicorn_target_box = map
            .get("unicorn_target_box")
            .map(|s| parse_bool(s))
            .unwrap_or(cfg.halo);
        cfg.unicorn_target_color = map
            .get("unicorn_target_color")
            .and_then(|s| parse_rgb(s))
            .unwrap_or(cfg.halo_color);
        if let Some(v) = map.get("unicorn_line_origin") {
            cfg.unicorn_line_origin = match v.as_str() {
                "top" => UnicornLineOrigin::Top,
                _ => UnicornLineOrigin::Center,
            };
        }
        if let Some(v) = map.get("ui_col").and_then(|s| parse_rgb(s)) {
            cfg.ui_color = v;
        }
        if let Some(v) = map.get("xonly") {
            cfg.x_only = parse_bool(v);
        }
        if let Some(v) = map.get("fps").and_then(|s| s.parse().ok()) {
            cfg.fps = v;
        }

        if let Some(v) = map.get("rect_filter") {
            cfg.rect_filter = parse_bool(v);
        }
        if let Some(v) = map.get("rect_compare") {
            cfg.rect_compare = match v.as_str() {
                "Greater than" => RectCompare::GreaterThan,
                "Less than" => RectCompare::LessThan,
                _ => cfg.rect_compare,
            };
        }
        if let Some(v) = map.get("rect_ratio").and_then(|s| s.parse().ok()) {
            cfg.rect_ratio = v;
        }
        if let Some(v) = map.get("area_filter") {
            cfg.area_filter = parse_bool(v);
        }
        if let Some(v) = map.get("area_min").and_then(|s| s.parse().ok()) {
            cfg.area_min = v;
        }
        if let Some(v) = map.get("area_max").and_then(|s| s.parse().ok()) {
            cfg.area_max = v;
        }

        if let Some(v) = map.get("col_mode") {
            cfg.pixel_acquisition = match v.as_str() {
                "contouring" => PixelAcquisition::Contouring,
                "edging" => PixelAcquisition::Edging,
                _ => cfg.pixel_acquisition,
            };
        }
        if let Some(v) = map.get("apbpath") {
            cfg.apb_path = v.clone();
        }
        if let Some(v) = map.get("distance_threshold").and_then(|s| s.parse().ok()) {
            cfg.merge_distance = v;
        }
        if let Some(v) = map
            .get("unicorn_distance_threshold")
            .and_then(|s| s.parse().ok())
        {
            cfg.unicorn_distance = v;
        }

        if let Some(v) = map.get("tide_time_ms").and_then(|s| s.parse().ok()) {
            cfg.tide_time_ms = v;
        }
        if let Some(v) = map.get("tide_idle_reset_ms").and_then(|s| s.parse().ok()) {
            cfg.tide_idle_reset_ms = v;
        }
        if let Some(v) = map.get("tide_apply") {
            cfg.tide_apply = match v.as_str() {
                "delta" => TideApply::Delta,
                "beesmas_value" => TideApply::BeesmasValue,
                _ => cfg.tide_apply,
            };
        }
        if let Some(v) = map.get("tide_ease") {
            cfg.tide_ease = match v.as_str() {
                "linear" => ArcCurve::Linear,
                "beesmasstep" => ArcCurve::Beesmasstep,
                "beesmascurve" => ArcCurve::Beesmascurve,
                "quad" => ArcCurve::Quad,
                "cubic" => ArcCurve::Cubic,
                _ => cfg.tide_ease,
            };
        }

        if let Some(v) = map.get("velvet_charge") {
            cfg.velvet_charge = parse_bool(v);
        }
        if let Some(v) = map.get("velvet_distance").and_then(|s| s.parse().ok()) {
            cfg.velvet_distance = v;
        }
        if let Some(v) = map.get("velvet_build_ms").and_then(|s| s.parse().ok()) {
            cfg.velvet_build_ms = v;
        }
        if let Some(v) = map.get("velvet_effect") {
            cfg.velvet_effect = match v.as_str() {
                "strength" => VelvetEffect::Strength,
                "beesmas_value" => VelvetEffect::BeesmasValue,
                _ => cfg.velvet_effect,
            };
        }
        if let Some(v) = map.get("velvet_ease") {
            cfg.velvet_ease = match v.as_str() {
                "linear" => ArcCurve::Linear,
                "beesmasstep" => ArcCurve::Beesmasstep,
                "beesmascurve" => ArcCurve::Beesmascurve,
                "quad" => ArcCurve::Quad,
                "cubic" => ArcCurve::Cubic,
                _ => cfg.velvet_ease,
            };
        }

        if let Some(v) = map.get("mouse_polling_rate").and_then(|s| s.parse().ok()) {
            cfg.mouse_polling_rate = v;
        }

        cfg
    }

    pub fn to_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("com_port".to_string(), self.com_port.clone());

        let format_vec2 = |v: Vec2i| -> String { format!("{}, {}", v.x, v.y) };
        let format_hsv = |v: [u8; 3]| -> String { format!("{}, {}, {}", v[0], v[1], v[2]) };
        let format_rgb = |v: Rgb| -> String { format!("({},{},{})", v.r, v.g, v.b) };
        let format_bool = |v: bool| -> String {
            if v {
                "True".to_string()
            } else {
                "False".to_string()
            }
        };

        map.insert(
            "lanterns".into(),
            format_bool(self.lanterns),
        );
        map.insert("memory_features".into(), format_bool(self.memory_features));
        map.insert("vertical_tracking".into(), self.vertical_tracking.key().into());
        for (key, enabled) in ["vertical_head", "vertical_neck", "vertical_torso", "vertical_feet"].into_iter().zip(self.vertical_points) {
            map.insert(key.into(), format_bool(enabled));
        }
        map.insert("comet_jade".into(), format_bool(self.comet_jade));
        map.insert("comet".into(), format_bool(self.comet));
        map.insert("jade".into(), format_bool(self.jade));
        map.insert("wisps".into(), format_bool(self.wisps));
        map.insert("comet_filter".into(), match self.comet_filter {
            LanternFilter::Opponents => "opponents",
            LanternFilter::MissionPlayers => "mission_players",
            LanternFilter::AllPlayers => "all_players",
        }.into());
        map.insert("comet_placement".into(), self.comet_placement.to_string());
        map.insert("comet_scale".into(), self.comet_scale.to_string());
        map.insert("comet_x_percent".into(), self.comet_x_percent.to_string());
        map.insert("comet_y_percent".into(), self.comet_y_percent.to_string());
        map.insert("lantern_key".into(), self.lantern_key.to_string());
        map.insert(
            "aurora_style".into(),
            match self.aurora_style {
                AuroraStyle::Boxes => "boxes",
                AuroraStyle::Dots => "dots",
            }
            .into(),
        );
        map.insert("show_distance".into(), format_bool(self.show_distance));
        map.insert(
            "orbit_filter".into(),
            match self.orbit_filter {
                LanternFilter::Opponents => "opponents",
                LanternFilter::MissionPlayers => "mission_players",
                LanternFilter::AllPlayers => "all_players",
            }
            .into(),
        );
        map.insert(
            "orbit_order".into(),
            match self.orbit_order {
                OrbitOrder::ClosestUnicorn => "closest_unicorn",
                OrbitOrder::ClosestDistance => "closest_distance",
            }
            .into(),
        );
        map.insert("prism_only".into(), self.prism_only.to_string());
        map.insert(
            "lantern_key_name".into(),
            self.lantern_key_name.clone(),
        );
        map.insert(
            "lantern_filter".into(),
            match self.lantern_filter {
                LanternFilter::Opponents => "opponents",
                LanternFilter::MissionPlayers => "mission_players",
                LanternFilter::AllPlayers => "all_players",
            }
            .into(),
        );
        for (key, color) in [
            ("opponent_color", self.opponent_color),
            ("teammate_color", self.teammate_color),
            ("outside_mission_color", self.outside_mission_color),
            ("unknown_color", self.unknown_color),
        ] {
            map.insert(key.into(), format_rgb(color));
        }

        map.insert("monires".to_string(), format_vec2(self.monitor_resolution));
        map.insert(
            "grab_resolution".to_string(),
            format_vec2(self.grab_resolution),
        );
        map.insert("grab_shift".to_string(), format_vec2(self.grab_shift));
        map.insert("unicorn_shift".to_string(), format_vec2(self.unicorn_shift));
        map.insert(
            "coordinate_source".to_string(),
            match self.coordinate_source {
                CoordinateSource::ColorDetection => "color".to_string(),
                CoordinateSource::Memory => "memory".to_string(),
            },
        );
        map.insert(
            "pegasus_source".to_string(),
            match self.pegasus_source {
                CoordinateSource::ColorDetection => "color".to_string(),
                CoordinateSource::Memory => "memory".to_string(),
            },
        );

        map.insert(
            "unicorn_hsv_lower".to_string(),
            format_hsv(self.unicorn_hsv[0].lower),
        );
        map.insert(
            "unicorn_hsv_upper".to_string(),
            format_hsv(self.unicorn_hsv[0].upper),
        );
        map.insert(
            "unicorn_hsv_lower_2".to_string(),
            format_hsv(self.unicorn_hsv[1].lower),
        );
        map.insert(
            "unicorn_hsv_upper_2".to_string(),
            format_hsv(self.unicorn_hsv[1].upper),
        );
        map.insert(
            "unicorn_hsv_lower_3".to_string(),
            format_hsv(self.unicorn_hsv[2].lower),
        );
        map.insert(
            "unicorn_hsv_upper_3".to_string(),
            format_hsv(self.unicorn_hsv[2].upper),
        );

        map.insert(
            "pegasus_hsv_lower".to_string(),
            format_hsv(self.pegasus_hsv.lower),
        );
        map.insert(
            "pegasus_hsv_upper".to_string(),
            format_hsv(self.pegasus_hsv.upper),
        );

        map.insert("unicorn".to_string(), format_bool(self.unicorn));
        map.insert(
            "unicorn_base_activation".to_string(),
            match self.unicorn_base_activation {
                UnicornActivation::Toggle => "Toggle".to_string(),
                UnicornActivation::Hold => "Hold".to_string(),
                UnicornActivation::Timed => "Timed".to_string(),
            },
        );
        map.insert(
            "unicorntype".to_string(),
            match self.unicorn_base_activation {
                UnicornActivation::Toggle => "Toggle".to_string(),
                UnicornActivation::Hold => "Hold".to_string(),
                UnicornActivation::Timed => "Timed".to_string(),
            },
        );
        map.insert("unicorn_on_unicorn".to_string(), format_bool(self.unicorn_on_unicorn));

        map.insert("pegasus".to_string(), format_bool(self.pegasus));
        map.insert(
            "pegasus_activates_unicorn".into(),
            format_bool(self.pegasus_activates_unicorn),
        );
        map.insert(
            "pegasus_on_red".to_string(),
            format_bool(self.pegasus_on_red),
        );
        map.insert(
            "pegasus_on_green".to_string(),
            format_bool(self.pegasus_on_green),
        );
        map.insert(
            "trigtype".to_string(),
            match self.pegasus_activation {
                PegasusActivation::Toggle => "Toggle".to_string(),
                PegasusActivation::Hold => "Hold".to_string(),
                PegasusActivation::DoubleCrouch => "DoubleCrouch".to_string(),
            },
        );

        map.insert("Beesmas".to_string(), format_bool(self.beesmas));
        map.insert(
            "beesmastype".to_string(),
            match self.beesmas_type {
                BeesmasType::Linear => "Linear".to_string(),
                BeesmasType::Logarithmic => "Logarithmic".to_string(),
                BeesmasType::Hyperbolic => "Hyperbolic".to_string(),
                BeesmasType::Bezier => "Bezier".to_string(),
            },
        );
        map.insert("beesmas_value".to_string(), self.beesmas_value.to_string());
        map.insert(
            "PredictiveBraking".to_string(),
            format_bool(self.drift_guard),
        );
        map.insert("trigthresh".to_string(), self.pegasus_threshold.to_string());
        map.insert("MLUnicornEnabled".into(), format_bool(self.ml_unicorn_enabled));
        map.insert("MLUnicornModel".into(), self.ml_unicorn_model.clone());
        map.insert("MLUnicornInfluence".into(), self.ml_unicorn_influence.to_string());
        map.insert("delay".to_string(), self.pegasus_delay_ms.to_string());

        map.insert("unicornkey".to_string(), self.unicorn_key.to_string());
        map.insert("unicornkeyname".to_string(), self.unicorn_key_name.clone());
        map.insert("trigkey".to_string(), self.pegasus_key.to_string());
        map.insert("trigkeyname".to_string(), self.pegasus_key_name.clone());
        map.insert("pausekey".to_string(), self.pause_key.to_string());
        map.insert("pausekeyname".to_string(), self.pause_key_name.clone());
        map.insert("viskey".to_string(), self.aurora_key.to_string());
        map.insert("viskeyname".to_string(), self.aurora_key_name.clone());
        map.insert("blockkey".to_string(), self.block_key.to_string());
        map.insert("blockkeyname".to_string(), self.block_key_name.clone());

        map.insert(
            "discord_rich_presence".to_string(),
            format_bool(self.discord_rich_presence),
        );
        map.insert("highest_pixel".to_string(), format_bool(self.highest_pixel));
        map.insert("downscale".to_string(), format_bool(self.downscale));
        map.insert(
            "downscale_factor".to_string(),
            self.downscale_factor.to_string(),
        );
        map.insert("aurora_enabled".to_string(), format_bool(self.aurora_enabled));
        map.insert("lines".to_string(), format_bool(self.lines));
        map.insert("lines_col".to_string(), format_rgb(self.lines_color));
        map.insert(
            "unicorn_line_origin".into(),
            match self.unicorn_line_origin {
                UnicornLineOrigin::Center => "center",
                UnicornLineOrigin::Top => "top",
            }
            .into(),
        );
        map.insert("unicorn_target_box".into(), format_bool(self.unicorn_target_box));
        map.insert("unicorn_target_color".into(), format_rgb(self.unicorn_target_color));
        map.insert("halo".to_string(), format_bool(self.halo));
        map.insert("halo_col".to_string(), format_rgb(self.halo_color));
        map.insert("ui_col".to_string(), format_rgb(self.ui_color));
        map.insert("xonly".to_string(), format_bool(self.x_only));
        map.insert("fps".to_string(), self.fps.to_string());

        map.insert("rect_filter".to_string(), format_bool(self.rect_filter));
        map.insert(
            "rect_compare".to_string(),
            match self.rect_compare {
                RectCompare::GreaterThan => "Greater than".to_string(),
                RectCompare::LessThan => "Less than".to_string(),
            },
        );
        map.insert("rect_ratio".to_string(), self.rect_ratio.to_string());
        map.insert("area_filter".to_string(), format_bool(self.area_filter));
        map.insert("area_min".to_string(), self.area_min.to_string());
        map.insert("area_max".to_string(), self.area_max.to_string());

        map.insert(
            "col_mode".to_string(),
            match self.pixel_acquisition {
                PixelAcquisition::Contouring => "contouring".to_string(),
                PixelAcquisition::Edging => "edging".to_string(),
            },
        );
        map.insert("apbpath".to_string(), self.apb_path.clone());
        map.insert(
            "distance_threshold".to_string(),
            self.merge_distance.to_string(),
        );
        map.insert(
            "unicorn_distance_threshold".to_string(),
            self.unicorn_distance.to_string(),
        );

        map.insert("tide_time_ms".to_string(), self.tide_time_ms.to_string());
        map.insert(
            "tide_idle_reset_ms".to_string(),
            self.tide_idle_reset_ms.to_string(),
        );
        map.insert(
            "tide_apply".to_string(),
            match self.tide_apply {
                TideApply::Delta => "delta".to_string(),
                TideApply::BeesmasValue => "beesmas_value".to_string(),
            },
        );
        map.insert(
            "tide_ease".to_string(),
            match self.tide_ease {
                ArcCurve::Linear => "linear".to_string(),
                ArcCurve::Beesmasstep => "beesmasstep".to_string(),
                ArcCurve::Beesmascurve => "beesmascurve".to_string(),
                ArcCurve::Quad => "quad".to_string(),
                ArcCurve::Cubic => "cubic".to_string(),
            },
        );

        map.insert(
            "velvet_charge".to_string(),
            format_bool(self.velvet_charge),
        );
        map.insert(
            "velvet_distance".to_string(),
            self.velvet_distance.to_string(),
        );
        map.insert(
            "velvet_build_ms".to_string(),
            self.velvet_build_ms.to_string(),
        );
        map.insert(
            "velvet_effect".to_string(),
            match self.velvet_effect {
                VelvetEffect::Strength => "strength".to_string(),
                VelvetEffect::BeesmasValue => "beesmas_value".to_string(),
            },
        );
        map.insert(
            "velvet_ease".to_string(),
            match self.velvet_ease {
                ArcCurve::Linear => "linear".to_string(),
                ArcCurve::Beesmasstep => "beesmasstep".to_string(),
                ArcCurve::Beesmascurve => "beesmascurve".to_string(),
                ArcCurve::Quad => "quad".to_string(),
                ArcCurve::Cubic => "cubic".to_string(),
            },
        );

        map.insert(
            "mouse_polling_rate".to_string(),
            self.mouse_polling_rate.to_string(),
        );

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
                current_section = line[1..line.len() - 1].trim().to_string();
                continue;
            }
            if let Some(pos) = line.find('=') {
                let key = line[..pos].trim().to_string();
                let value = line[pos + 1..].trim().to_string();
                sections
                    .entry(current_section.clone())
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
    PathBuf::from(home)
        .join("Documents")
        .join(crate::passport::APP_DIRECTORY)
        .join("config.ini")
}

pub fn load_profile(name: &str) -> std::io::Result<Option<ProfileConfig>> {
    let sections = crate::folio_store::read(&get_config_path())?;
    Ok(sections.get(name).map(ProfileConfig::from_map))
}

pub fn save_profile(name: &str, config: &ProfileConfig) -> std::io::Result<()> {
    let path = get_config_path();
    if name.trim().is_empty() { return Err(std::io::Error::other("Profile name cannot be empty.")); }
    let mut sections = crate::folio_store::read(&path)?;
    sections.insert(name.to_string(), config.to_map());
    crate::folio_store::write(&path, &sections)
}

pub fn get_profile_names() -> std::io::Result<Vec<String>> {
    let sections = crate::folio_store::read(&get_config_path())?;
    let mut names: Vec<String> = sections.keys().cloned().collect();
    if !names.contains(&"default".to_string()) {
        names.push("default".to_string());
    }
    names.sort();
    Ok(names)
}

pub fn get_login_path() -> PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\".to_string());
    PathBuf::from(home)
        .join("Documents")
        .join(crate::passport::APP_DIRECTORY)
        .join("login.ini")
}

fn get_protected_passport_path() -> PathBuf {
    get_login_path()
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(crate::protocol_names::passport_file())
}

pub fn load_passport_key() -> String {
    let protected_path = get_protected_passport_path();
    if let Ok(wrapped) = fs::read(&protected_path) {
        if let Ok(plaintext) = crate::passport::dpapi_unprotect(&wrapped) {
            if let Ok(key) = String::from_utf8(plaintext) {
                return key.trim().to_string();
            }
        }
        return String::new();
    }

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
            let key = key.trim().to_string();
            if !key.is_empty() {
                let _ = save_passport_key(&key);
            }
            return key;
        }
    }
    String::new()
}

pub fn save_passport_key(key: &str) -> std::io::Result<()> {
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
    let protected = crate::passport::dpapi_protect(key.trim().as_bytes(), "Verux License Key")
        .map_err(std::io::Error::other)?;
    fs::write(get_protected_passport_path(), protected)?;

    let mut login_section = ini.sections.remove("login").unwrap_or_default();
    login_section.remove("license_key");
    if !login_section.is_empty() {
        ini.sections.insert("login".to_string(), login_section);
    }
    let serialized = ini.serialize();
    fs::write(&path, serialized)?;
    Ok(())
}

const DEFAULT_UI_INI: &[u8] = include_bytes!("../assets/src_ini/DefaultUI.ini");
const DEFAULT_GAME_INI: &[u8] = include_bytes!("../assets/src_ini/DefaultGame.ini");
const APB_COMPAT_INI: &[u8] = include_bytes!("../assets/src_ini/APBCompat.ini");
const BASE_UI_INI: &[u8] = include_bytes!("../assets/src_ini/BaseUI.ini");
const APB_UBER_SHADERS: &[u8] =
    include_bytes!("../assets/src_ini/APBUberPostProcessBlendPixelShader.usf");
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
    fs::write(apb_config_dir.join("DefaultGame.ini"), DEFAULT_GAME_INI)?;
    fs::write(apb_config_dir.join("APBCompat.ini"), APB_COMPAT_INI)?;
    fs::write(engine_config_dir.join("BaseUI.ini"), BASE_UI_INI)?;
    fs::write(
        engine_shaders_dir.join("APBUberPostProcessBlendPixelShader.usf"),
        APB_UBER_SHADERS,
    )?;

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
    fs::write(apb_config_dir.join("DefaultGame.ini"), DEFAULT_GAME_INI)?;
    fs::write(apb_config_dir.join("APBCompat.ini"), APB_COMPAT_INI)?;
    fs::write(engine_config_dir.join("BaseUI.ini"), BASE_UI_INI)?;
    fs::write(
        engine_shaders_dir.join("APBUberPostProcessBlendPixelShader.usf"),
        APB_UBER_SHADERS,
    )?;
    fs::write(
        engine_shaders_dir.join("BasePassPixelShader.usf"),
        BASE_PASS_SHADERS,
    )?;
    fs::write(
        engine_shaders_dir.join("GpuSkinVertexFactory.usf"),
        GPU_SKIN_SHADERS,
    )?;

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
            config.unicorn_hsv[0],
            HsvRange::new([105, 214, 240], [135, 244, 255])
        );
        assert_eq!(
            config.pegasus_hsv,
            HsvRange::new([105, 227, 140], [135, 255, 170])
        );
        assert_eq!(config.unicorn_base_activation, UnicornActivation::Toggle);
        assert_eq!(config.pegasus_activation, PegasusActivation::Hold);
        assert_eq!(config.beesmas_value, 2.5);
        assert_eq!(config.tide_time_ms, 280);
        assert_eq!(config.tide_idle_reset_ms, 30);
        assert_eq!(config.velvet_ease, ArcCurve::Beesmasstep);
    }

    #[test]
    fn test_ini_serialization_roundtrip() {
        let mut config = ProfileConfig::default();
        config.beesmas_value = 4.2;
        config.unicorn_key_name = "VK_XBUTTON1".to_string();
        config.lines_color = Rgb::new(10, 20, 30);

        let map = config.to_map();
        let parsed = ProfileConfig::from_map(&map);

        assert_eq!(parsed.beesmas_value, 4.2);
        assert_eq!(parsed.unicorn_key_name, "VK_XBUTTON1");
        assert_eq!(parsed.lines_color, Rgb::new(10, 20, 30));
        assert_eq!(parsed.monitor_resolution, Vec2i::new(1920, 1080));
    }

    #[test]
    fn test_ini_file_parsing() {
        let content = "
[default]
beesmas_value = 3.5
unicornkeyname = VK_F5

[custom_profile]
beesmas_value = 5.0
unicornkeyname = VK_F6
";
        let ini = IniFile::parse(content);
        assert_eq!(ini.sections.len(), 2);

        let def = ini.sections.get("default").unwrap();
        assert_eq!(def.get("beesmas_value").unwrap(), "3.5");
        assert_eq!(def.get("unicornkeyname").unwrap(), "VK_F5");

        let cust = ini.sections.get("custom_profile").unwrap();
        assert_eq!(cust.get("beesmas_value").unwrap(), "5.0");
        assert_eq!(cust.get("unicornkeyname").unwrap(), "VK_F6");
    }
}
