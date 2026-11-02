//! Independent object acquisition workers; rendering only rereads accepted instances.
use crate::apb_jade::JadeSurvey;
use crate::apb_orchid::{self as orchid, PawnRef};
use crate::apb_wisp::WispSurvey;
use crate::atlas_reader::AtlasReader;
use crate::lattice_encoding::{EncodedAddress, EncodedOffset, ObjectType};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const PREFAB: ObjectType = ObjectType::new(0x3151050, 4741);
const PROP: ObjectType = ObjectType::new(0x2B3A290, 1650);
const PROJECTILE: ObjectType = ObjectType::new(0x2B68D90, 6770);
const POSITION: EncodedOffset = EncodedOffset::new(0x88);
const ACTOR_WORLD: EncodedOffset = EncodedOffset::new(0x2E0);
const PREFAB_ASSET: EncodedOffset = EncodedOffset::new(0x308);
const PROP_HIDDEN: EncodedOffset = EncodedOffset::new(0x7C);
const PROP_DISABLED: EncodedOffset = EncodedOffset::new(0x84);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Jade,
    Wisp,
}

impl ObjectKind {
    fn index(self) -> usize {
        match self {
            Self::Jade => 0,
            Self::Wisp => 1,
        }
    }
    fn interval(self) -> Duration {
        Duration::from_millis(match self {
            Self::Jade => 100,
            Self::Wisp => 75,
        })
    }
    fn max_age(self) -> Duration {
        Duration::from_millis(match self {
            Self::Jade => 5_000,
            // Positions are reread before projection; this bounds candidate discovery age.
            Self::Wisp => 1_000,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ObjectPoint {
    pub kind: ObjectKind,
    pub red: bool,
    pub instance: PawnRef,
    pub position: [f32; 3],
}

#[derive(Clone)]
struct Batch {
    world: PawnRef,
    completed: Instant,
    objects: Vec<Observed>,
}

#[derive(Clone)]
struct Observed {
    instance: PawnRef,
    position: [f32; 3],
    prop: Option<PawnRef>,
}

fn observed(
    instance: PawnRef,
    position: [f32; 3],
    prop: Option<PawnRef>,
) -> Result<Observed, String> {
    if position
        .iter()
        .any(|value| !value.is_finite() || value.abs() > 10_000_000.0)
    {
        return Err("Object worker observed invalid coordinates; batch withheld.".into());
    }
    Ok(Observed {
        instance,
        position,
        prop,
    })
}

fn nearby(position: [f32; 3], camera: [f32; 3], radius: f64) -> bool {
    position
        .iter()
        .zip(camera)
        .map(|(value, camera)| (*value as f64 - camera as f64).powi(2))
        .sum::<f64>()
        <= radius.powi(2)
}

struct State {
    world: Option<PawnRef>,
    generation: u64,
    stop: bool,
    batches: [Option<Batch>; 2],
    pending: [Option<Instant>; 2],
    slow_logged: [bool; 2],
    error: Option<String>,
}

type Shared = Arc<(Mutex<State>, Condvar)>;

pub struct ObjectWorkers {
    base: EncodedAddress,
    pid: u32,
    shared: Shared,
    threads: Vec<JoinHandle<()>>,
}

impl ObjectWorkers {
    pub fn new(
        pid: u32,
        base: u64,
        jade_enabled: bool,
        wisp_enabled: bool,
    ) -> Result<Self, String> {
        if pid == 0 || !orchid::pointer(base) {
            return Err("Object workers require the verified APB PID and module base.".into());
        }
        let mut workers = Self {
            base: EncodedAddress::new(base)?,
            pid,
            shared: Arc::new((
                Mutex::new(State {
                    world: None,
                    generation: 0,
                    stop: false,
                    batches: [None, None],
                    pending: [None, None],
                    slow_logged: [false; 2],
                    error: None,
                }),
                Condvar::new(),
            )),
            threads: Vec::new(),
        };
        for (kind, enabled) in [
            (ObjectKind::Jade, jade_enabled),
            (ObjectKind::Wisp, wisp_enabled),
        ] {
            if !enabled {
                continue;
            }
            let shared = Arc::clone(&workers.shared);
            let encoded_base = workers.base;
            let handle = thread::Builder::new()
                .name(format!("apb-{kind:?}"))
                .spawn(move || {
                    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        run_worker(kind, pid, encoded_base, &shared)
                    }));
                    let error = match failure {
                        Ok(Ok(())) => return,
                        Ok(Err(error)) => error,
                        Err(_) => format!(
                            "{kind:?} acquisition worker panicked; restart object acquisition."
                        ),
                    };
                    crate::log_err!("Object acquisition stopped: {error}");
                    if let Ok(mut state) = shared.0.lock() {
                        state.batches[kind.index()] = None;
                        if state.error.is_none() {
                            state.error = Some(error);
                        }
                        shared.1.notify_all();
                    }
                })
                .map_err(|error| format!("Cannot start {kind:?} acquisition worker: {error}"))?;
            workers.threads.push(handle);
        }
        Ok(workers)
    }

    /// Call with None whenever APB loses foreground or verified local ownership.
    pub fn set_world(&self, world: Option<PawnRef>) -> Result<(), String> {
        let mut state = self
            .shared
            .0
            .lock()
            .map_err(|_| "Object acquisition context lock poisoned.")?;
        if state.world != world {
            state.world = world;
            state.generation = state.generation.wrapping_add(1);
            state.batches = [None, None];
            state.pending = [None, None];
            state.slow_logged = [false; 2];
            self.shared.1.notify_all();
        }
        Ok(())
    }

    /// Also callable without an active World, so driver/startup failures stay visible.
    pub fn error(&self) -> Result<(), String> {
        let state = self
            .shared
            .0
            .lock()
            .map_err(|_| "Object acquisition context lock poisoned.")?;
        match &state.error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    /// Discovery positions cheaply restrict hot reads to 80 m (100 APB units per meter).
    pub fn points(
        &self,
        reader: &AtlasReader,
        world: PawnRef,
        camera_position: [f32; 3],
        radius: f64,
    ) -> Result<Vec<ObjectPoint>, String> {
        if camera_position.iter().any(|value| !value.is_finite()) {
            return Err("Object projection requires finite camera coordinates.".into());
        }
        let (batches, generation) = {
            let mut state = self
                .shared
                .0
                .lock()
                .map_err(|_| "Object acquisition context lock poisoned.")?;
            if let Some(error) = &state.error {
                return Err(error.clone());
            }
            if state.world != Some(world) {
                return Ok(Vec::new());
            }
            for index in 0..2 {
                if let Some(pending) = state.pending[index] {
                    if pending.elapsed() > Duration::from_secs(5) && !state.slow_logged[index] {
                        crate::log_info!("Object worker {} is still acquiring its first batch after {} ms; no markers published yet.", index, pending.elapsed().as_millis());
                        state.slow_logged[index] = true;
                    }
                }
            }
            (state.batches.clone(), state.generation)
        };
        if orchid::typed(
            reader,
            self.pid,
            self.base.get(),
            world.address.get(),
            orchid::WORLD,
        )? != world.signature
        {
            return Err("Object projection World instance changed; reacquire the district.".into());
        }
        let mut points = Vec::new();
        for (index, batch) in batches.into_iter().enumerate() {
            let Some(batch) = batch else {
                continue;
            };
            let kind = if index == 0 {
                ObjectKind::Jade
            } else {
                ObjectKind::Wisp
            };
            if batch.world != world {
                continue;
            }
            if batch.completed.elapsed() > kind.max_age() {
                return Err(format!(
                    "{kind:?} acquisition is stale ({} ms); markers withheld until a fresh survey.",
                    batch.completed.elapsed().as_millis()
                ));
            }
            let mut expired = Vec::new();
            for object in batch.objects {
                if !nearby(object.position, camera_position, radius) {
                    continue;
                }
                let instance = object.instance;
                match self.point(reader, world, kind, instance, object.prop)? {
                    Some(point) => {
                        if nearby(point.position, camera_position, radius) {
                            points.push(point);
                        }
                    }
                    None => {
                        crate::log_info!(
                            "{kind:?} marker at 0x{:X} is ineligible or expired; marker removed.",
                            instance.address.get()
                        );
                        expired.push(instance);
                    }
                }
            }
            if !expired.is_empty() {
                let mut state = self
                    .shared
                    .0
                    .lock()
                    .map_err(|_| "Object acquisition context lock poisoned.")?;
                if let Some(current) = state.batches[index].as_mut() {
                    if current.world == world {
                        current
                            .objects
                            .retain(|object| !expired.contains(&object.instance));
                    }
                }
            }
        }
        if orchid::typed(
            reader,
            self.pid,
            self.base.get(),
            world.address.get(),
            orchid::WORLD,
        )? != world.signature
        {
            return Err("Object projection World changed during the frame.".into());
        }
        let state = self
            .shared
            .0
            .lock()
            .map_err(|_| "Object acquisition context lock poisoned.")?;
        if let Some(error) = &state.error {
            return Err(error.clone());
        }
        if state.world != Some(world) || state.generation != generation {
            points.clear();
        }
        Ok(points)
    }

    fn point(
        &self,
        reader: &AtlasReader,
        world: PawnRef,
        kind: ObjectKind,
        instance: PawnRef,
        prop: Option<PawnRef>,
    ) -> Result<Option<ObjectPoint>, String> {
        let expected = match kind {
            ObjectKind::Jade => PREFAB,
            ObjectKind::Wisp => PROJECTILE,
        };
        let Some(body) =
            self.point_read::<0x310>(reader, world, kind, instance, instance.address.get())?
        else {
            return Ok(None);
        };
        if orchid::signature(&body) != instance.signature
            || orchid::kind(&body, self.base.get()) != expected
        {
            return Ok(None);
        }
        if orchid::u64_at(&body, ACTOR_WORLD.get() as usize) != world.address.get() {
            return Err(format!(
                "{kind:?} marker World backlink differs; frame withheld."
            ));
        }
        let asset = if kind == ObjectKind::Jade {
            let address = orchid::u64_at(&body, PREFAB_ASSET.get() as usize);
            if !orchid::pointer(address) {
                return Ok(None);
            }
            let header = orchid::read::<64>(reader, self.pid, address)?;
            let Some(red) =
                crate::apb_jade::asset_color(reader, self.pid, self.base.get(), &header)?
            else {
                return Ok(None);
            };
            Some((
                address,
                orchid::signature(&header),
                red,
                orchid::kind(&header, self.base.get()),
            ))
        } else {
            None
        };
        let position: [f32; 3] = std::array::from_fn(|axis| {
            f32::from_bits(orchid::u32_at(&body, POSITION.get() as usize + axis * 4))
        });
        if position
            .iter()
            .any(|value| !value.is_finite() || value.abs() > 10_000_000.0)
        {
            return Err(format!(
                "{kind:?} marker coordinates are invalid; frame withheld."
            ));
        }
        let live_prop = if kind == ObjectKind::Jade {
            let Some(prop) = prop else {
                crate::log_info!(
                    "Pumpkin at 0x{:X} has no verified destructible prop; marker withheld.",
                    instance.address.get()
                );
                return Ok(None);
            };
            if !self.prop_eligible(reader, world, prop, position)? {
                return Ok(None);
            }
            Some(prop)
        } else {
            None
        };
        let Some(final_header) =
            self.point_read::<64>(reader, world, kind, instance, instance.address.get())?
        else {
            return Ok(None);
        };
        if orchid::kind(&final_header, self.base.get()) != expected
            || orchid::signature(&final_header) != instance.signature
        {
            return Ok(None);
        }
        if let Some((address, signature, _, expected_asset)) = asset {
            if orchid::field(
                reader,
                self.pid,
                instance.address.get() + PREFAB_ASSET.get(),
            )? != address
            {
                return Ok(None);
            }
            let final_asset = orchid::read::<64>(reader, self.pid, address)?;
            if orchid::kind(&final_asset, self.base.get()) != expected_asset
                || orchid::signature(&final_asset) != signature
            {
                return Ok(None);
            }
        }
        // Collection can happen between the prefab read and its final asset check.
        if let Some(prop) = live_prop {
            if !self.prop_eligible(reader, world, prop, position)? {
                return Ok(None);
            }
        }
        Ok(Some(ObjectPoint {
            kind,
            red: asset.map_or(false, |(_, _, red, _)| red),
            instance,
            position,
        }))
    }

    fn point_read<const N: usize>(
        &self,
        reader: &AtlasReader,
        world: PawnRef,
        kind: ObjectKind,
        instance: PawnRef,
        address: u64,
    ) -> Result<Option<[u8; N]>, String> {
        if kind == ObjectKind::Wisp {
            return self.persistent_read(reader, world, instance, address, "Projectile");
        }
        orchid::read::<N>(reader, self.pid, address).map(Some)
    }

    fn persistent_read<const N: usize>(
        &self,
        reader: &AtlasReader,
        world: PawnRef,
        instance: PawnRef,
        address: u64,
        label: &str,
    ) -> Result<Option<[u8; N]>, String> {
        match orchid::read::<N>(reader, self.pid, address) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) => {
                match WispSurvey::current_member(
                    reader,
                    self.pid,
                    self.base.get(),
                    world,
                    instance.address,
                )? {
                    Some(true) => Err(error),
                    Some(false) => {
                        crate::log_info!(
                            "{label} removed during frame read at 0x{:X}; marker withheld.",
                            instance.address.get()
                        );
                        Ok(None)
                    }
                    None => {
                        crate::log_info!("{label} marker at 0x{:X} withheld during changing persistent membership; fresh survey pending.", instance.address.get());
                        Ok(None)
                    }
                }
            }
        }
    }

    fn prop_eligible(
        &self,
        reader: &AtlasReader,
        world: PawnRef,
        prop: PawnRef,
        prefab_position: [f32; 3],
    ) -> Result<bool, String> {
        let Some(body) =
            self.persistent_read::<0x2E8>(reader, world, prop, prop.address.get(), "Pumpkin prop")?
        else {
            return Ok(false);
        };
        if orchid::kind(&body, self.base.get()) != PROP
            || orchid::signature(&body) != prop.signature
        {
            return Ok(false);
        }
        if orchid::u64_at(&body, ACTOR_WORLD.get() as usize) != world.address.get() {
            return Err("Pumpkin prop World backlink differs; frame withheld.".into());
        }
        let position: [f32; 3] = std::array::from_fn(|axis| {
            f32::from_bits(orchid::u32_at(&body, POSITION.get() as usize + axis * 4))
        });
        if position
            .iter()
            .any(|value| !value.is_finite() || value.abs() > 10_000_000.0)
        {
            return Err("Pumpkin prop coordinates are invalid; frame withheld.".into());
        }
        let expected = [
            prefab_position[0] as f64,
            prefab_position[1] as f64,
            prefab_position[2] as f64 - 1.0,
        ];
        let separation = position
            .into_iter()
            .zip(expected)
            .map(|(actual, expected)| (actual as f64 - expected).powi(2))
            .sum::<f64>();
        if separation > 3.0f64.powi(2) {
            return Ok(false);
        }
        Ok(
            orchid::u32_at(&body, PROP_HIDDEN.get() as usize) & 0x4000 == 0
                && orchid::u32_at(&body, PROP_DISABLED.get() as usize) & 0x80 == 0,
        )
    }
}

enum Survey {
    Jade(JadeSurvey),
    Wisp(WispSurvey),
}

fn run_worker(
    kind: ObjectKind,
    pid: u32,
    base: EncodedAddress,
    shared: &Shared,
) -> Result<(), String> {
    let reader = AtlasReader::connect()?;
    let mut survey = match kind {
        ObjectKind::Jade => Survey::Jade(JadeSurvey::new(base.get(), pid)?),
        ObjectKind::Wisp => Survey::Wisp(WispSurvey::new(base.get(), pid)?),
    };
    loop {
        let (world, generation) = {
            let mut state = shared
                .0
                .lock()
                .map_err(|_| "Object worker context lock poisoned.")?;
            while state.world.is_none() && !state.stop {
                state = shared
                    .1
                    .wait(state)
                    .map_err(|_| "Object worker wait lock poisoned.")?;
            }
            if state.stop {
                return Ok(());
            }
            let world = state.world.ok_or("Object worker has no current World.")?;
            let index = kind.index();
            if state.batches[index].is_none() && state.pending[index].is_none() {
                state.pending[index] = Some(Instant::now());
                crate::log_info!("{kind:?} acquisition pending its first verified batch.");
            }
            (world, state.generation)
        };
        let started = Instant::now();
        let result = match &mut survey {
            Survey::Jade(survey) => survey.refresh(&reader, world).and_then(|objects| {
                objects
                    .into_iter()
                    .map(|object| {
                        observed(
                            object.instance,
                            object.position.map(|value| value as f32),
                            object.prop,
                        )
                    })
                    .collect::<Result<Vec<_>, String>>()
            }),
            Survey::Wisp(survey) => survey.refresh(&reader, world).and_then(|objects| {
                objects
                    .into_iter()
                    .map(|object| observed(object.instance, object.position, None))
                    .collect::<Result<Vec<_>, String>>()
            }),
        };
        let mut state = shared
            .0
            .lock()
            .map_err(|_| "Object worker publish lock poisoned.")?;
        if state.stop {
            return Ok(());
        }
        if state.generation != generation || state.world != Some(world) {
            if let Err(error) = result {
                crate::log_info!(
                    "{kind:?} obsolete-context survey discarded after context changed: {error}"
                );
            }
            continue;
        }
        let objects = result?;
        let index = kind.index();
        if let Some(pending) = state.pending[index].take() {
            crate::log_info!(
                "{kind:?} first verified batch: {} markers after {} ms.",
                objects.len(),
                pending.elapsed().as_millis()
            );
        }
        if started.elapsed() > Duration::from_secs(5) {
            crate::log_info!(
                "{kind:?} acquisition pass completed in {} ms.",
                started.elapsed().as_millis()
            );
        }
        state.batches[index] = Some(Batch {
            world,
            completed: Instant::now(),
            objects,
        });
        state.slow_logged[index] = false;
        drop(
            shared
                .1
                .wait_timeout(state, kind.interval())
                .map_err(|_| "Object worker interval lock poisoned.")?,
        );
    }
}

impl Drop for ObjectWorkers {
    fn drop(&mut self) {
        match self.shared.0.lock() {
            Ok(mut state) => {
                state.stop = true;
                state.world = None;
                state.batches = [None, None];
            }
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.stop = true;
                state.world = None;
            }
        }
        self.shared.1.notify_all();
        for handle in self.threads.drain(..) {
            if handle.join().is_err() {
                crate::log_err!("Object worker stopped with a thread panic.");
            }
        }
    }
}
