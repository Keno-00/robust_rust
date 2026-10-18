//! Render recency for the executable build enforced by apb_atlas.
//! This is the observed pawn timestamp, not an instantaneous geometry trace.
use crate::lattice_encoding::EncodedOffset;
use crate::apb_orchid::{field, kind, read, signature, typed, u64_at, PawnRef, PAWN, WORLD};
use crate::atlas_reader::AtlasReader;
use std::collections::{HashMap, HashSet};

pub const PAWN_LAST_RENDER_TIME: EncodedOffset = EncodedOffset::new(0xB4);
pub const WORLD_TIME_SECONDS: EncodedOffset = EncodedOffset::new(0x6A0);
pub const RECENCY_SECONDS: f64 = 1.0;
const ACQUIRE_AGE_SECONDS: f64 = 0.10;
const ACQUIRE_STABLE_SECONDS: f64 = 0.25;
const MAX_SAMPLE_GAP_SECONDS: f64 = 0.15;
const MIN_RENDER_ADVANCES: usize = 3;
const PAWN_WORLD: usize = 0x2E0;
const NEGATIVE_AGE_TOLERANCE: f64 = 0.25;

struct RecencyRun {
    since: f64,
    sampled_at: f64,
    rendered_at: f64,
    advances: usize,
}

/// A fresh render age must persist before aim can acquire this exact instance.
/// This suppresses intermittent recency; it does not establish wall visibility.
#[derive(Default)]
pub struct StableRecency {
    world: Option<PawnRef>,
    runs: HashMap<PawnRef, RecencyRun>,
}

impl StableRecency {
    pub fn clear(&mut self) {
        self.runs.clear();
        self.world = None;
    }

    pub fn retain(&mut self, current: &HashSet<PawnRef>) {
        self.runs.retain(|pawn, _| current.contains(pawn));
    }

    pub fn eligible(&mut self, world: PawnRef, pawn: PawnRef, value: RenderRecency) -> bool {
        if self.world != Some(world) {
            self.clear();
            self.world = Some(world);
        }
        if !value.recently_rendered || !(0.0..=ACQUIRE_AGE_SECONDS).contains(&value.age_seconds) {
            self.runs.remove(&pawn);
            return false;
        }
        let run = self.runs.entry(pawn).or_insert(RecencyRun {
            since: value.world_time,
            sampled_at: value.world_time,
            rendered_at: value.last_render_time,
            advances: 0,
        });
        let gap = value.world_time - run.sampled_at;
        if !(0.0..=MAX_SAMPLE_GAP_SECONDS).contains(&gap) || value.last_render_time < run.rendered_at {
            *run = RecencyRun {
                since: value.world_time,
                sampled_at: value.world_time,
                rendered_at: value.last_render_time,
                advances: 0,
            };
            return false;
        }
        if value.last_render_time > run.rendered_at {
            run.advances = run.advances.saturating_add(1);
        }
        run.sampled_at = value.world_time;
        run.rendered_at = value.last_render_time;
        run.advances >= MIN_RENDER_ADVANCES && value.world_time - run.since >= ACQUIRE_STABLE_SECONDS
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RenderRecency {
    pub last_render_time: f64,
    pub world_time: f64,
    pub age_seconds: f64,
    pub recently_rendered: bool,
}

impl RenderRecency {
    pub fn at_world_time(self, world_time: f64) -> Result<Self, String> {
        if !world_time.is_finite() || world_time < self.world_time {
            return Err(
                "Visibility World clock is invalid or moved backward; targets withheld.".into(),
            );
        }
        let age = world_time - self.last_render_time;
        if !age.is_finite() || age < -NEGATIVE_AGE_TOLERANCE {
            return Err(format!("Visibility timestamp is ahead of World clock: render={}, World={world_time}; targets withheld. Inspect verux_apb.log.", self.last_render_time));
        }
        Ok(Self {
            world_time,
            age_seconds: age,
            // Zero is an unrendered timestamp, never permission to acquire.
            recently_rendered: self.last_render_time > 0.0 && age <= RECENCY_SECONDS,
            ..self
        })
    }
}

pub fn world_clock(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    world: PawnRef,
) -> Result<f64, String> {
    if typed(reader, pid, base, world.address.get(), WORLD)? != world.signature {
        return Err("Visibility World instance changed before clock read.".into());
    }
    let value = f64::from_le_bytes(read::<8>(reader, pid, world.address.get() + WORLD_TIME_SECONDS.get())?);
    if !value.is_finite() || value < 0.0 {
        return Err(format!(
            "Invalid float64 visibility clock at World 0x{:X}: {value}; inspect verux_apb.log.",
            world.address.get()
        ));
    }
    if typed(reader, pid, base, world.address.get(), WORLD)? != world.signature {
        return Err("Visibility World changed during clock read.".into());
    }
    Ok(value)
}

pub fn sample(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    pawn: PawnRef,
    world: PawnRef,
) -> Result<RenderRecency, String> {
    let before = world_clock(reader, pid, base, world)?;
    // One prefix read contains identity, the four-byte-aligned double and World link.
    let body = read::<{ PAWN_WORLD + 8 }>(reader, pid, pawn.address.get())?;
    if kind(&body, base) != PAWN
        || signature(&body) != pawn.signature
        || u64_at(&body, PAWN_WORLD) != world.address.get()
    {
        return Err(format!(
            "Visibility pawn 0x{:X} changed instance or World; reacquire its current roster.",
            pawn.address.get()
        ));
    }
    let timestamp = f64::from_le_bytes(
        body[PAWN_LAST_RENDER_TIME.get() as usize..PAWN_LAST_RENDER_TIME.get() as usize + 8]
            .try_into()
            .unwrap(),
    );
    if !timestamp.is_finite() || timestamp < 0.0 {
        return Err(format!(
            "Invalid pawn +0xB4 float64 at 0x{:X}: {timestamp}; inspect verux_apb.log.",
            pawn.address.get()
        ));
    }
    let after = world_clock(reader, pid, base, world)?;
    if typed(reader, pid, base, pawn.address.get(), PAWN)? != pawn.signature
        || field(reader, pid, pawn.address.get() + PAWN_WORLD as u64)? != world.address.get()
    {
        return Err("Visibility pawn/World link changed during timestamp read.".into());
    }
    // Use the later clock conservatively. No division by a host read-time span
    // is required, so equal Windows timer ticks cannot break this check.
    RenderRecency {
        last_render_time: timestamp,
        world_time: before,
        age_seconds: 0.0,
        recently_rendered: false,
    }
    .at_world_time(after)
}
