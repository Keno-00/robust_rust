// Build-specific legacy import aliases: reversible masking, not a security boundary.
const ALIASES: &[(&[u8], &str)] = &[
    (
        &crate::native_text::from_legacy([
            234, 235, 230, 206, 202, 226, 201, 198, 197, 203, 194, 195,
        ]),
        "MLUnicornEnabled",
    ),
    (
        &crate::native_text::from_legacy([
            234, 235, 230, 206, 202, 238, 201, 193, 203, 210, 194, 201, 196, 194,
        ]),
        "MLUnicornInfluence",
    ),
    (
        &crate::native_text::from_legacy([234, 235, 230, 206, 202, 234, 200, 195, 194, 203]),
        "MLUnicornModel",
    ),
    (
        &crate::native_text::from_legacy([244, 202, 200, 200, 211, 207, 197, 200, 211]),
        "Beesmas",
    ),
    (
        &crate::native_text::from_legacy([
            198, 206, 202, 248, 197, 198, 212, 194, 248, 198, 196, 211, 206, 209, 198, 211, 206,
            200, 201,
        ]),
        "unicorn_base_activation",
    ),
    (
        &crate::native_text::from_legacy([
            198, 206, 202, 248, 195, 206, 212, 211, 198, 201, 196, 194, 248, 211, 207, 213, 194,
            212, 207, 200, 203, 195,
        ]),
        "unicorn_distance_threshold",
    ),
    (
        &crate::native_text::from_legacy([
            198, 206, 202, 248, 203, 206, 201, 194, 248, 200, 213, 206, 192, 206, 201,
        ]),
        "unicorn_line_origin",
    ),
    (
        &crate::native_text::from_legacy([198, 206, 202, 248, 200, 201, 248, 198, 206, 202]),
        "unicorn_on_unicorn",
    ),
    (
        &crate::native_text::from_legacy([198, 206, 202, 248, 212, 207, 206, 193, 211]),
        "unicorn_shift",
    ),
    (
        &crate::native_text::from_legacy([
            198, 206, 202, 248, 211, 198, 213, 192, 194, 211, 248, 197, 200, 223,
        ]),
        "unicorn_target_box",
    ),
    (
        &crate::native_text::from_legacy([
            198, 206, 202, 248, 211, 198, 213, 192, 194, 211, 248, 196, 200, 203, 200, 213,
        ]),
        "unicorn_target_color",
    ),
    (
        &crate::native_text::from_legacy([198, 206, 202, 197, 200, 211]),
        "unicorn",
    ),
    (
        &crate::native_text::from_legacy([
            198, 206, 202, 197, 200, 211, 248, 207, 212, 209, 248, 203, 200, 208, 194, 213,
        ]),
        "unicorn_hsv_lower",
    ),
    (
        &crate::native_text::from_legacy([
            198, 206, 202, 197, 200, 211, 248, 207, 212, 209, 248, 203, 200, 208, 194, 213, 248,
            149,
        ]),
        "unicorn_hsv_lower_2",
    ),
    (
        &crate::native_text::from_legacy([
            198, 206, 202, 197, 200, 211, 248, 207, 212, 209, 248, 203, 200, 208, 194, 213, 248,
            148,
        ]),
        "unicorn_hsv_lower_3",
    ),
    (
        &crate::native_text::from_legacy([
            198, 206, 202, 197, 200, 211, 248, 207, 212, 209, 248, 210, 215, 215, 194, 213,
        ]),
        "unicorn_hsv_upper",
    ),
    (
        &crate::native_text::from_legacy([
            198, 206, 202, 197, 200, 211, 248, 207, 212, 209, 248, 210, 215, 215, 194, 213, 248,
            149,
        ]),
        "unicorn_hsv_upper_2",
    ),
    (
        &crate::native_text::from_legacy([
            198, 206, 202, 197, 200, 211, 248, 207, 212, 209, 248, 210, 215, 215, 194, 213, 248,
            148,
        ]),
        "unicorn_hsv_upper_3",
    ),
    (
        &crate::native_text::from_legacy([198, 206, 202, 204, 194, 222]),
        "unicornkey",
    ),
    (
        &crate::native_text::from_legacy([198, 206, 202, 204, 194, 222, 201, 198, 202, 194]),
        "unicornkeyname",
    ),
    (
        &crate::native_text::from_legacy([198, 206, 202, 211, 222, 215, 194]),
        "unicorntype",
    ),
    (
        &crate::native_text::from_legacy([196, 203, 200, 212, 194, 212, 211, 248, 198, 206, 202]),
        "closest_unicorn",
    ),
    (
        &crate::native_text::from_legacy([
            212, 202, 200, 200, 211, 207, 194, 213, 212, 211, 194, 215,
        ]),
        "beesmascurve",
    ),
    (
        &crate::native_text::from_legacy([212, 202, 200, 200, 211, 207, 206, 201, 192]),
        "beesmas_value",
    ),
    (
        &crate::native_text::from_legacy([212, 202, 200, 200, 211, 207, 212, 211, 194, 215]),
        "beesmasstep",
    ),
    (
        &crate::native_text::from_legacy([212, 202, 200, 200, 211, 207, 211, 222, 215, 194]),
        "beesmastype",
    ),
    (
        &crate::native_text::from_legacy([
            211, 213, 206, 192, 192, 194, 213, 248, 198, 196, 211, 206, 209, 198, 211, 194, 212,
            248, 198, 206, 202,
        ]),
        "pegasus_activates_unicorn",
    ),
    (
        &crate::native_text::from_legacy([
            211, 213, 206, 192, 192, 194, 213, 248, 200, 201, 248, 192, 213, 194, 194, 201,
        ]),
        "pegasus_on_green",
    ),
    (
        &crate::native_text::from_legacy([
            211, 213, 206, 192, 192, 194, 213, 248, 200, 201, 248, 213, 194, 195,
        ]),
        "pegasus_on_red",
    ),
    (
        &crate::native_text::from_legacy([
            211, 213, 206, 192, 192, 194, 213, 248, 212, 200, 210, 213, 196, 194,
        ]),
        "pegasus_source",
    ),
    (
        &crate::native_text::from_legacy([211, 213, 206, 192, 192, 194, 213, 197, 200, 211]),
        "pegasus",
    ),
    (
        &crate::native_text::from_legacy([
            211, 213, 206, 192, 192, 194, 213, 197, 200, 211, 248, 207, 212, 209, 248, 203, 200,
            208, 194, 213,
        ]),
        "pegasus_hsv_lower",
    ),
    (
        &crate::native_text::from_legacy([
            211, 213, 206, 192, 192, 194, 213, 197, 200, 211, 248, 207, 212, 209, 248, 210, 215,
            215, 194, 213,
        ]),
        "pegasus_hsv_upper",
    ),
    (
        &crate::native_text::from_legacy([194, 212, 215, 248, 212, 211, 222, 203, 194]),
        "aurora_style",
    ),
    (&crate::native_text::from_legacy([193, 200, 209]), "halo"),
    (
        &crate::native_text::from_legacy([
            202, 194, 202, 200, 213, 222, 248, 209, 206, 212, 206, 197, 203, 194, 248, 200, 201,
            203, 222,
        ]),
        "prism_only",
    ),
    (
        &crate::native_text::from_legacy([
            215, 203, 198, 222, 194, 213, 248, 215, 213, 200, 205, 194, 196, 211, 206, 200, 201,
            212,
        ]),
        "lanterns",
    ),
    (
        &crate::native_text::from_legacy([
            215, 213, 200, 205, 194, 196, 211, 206, 200, 201, 248, 193, 206, 203, 211, 194, 213,
        ]),
        "lantern_filter",
    ),
    (
        &crate::native_text::from_legacy([
            215, 213, 200, 205, 194, 196, 211, 206, 200, 201, 212, 248, 204, 194, 222,
        ]),
        "lantern_key",
    ),
    (
        &crate::native_text::from_legacy([
            215, 213, 200, 205, 194, 196, 211, 206, 200, 201, 212, 248, 204, 194, 222, 248, 201,
            198, 202, 194,
        ]),
        "lantern_key_name",
    ),
    (
        &crate::native_text::from_legacy([
            215, 213, 200, 223, 248, 197, 210, 206, 203, 195, 248, 202, 212,
        ]),
        "velvet_build_ms",
    ),
    (
        &crate::native_text::from_legacy([215, 213, 200, 223, 248, 196, 207, 198, 213, 192, 194]),
        "velvet_charge",
    ),
    (
        &crate::native_text::from_legacy([
            215, 213, 200, 223, 248, 195, 206, 212, 211, 198, 201, 196, 194,
        ]),
        "velvet_distance",
    ),
    (
        &crate::native_text::from_legacy([215, 213, 200, 223, 248, 194, 198, 212, 194]),
        "velvet_ease",
    ),
    (
        &crate::native_text::from_legacy([215, 213, 200, 223, 248, 194, 193, 193, 194, 196, 211]),
        "velvet_effect",
    ),
    (
        &crate::native_text::from_legacy([213, 198, 195, 198, 213]),
        "comet",
    ),
    (
        &crate::native_text::from_legacy([
            213, 198, 195, 198, 213, 248, 193, 206, 203, 211, 194, 213,
        ]),
        "comet_filter",
    ),
    (
        &crate::native_text::from_legacy([
            213, 198, 195, 198, 213, 248, 215, 203, 198, 196, 194, 202, 194, 201, 211,
        ]),
        "comet_placement",
    ),
    (
        &crate::native_text::from_legacy([213, 198, 195, 198, 213, 248, 212, 196, 198, 203, 194]),
        "comet_scale",
    ),
    (
        &crate::native_text::from_legacy([
            213, 198, 195, 198, 213, 248, 223, 248, 215, 194, 213, 196, 194, 201, 211,
        ]),
        "comet_x_percent",
    ),
    (
        &crate::native_text::from_legacy([
            213, 198, 195, 198, 213, 248, 222, 248, 215, 194, 213, 196, 194, 201, 211,
        ]),
        "comet_y_percent",
    ),
    (
        &crate::native_text::from_legacy([213, 198, 202, 215, 248, 198, 215, 215, 203, 222]),
        "tide_apply",
    ),
    (
        &crate::native_text::from_legacy([213, 198, 202, 215, 248, 194, 198, 212, 194]),
        "tide_ease",
    ),
    (
        &crate::native_text::from_legacy([
            213, 198, 202, 215, 248, 206, 195, 203, 194, 248, 213, 194, 212, 194, 211, 248, 202,
            212,
        ]),
        "tide_idle_reset_ms",
    ),
    (
        &crate::native_text::from_legacy([
            213, 198, 202, 215, 248, 211, 206, 202, 194, 248, 202, 212,
        ]),
        "tide_time_ms",
    ),
    (
        &crate::native_text::from_legacy([
            211, 198, 213, 192, 194, 211, 248, 193, 206, 203, 211, 194, 213,
        ]),
        "orbit_filter",
    ),
    (
        &crate::native_text::from_legacy([
            211, 198, 213, 192, 194, 211, 248, 215, 213, 206, 200, 213, 206, 211, 222,
        ]),
        "orbit_order",
    ),
    (
        &crate::native_text::from_legacy([209, 206, 212, 210, 198, 203, 212]),
        "aurora_enabled",
    ),
    (
        &crate::native_text::from_legacy([193, 200, 209, 248, 196, 200, 203]),
        "halo_col",
    ),
];

pub(crate) fn import(map: &mut std::collections::HashMap<String, String>) -> std::io::Result<bool> {
    let mut changed = false;
    for (encoded, current) in ALIASES {
        let old = crate::native_text::open(encoded);
        if let Some(value) = map.remove(old.as_str()) {
            changed = true;
            if let Some(existing) = map.get(*current) {
                if existing != &value {
                    return Err(std::io::Error::other(format!("Conflicting profile setting: {current}. Resolve the duplicate before importing.")));
                }
            } else {
                map.insert((*current).to_owned(), value);
            }
        }
        for key in [
            "tide_apply",
            "tide_ease",
            "velvet_effect",
            "velvet_ease",
            "orbit_order",
            "ramp_apply",
            "ramp_ease",
            "prox_effect",
            "prox_ease",
            "target_priority",
        ] {
            if let Some(value) = map.get_mut(key) {
                if value == old.as_str() {
                    *value = (*current).to_owned();
                    changed = true;
                }
            }
        }
    }
    Ok(changed)
}
