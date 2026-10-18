"""Named, user-confirmed captures for visibility discovery (never engine LOS)."""


def phase(name, observation, partition, prompt, moving=False):
    return {"name": name, "observation": observation, "partition": partition,
            "prompt": prompt, "moving_target": moving}


CLEAR = "Laptop: stand fully exposed inside the PC view."
BLOCKED = "Laptop: hide fully behind an opaque wall, still inside the PC view."

# Reversals reject fields that merely increase with elapsed time. Rechecks never
# choose a threshold or recalibrate a clock. The observer stays fixed throughout.
PHASES = (
    phase("clear-1", "clear", "discovery", CLEAR),
    phase("blocked-1", "blocked", "discovery", BLOCKED),
    phase("clear-2", "clear", "discovery", CLEAR),
    phase("blocked-2", "blocked", "discovery", BLOCKED),
    phase("clear-3", "clear", "discovery", CLEAR),
    phase("movement-control", "clear", "control",
          "Laptop: walk in open space, fully exposed throughout capture.", moving=True),
    phase("recheck-clear-1", "clear", "recheck", CLEAR + " Use another exposed spot."),
    phase("recheck-blocked-1", "blocked", "recheck", BLOCKED),
    phase("recheck-clear-2", "clear", "recheck", CLEAR),
    phase("recheck-blocked-2", "blocked", "recheck", BLOCKED),
    phase("recheck-clear-3", "clear", "recheck", CLEAR + " Use another exposed spot."),
    phase("recheck-blocked-3", "blocked", "recheck", BLOCKED + " Use another opaque wall in this fixed view."),
)

# Same target position, different observer lines of sight. Observer motion is
# permitted between bursts and explicitly during one exposed-motion control.
OBSERVER_CLEAR = "Laptop stays fixed. PC: expose the laptop by moving around cover; stop and aim at it."
OBSERVER_BLOCKED = "Laptop stays fixed. PC: move behind opaque cover; stop and aim at its hidden location."
OBSERVER_PHASES = (
    phase("observer-clear-1", "clear", "discovery", OBSERVER_CLEAR),
    phase("observer-blocked-1", "blocked", "discovery", OBSERVER_BLOCKED),
    phase("observer-clear-2", "clear", "discovery", OBSERVER_CLEAR),
    phase("observer-blocked-2", "blocked", "discovery", OBSERVER_BLOCKED),
    {**phase("observer-movement-control", "clear", "control",
             "Laptop stays fixed. PC: walk in open view while keeping the laptop fully exposed and in view."),
     "moving_observer": True},
    phase("observer-recheck-clear-1", "clear", "recheck", OBSERVER_CLEAR + " Use another viewing angle."),
    phase("observer-recheck-blocked-1", "blocked", "recheck", OBSERVER_BLOCKED),
    phase("observer-recheck-clear-2", "clear", "recheck", OBSERVER_CLEAR + " Use another viewing angle."),
    phase("observer-recheck-blocked-2", "blocked", "recheck", OBSERVER_BLOCKED),
)
