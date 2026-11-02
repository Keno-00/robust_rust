//! User-defined configuration recommendations, not detection guarantees.
use crate::config::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Risk {
    White,
    Yellow,
    Red,
}
use Risk::*;

pub(crate) const IDS: &[&str] = &[
    "u082",
    "u083",
    "u084",
    "u085",
    "u088",
    "u136",
    "u087",
    "u065",
    "u066",
    "u067",
    "u068",
    "u069",
    "u070",
    "u071",
    "u075",
    "u077",
    "u078",
    "u131",
    "u132",
    "u053",
    "u054",
    "u055",
    "u058",
    "u061",
    "u115",
    "u032",
    "u034",
    "u036",
    "u038",
    "u096",
    "u133",
    "u134",
    "u043",
    "v_radar_objects",
];
fn on(value: bool) -> Risk {
    if value {
        Red
    } else {
        White
    }
}
fn ramp(value: u32) -> Risk {
    if value < 250 {
        Red
    } else if value <= 300 {
        Yellow
    } else {
        White
    }
}
fn mouse(key: u32) -> bool {
    key == 1 || key == 2
}

pub(crate) fn risk(c: &ProfileConfig, id: &str) -> Risk {
    let e = c.eligibility();
    match id {
        "u082" if e.aim => [
            "u083", "u085", "u088", "u136", "u087", "u065", "u066", "u067", "u068", "u069", "u070",
            "u071", "u075", "u077", "u078", "u131", "u132",
        ]
        .into_iter()
        .map(|id| risk(c, id))
        .max()
        .unwrap_or(White),
        "u053" if e.trigger => [risk(c, "u054"), risk(c, "u058"), risk(c, "u061")]
            .into_iter()
            .max()
            .unwrap_or(White),
        "u083" if e.aim => on(c.coordinate_source == CoordinateSource::Memory),
        "u054" if e.trigger => on(c.pegasus_source == CoordinateSource::Memory),
        "u084" | "u085" if e.aim => {
            if mouse(c.unicorn_key) || c.unicorn_base_activation == UnicornActivation::Toggle {
                White
            } else {
                Yellow
            }
        }
        "u055" | "u058" if e.trigger => match c.pegasus_activation {
            PegasusActivation::DoubleCrouch => White,
            PegasusActivation::Hold if c.pegasus_key == 2 => White,
            _ => Yellow,
        },
        "u088" if e.memory_aim => {
            if c.orbit_filter == LanternFilter::AllPlayers {
                Yellow
            } else {
                White
            }
        }
        "u087" if e.memory_aim => on(!c.prism_only),
        "u136" if e.aim => match c.vertical_tracking {
            VerticalTracking::NoY => White,
            VerticalTracking::Current => Yellow,
            VerticalTracking::ClosestPoint
                if c.vertical_points.iter().filter(|v| **v).count() == 4 =>
            {
                Yellow
            }
            _ => Red,
        },
        "u065" if e.aim => on(!c.beesmas),
        "u066" if e.aim && c.beesmas => match c.beesmas_type {
            BeesmasType::Linear | BeesmasType::Logarithmic => Red,
            BeesmasType::Hyperbolic => Yellow,
            BeesmasType::Bezier => White,
        },
        "u067" if e.aim && c.beesmas => {
            if !c.beesmas_value.is_finite() || c.beesmas_value < 2.0 {
                Red
            } else if c.beesmas_value <= 3.0 {
                Yellow
            } else {
                White
            }
        }
        "u068" if e.aim => on(!c.drift_guard),
        "u069" if e.aim => {
            if c.tide_time_ms == 0 {
                Yellow
            } else {
                White
            }
        }
        "u070" if e.aim && c.tide_time_ms > 0 => ramp(c.tide_time_ms),
        "u071" if e.aim && c.tide_time_ms > 0 => {
            if c.tide_idle_reset_ms < 50 {
                Yellow
            } else {
                White
            }
        }
        "u075" if e.aim && c.velvet_charge => Yellow,
        "u077" if e.aim && c.velvet_charge => ramp(c.velvet_build_ms),
        "u078" if e.aim && c.velvet_charge => Yellow,
        "u131" if e.aim => on(!c.ml_unicorn_enabled),
        "u132" if e.aim && c.ml_unicorn_enabled => {
            if !c.ml_unicorn_influence.is_finite() || c.ml_unicorn_influence < 50.0 {
                Red
            } else if c.ml_unicorn_influence < 90.0 {
                Yellow
            } else {
                White
            }
        }
        "u061" if e.trigger => {
            if c.pegasus_delay_ms <= 80 {
                Red
            } else if c.pegasus_delay_ms <= 125 {
                Yellow
            } else {
                White
            }
        }
        "u115" => on(e.memory),
        "u032" => on(c.aurora_enabled),
        "u034" => on(c.aurora_enabled && c.halo),
        "u036" => on(c.aurora_enabled && e.aim && c.unicorn_target_box),
        "u038" => on(c.aurora_enabled && e.aim && c.lines),
        "u096" => on(e.memory && c.lanterns),
        "u133" => on(e.memory && c.jade),
        "u134" => on(e.memory && c.wisps),
        "u043" => on(e.memory && c.comet),
        "v_radar_objects" => on(e.memory && c.comet && c.comet_jade),
        _ => White,
    }
}
pub(crate) fn draw<T>(ui: &imgui::Ui, enabled: bool, risk: Risk, f: impl FnOnce() -> T) -> T {
    let color = if enabled {
        match risk {
            Red => [1.0, 0.27, 0.27, 1.0],
            Yellow => [1.0, 0.8, 0.2, 1.0],
            White => [1.0; 4],
        }
    } else {
        ui.style_color(imgui::StyleColor::Text)
    };
    let _text = ui.push_style_color(imgui::StyleColor::Text, color);
    f()
}
