//! Current persistent-level projectile observations for the hash-gated APB build.
//! Two recorded cAPBProjectile instances establish this type, not every projectile subclass.
//! Evidence: captures/apb-validation-live-20261018-392e67cf/events.jsonl first_seen.
use crate::apb_orchid::{
    field, kind, pointer, read, signature, typed, u32_at, u64_at, PawnRef, WORLD,
};
use crate::atlas_reader::AtlasReader;
use crate::lattice_encoding::{EncodedAddress, EncodedOffset, ObjectType};
use std::collections::{HashMap, HashSet};
use std::time::{Instant, SystemTime};

const PROJECTILE: ObjectType = ObjectType::new(0x2B68D90, 6770);
const LEVEL_VTABLE: EncodedOffset = EncodedOffset::new(0x3208BF8);
const NAME_ROOT: EncodedOffset = EncodedOffset::new(0x3CD1F00);
const OUTER: EncodedOffset = EncodedOffset::new(0x44);
const ACTORS: EncodedOffset = EncodedOffset::new(0xB4);
const POSITION: EncodedOffset = EncodedOffset::new(0x88);
const ACTOR_WORLD: EncodedOffset = EncodedOffset::new(0x2E0);
const WORLD_CLOCK: EncodedOffset = EncodedOffset::new(0x6A0);
const MAX_ACTORS: usize = 100_000;

#[derive(Debug, Clone)]
pub struct WispObject {
    pub instance: PawnRef,
    pub position: [f32; 3],
    /// Observation times only: never spawn time, fuse or detonation time.
    pub first_seen: Instant,
    pub last_seen: Instant,
    pub first_seen_utc: SystemTime,
    pub last_seen_utc: SystemTime,
    pub first_seen_world_time: f64,
    pub last_seen_world_time: f64,
}

struct Membership {
    level: PawnRef,
    members: HashSet<EncodedAddress>,
}

enum SurveyError {
    Changed(&'static str),
    Fault(String),
}

impl From<String> for SurveyError {
    fn from(error: String) -> Self {
        Self::Fault(error)
    }
}

impl From<&str> for SurveyError {
    fn from(error: &str) -> Self {
        Self::Fault(error.to_owned())
    }
}

pub struct WispSurvey {
    base: EncodedAddress,
    pid: u32,
    name_table: Option<EncodedAddress>,
    world: Option<PawnRef>,
    classifications: HashMap<PawnRef, bool>,
    observed: HashMap<PawnRef, WispObject>,
}

impl WispSurvey {
    pub fn new(base: u64, pid: u32) -> Result<Self, String> {
        if !pointer(base) || pid == 0 {
            return Err("Projectile survey requires a verified APB base and PID.".into());
        }
        Ok(Self {
            base: EncodedAddress::new(base)?,
            pid,
            name_table: None,
            world: None,
            classifications: HashMap::new(),
            observed: HashMap::new(),
        })
    }

    fn membership(&self, reader: &AtlasReader, world: PawnRef) -> Result<Membership, SurveyError> {
        if typed(
            reader,
            self.pid,
            self.base.get(),
            world.address.get(),
            WORLD,
        )? != world.signature
        {
            return Err("Projectile survey World instance changed; reacquire the district.".into());
        }
        let level = field(reader, self.pid, world.address.get() + OUTER.get())?;
        if !pointer(level) {
            return Err("Projectile survey persistent level is unavailable.".into());
        }
        let header = read::<64>(reader, self.pid, level)?;
        // Level names differ by package; validate the observed level vtable and exact instance.
        if kind(&header, self.base.get()).0 != LEVEL_VTABLE.get() {
            return Err(
                "Projectile survey persistent-level type differs from the verified layout.".into(),
            );
        }
        let level = PawnRef {
            address: EncodedAddress::new(level)?,
            signature: signature(&header),
        };
        let root = read::<16>(reader, self.pid, level.address.get() + ACTORS.get())?;
        let table = u64_at(&root, 0);
        let count = u32_at(&root, 8) as usize;
        let capacity = u32_at(&root, 12) as usize;
        if count > capacity || capacity > MAX_ACTORS || (count != 0 && !pointer(table)) {
            return Err("Projectile survey actor-array bounds are invalid.".into());
        }
        let mut bytes = vec![0u8; count * 8];
        for (index, chunk) in bytes.chunks_mut(4096).enumerate() {
            if let Err(error) = reader.read_exact(self.pid, table + index as u64 * 4096, chunk) {
                if read::<16>(reader, self.pid, level.address.get() + ACTORS.get())? != root {
                    return Err(SurveyError::Changed(
                        "actor array resized during membership read",
                    ));
                }
                return Err(error.into());
            }
        }
        if read::<16>(reader, self.pid, level.address.get() + ACTORS.get())? != root
            || signature(&read::<64>(reader, self.pid, level.address.get())?) != level.signature
            || field(reader, self.pid, world.address.get() + OUTER.get())? != level.address.get()
            || typed(
                reader,
                self.pid,
                self.base.get(),
                world.address.get(),
                WORLD,
            )? != world.signature
        {
            return Err(SurveyError::Changed(
                "actor-array binding changed during membership capture",
            ));
        }
        let mut members = HashSet::new();
        for entry in bytes.chunks_exact(8) {
            let address = u64_at(entry, 0);
            if address == 0 {
                continue;
            }
            if !pointer(address) {
                return Err("Projectile survey actor array contains an invalid pointer.".into());
            }
            members.insert(EncodedAddress::new(address)?);
        }
        Ok(Membership { level, members })
    }

    /// A bounded current persistent-level membership proof, used only after a failed object read.
    /// None means the array changed while proving membership; publish no marker and retry.
    pub(crate) fn current_member(
        reader: &AtlasReader,
        pid: u32,
        base: u64,
        world: PawnRef,
        address: EncodedAddress,
    ) -> Result<Option<bool>, String> {
        let survey = Self::new(base, pid)?;
        match survey.membership(reader, world) {
            Ok(snapshot) => Ok(Some(snapshot.members.contains(&address))),
            Err(SurveyError::Changed(reason)) => {
                crate::log_info!("Projectile read withheld while checking membership: {reason}; fresh survey pending.");
                Ok(None)
            }
            Err(SurveyError::Fault(error)) => Err(error),
        }
    }

    fn verify_name(&mut self, reader: &AtlasReader) -> Result<EncodedAddress, String> {
        let root = read::<16>(reader, self.pid, self.base.get() + NAME_ROOT.get())?;
        let table = u64_at(&root, 0);
        let count = u32_at(&root, 8);
        let capacity = u32_at(&root, 12);
        if !pointer(table) || count <= 6770 || count > capacity || capacity > 1_000_000 {
            return Err("Projectile survey reflected name-table bounds are invalid.".into());
        }
        let table = EncodedAddress::new(table)?;
        if self.name_table != Some(table) {
            let entry = field(reader, self.pid, table.get() + 6770 * 8)?;
            // FName entries are byte-packed, unlike aligned UObject instances.
            if !(0x10000..0x800000000000).contains(&entry) {
                return Err("Projectile reflected name-entry pointer is invalid.".into());
            }
            let bytes = read::<128>(reader, self.pid, entry)?;
            let flags = u32_at(&bytes, 8);
            if flags >> 1 != 6770 {
                return Err("Projectile reflected name-entry identity differs.".into());
            }
            let name = if flags & 1 == 0 {
                let text = bytes[24..].split(|value| *value == 0).next().unwrap_or(&[]);
                String::from_utf8(text.to_vec())
                    .map_err(|_| "Projectile reflected name is invalid UTF-8.")?
            } else {
                let text: Vec<u16> = bytes[24..]
                    .chunks_exact(2)
                    .map(|word| u16::from_le_bytes([word[0], word[1]]))
                    .take_while(|word| *word != 0)
                    .collect();
                String::from_utf16(&text)
                    .map_err(|_| "Projectile reflected name is invalid UTF-16.")?
            };
            if name != "cAPBProjectile" {
                return Err(
                    "Projectile reflected name differs from the captured cAPBProjectile type."
                        .into(),
                );
            }
            self.name_table = Some(table);
            self.classifications.clear();
        }
        Ok(table)
    }

    fn position(
        &self,
        reader: &AtlasReader,
        world: PawnRef,
        instance: PawnRef,
        expected_level: PawnRef,
    ) -> Result<Option<[f32; 3]>, SurveyError> {
        let body = match read::<0x2E8>(reader, self.pid, instance.address.get()) {
            Ok(body) => body,
            Err(error) => {
                let now = self.membership(reader, world)?;
                if now.level != expected_level {
                    return Err(SurveyError::Changed(
                        "persistent-level instance changed during position read",
                    ));
                }
                if now.members.contains(&instance.address) {
                    return Err(error.into());
                }
                crate::log_info!(
                    "Projectile removed during position read at 0x{:X}; marker withheld.",
                    instance.address.get()
                );
                return Ok(None);
            }
        };
        if signature(&body) != instance.signature || kind(&body, self.base.get()) != PROJECTILE {
            crate::log_info!(
                "Projectile instance expired at 0x{:X}; marker withheld.",
                instance.address.get()
            );
            return Ok(None);
        }
        if u64_at(&body, ACTOR_WORLD.get() as usize) != world.address.get() {
            return Err("Projectile World backlink changed; sample withheld.".into());
        }
        let offset = POSITION.get() as usize;
        let position = std::array::from_fn(|axis| f32::from_bits(u32_at(&body, offset + axis * 4)));
        if position
            .iter()
            .any(|value| !value.is_finite() || value.abs() >= 1_000_000.0)
        {
            return Err("Projectile position is invalid; sample withheld.".into());
        }
        Ok(Some(position))
    }

    /// Run in the acquisition worker, not the render loop. No streamed-level sweep.
    pub fn refresh(
        &mut self,
        reader: &AtlasReader,
        world: PawnRef,
    ) -> Result<Vec<WispObject>, String> {
        match self.refresh_inner(reader, world) {
            Ok(objects) => Ok(objects),
            Err(SurveyError::Changed(reason)) => {
                self.observed.clear();
                crate::log_info!("Projectile survey withheld: {reason}; retrying the current reader on its next pass.");
                Ok(Vec::new())
            }
            Err(SurveyError::Fault(error)) => {
                self.observed.clear();
                crate::log_err!("Projectile survey withheld: {error}");
                Err(error)
            }
        }
    }

    fn refresh_inner(
        &mut self,
        reader: &AtlasReader,
        world: PawnRef,
    ) -> Result<Vec<WispObject>, SurveyError> {
        if self.world != Some(world) {
            self.observed.clear();
            self.classifications.clear();
            self.world = Some(world);
        }
        let before = self.membership(reader, world)?;
        let name_table = self.verify_name(reader)?;
        let mut candidates = Vec::new();
        let mut current = HashSet::new();
        for address in &before.members {
            let header = match read::<64>(reader, self.pid, address.get()) {
                Ok(header) => header,
                Err(error) => {
                    let now = self.membership(reader, world)?;
                    if now.level != before.level {
                        return Err(SurveyError::Changed(
                            "persistent-level instance changed during actor read",
                        ));
                    }
                    if now.members.contains(address) {
                        return Err(error.into());
                    }
                    crate::log_info!(
                        "Projectile survey actor removed during read at 0x{:X}.",
                        address.get()
                    );
                    continue;
                }
            };
            let instance = PawnRef {
                address: *address,
                signature: signature(&header),
            };
            current.insert(instance);
            let projectile = *self
                .classifications
                .entry(instance)
                .or_insert_with(|| kind(&header, self.base.get()) == PROJECTILE);
            if projectile {
                candidates.push(instance);
            }
        }
        let after = self.membership(reader, world)?;
        if before.level != after.level {
            return Err(SurveyError::Changed(
                "persistent-level instance changed during survey",
            ));
        }
        let mut found = HashMap::new();
        for instance in candidates {
            if !after.members.contains(&instance.address) {
                continue;
            }
            let Some(position) = self.position(reader, world, instance, before.level)? else {
                continue;
            };
            let world_time = f64::from_le_bytes(read::<8>(
                reader,
                self.pid,
                world.address.get() + WORLD_CLOCK.get(),
            )?);
            if !world_time.is_finite() || world_time < 0.0 {
                return Err("Projectile observation World clock is invalid.".into());
            }
            let now = Instant::now();
            let object = if let Some(previous) = self.observed.get(&instance) {
                WispObject {
                    position,
                    last_seen: now,
                    last_seen_utc: SystemTime::now(),
                    last_seen_world_time: world_time,
                    ..previous.clone()
                }
            } else {
                crate::log_info!(
                    "Projectile first observed at 0x{:X}, World time {:.3}; no fuse inferred.",
                    instance.address.get(),
                    world_time
                );
                WispObject {
                    instance,
                    position,
                    first_seen: now,
                    last_seen: now,
                    first_seen_utc: SystemTime::now(),
                    last_seen_utc: SystemTime::now(),
                    first_seen_world_time: world_time,
                    last_seen_world_time: world_time,
                }
            };
            found.insert(instance, object);
        }
        let final_membership = self.membership(reader, world)?;
        if final_membership.level != before.level {
            return Err(SurveyError::Changed(
                "persistent-level instance changed during final membership check",
            ));
        }
        if self.verify_name(reader)? != name_table {
            return Err(SurveyError::Changed(
                "validated name-table binding changed during survey",
            ));
        }
        found.retain(|instance, _| final_membership.members.contains(&instance.address));
        for instance in found.keys().copied().collect::<Vec<_>>() {
            let header = match read::<64>(reader, self.pid, instance.address.get()) {
                Ok(header) => header,
                Err(error) => {
                    let now = self.membership(reader, world)?;
                    if now.level != before.level {
                        return Err(SurveyError::Changed(
                            "persistent-level instance changed before projectile publication",
                        ));
                    }
                    if now.members.contains(&instance.address) {
                        return Err(error.into());
                    }
                    crate::log_info!(
                        "Projectile removed before publication at 0x{:X}; marker withheld.",
                        instance.address.get()
                    );
                    found.remove(&instance);
                    continue;
                }
            };
            if signature(&header) != instance.signature
                || kind(&header, self.base.get()) != PROJECTILE
            {
                crate::log_info!(
                    "Projectile instance replaced before publish at 0x{:X}; marker withheld.",
                    instance.address.get()
                );
                found.remove(&instance);
            }
        }
        for instance in self
            .observed
            .keys()
            .filter(|instance| !found.contains_key(instance))
        {
            crate::log_info!("Projectile last observation ended at 0x{:X}; removal does not establish detonation.", instance.address.get());
        }
        self.classifications
            .retain(|instance, _| current.contains(instance));
        self.observed = found;
        Ok(self.observed.values().cloned().collect())
    }
}
