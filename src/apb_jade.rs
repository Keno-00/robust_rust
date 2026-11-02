//! Green pumpkin identity and user-requested unconfirmed red candidates; availability is unknown.
use std::collections::{HashMap, HashSet};

use crate::apb_orchid::{self as orchid, PawnRef};
use crate::atlas_reader::AtlasReader;
use crate::lattice_encoding::{EncodedAddress, EncodedOffset, ObjectType};

const LEVEL: ObjectType = ObjectType::new(0x3208BF8, 1160);
const OWNER: ObjectType = ObjectType::new(0x3164988, 1151);
const PREFAB: ObjectType = ObjectType::new(0x3151050, 4741);
const PROP: ObjectType = ObjectType::new(0x2B3A290, 1650);
const ASSET_RVA: EncodedOffset = EncodedOffset::new(0x31517A8);
const MAX_CHANGED_CAPTURES: u8 = 5;

/// Asset name indices are allocated at runtime; red remains visually unconfirmed.
pub(crate) fn asset_color(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    header: &[u8],
) -> Result<Option<bool>, String> {
    if header.len() < 64 {
        return Err("Pumpkin asset header truncated.".into());
    }
    let (rva, index) = orchid::kind(header, base);
    if rva != ASSET_RVA.get() {
        return Ok(None);
    }
    let root_address = base + const { EncodedOffset::new(0x3CD1F00) }.get();
    let root = orchid::read::<16>(reader, pid, root_address)?;
    let table = orchid::u64_at(&root, 0);
    let count = orchid::u32_at(&root, 8);
    let capacity = orchid::u32_at(&root, 12);
    if !orchid::pointer(table) || index >= count || count > capacity || capacity > 1_000_000 {
        return Err("Pumpkin asset name table bounds invalid.".into());
    }
    let slot = table + u64::from(index) * 8;
    let entry = orchid::field(reader, pid, slot)?;
    // FName entries are byte-packed, unlike aligned UObject instances.
    if !(0x10000..0x800000000000).contains(&entry) {
        return Err("Pumpkin asset name entry unavailable.".into());
    }
    let body = orchid::read::<64>(reader, pid, entry)?;
    let flags = orchid::u32_at(&body, 8);
    if flags >> 1 != index {
        return Err("Pumpkin asset name entry identity changed.".into());
    }
    let matches =
        |expected: &[u8]| {
            if flags & 1 == 0 {
                &body[24..24 + expected.len()] == expected && body[24 + expected.len()] == 0
            } else {
                expected.iter().enumerate().all(|(offset, byte)| {
                    body[24 + offset * 2] == *byte && body[25 + offset * 2] == 0
                }) && body[24 + expected.len() * 2..26 + expected.len() * 2] == [0, 0]
            }
        };
    let color = if matches(b"PumpkinGreen_prop") {
        Some(false)
    } else if matches(b"PumpkinRed_prop") {
        Some(true)
    } else {
        None
    };
    // Unrelated names can grow/reallocate the table without changing this entry.
    let current = orchid::read::<16>(reader, pid, root_address)?;
    let current_table = orchid::u64_at(&current, 0);
    let current_count = orchid::u32_at(&current, 8);
    let current_capacity = orchid::u32_at(&current, 12);
    if !orchid::pointer(current_table)
        || index >= current_count
        || current_count > current_capacity
        || current_capacity > 1_000_000
        || orchid::field(reader, pid, current_table + u64::from(index) * 8)? != entry
    {
        return Err("Pumpkin asset name binding changed during lookup.".into());
    }
    Ok(color)
}

#[derive(Debug, Clone)]
pub struct JadeObject {
    pub instance: PawnRef,
    pub position: [f64; 3],
    pub prop: Option<PawnRef>,
}

struct LevelCache {
    signature: [u8; 36],
    members: Vec<EncodedAddress>,
    pumpkins: Vec<PawnRef>,
}

pub struct JadeSurvey {
    base: EncodedAddress,
    pid: u32,
    world: Option<PawnRef>,
    levels: HashMap<EncodedAddress, LevelCache>,
    next_level: usize,
    changed_this_survey: HashSet<EncodedAddress>,
    stable_this_survey: HashSet<EncodedAddress>,
    changed_captures: HashMap<EncodedAddress, u8>,
    last_unmatched: usize,
    pub scanned_levels: usize,
    pub total_levels: usize,
}

impl JadeSurvey {
    pub fn new(base: u64, pid: u32) -> Result<Self, String> {
        Ok(Self {
            base: EncodedAddress::new(base)?,
            pid,
            world: None,
            levels: HashMap::new(),
            next_level: 0,
            changed_this_survey: HashSet::new(),
            stable_this_survey: HashSet::new(),
            changed_captures: HashMap::new(),
            last_unmatched: 0,
            scanned_levels: 0,
            total_levels: 0,
        })
    }

    fn array(
        &self,
        reader: &AtlasReader,
        address: u64,
        limit: usize,
    ) -> Result<Option<Vec<EncodedAddress>>, String> {
        let before = orchid::read::<16>(reader, self.pid, address)?;
        let pointer = orchid::u64_at(&before, 0);
        let count = orchid::u32_at(&before, 8) as usize;
        let capacity = orchid::u32_at(&before, 12) as usize;
        if count > capacity || capacity > limit || (count != 0 && !orchid::pointer(pointer)) {
            return Err(
                "Pumpkin survey array bounds changed; waiting for a stable level registry.".into(),
            );
        }
        let mut bytes = vec![0; count * 8];
        for (index, chunk) in bytes.chunks_mut(4096).enumerate() {
            if let Err(error) = reader.read_exact(self.pid, pointer + (index * 4096) as u64, chunk)
            {
                if orchid::read::<16>(reader, self.pid, address)? != before {
                    crate::log_info!("Pumpkin array resized during read; withholding affected scope until next survey.");
                    return Ok(None);
                }
                return Err(error);
            }
        }
        if orchid::read::<16>(reader, self.pid, address)? != before {
            crate::log_info!("Pumpkin array membership changed during capture; withholding affected scope until next survey.");
            return Ok(None);
        }
        bytes
            .chunks_exact(8)
            .map(|entry| EncodedAddress::new(orchid::u64_at(entry, 0)))
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }

    fn withhold_changed(
        &mut self,
        reference: PawnRef,
        phase: &str,
    ) -> Result<Option<JadeObject>, String> {
        if self.changed_this_survey.insert(reference.address) {
            self.stable_this_survey.remove(&reference.address);
            let attempts = self.changed_captures.entry(reference.address).or_default();
            *attempts = attempts.saturating_add(1);
            crate::log_info!(
                "Pumpkin at 0x{:X}: {phase}; sample withheld, unstable capture {}/{}.",
                reference.address.get(),
                *attempts,
                MAX_CHANGED_CAPTURES
            );
            if *attempts >= MAX_CHANGED_CAPTURES {
                return Err(format!(
                    "Pumpkin at 0x{:X} remained unstable for {} captures; markers withheld. Wait for the level to settle and restart processing.",
                    reference.address.get(), MAX_CHANGED_CAPTURES
                ));
            }
        }
        Ok(None)
    }

    fn pumpkin(
        &mut self,
        reader: &AtlasReader,
        reference: PawnRef,
    ) -> Result<Option<JadeObject>, String> {
        let address = reference.address.get();
        let body = orchid::read::<0x310>(reader, self.pid, address)?;
        if orchid::kind(&body, self.base.get()) != PREFAB
            || orchid::signature(&body) != reference.signature
        {
            return self.withhold_changed(reference, "prefab identity changed before capture");
        }
        let asset = orchid::u64_at(&body, const { EncodedOffset::new(0x308) }.get() as usize);
        if !orchid::pointer(asset) {
            if self.changed_captures.contains_key(&reference.address) {
                self.stable_this_survey.insert(reference.address);
            }
            return Ok(None);
        }
        let asset_body = orchid::read::<64>(reader, self.pid, asset)?;
        if asset_color(reader, self.pid, self.base.get(), &asset_body)?.is_none() {
            if self.changed_captures.contains_key(&reference.address) {
                self.stable_this_survey.insert(reference.address);
            }
            return Ok(None);
        }
        let position = std::array::from_fn(|index| {
            let offset = const { EncodedOffset::new(0x88) }.get() as usize + index * 4;
            f32::from_le_bytes(body[offset..offset + 4].try_into().unwrap()) as f64
        });
        let final_header = orchid::read::<64>(reader, self.pid, address)?;
        if orchid::kind(&final_header, self.base.get()) != PREFAB
            || orchid::signature(&final_header) != reference.signature
            || orchid::field(
                reader,
                self.pid,
                address + const { EncodedOffset::new(0x308) }.get(),
            )? != asset
            || orchid::signature(&orchid::read::<64>(reader, self.pid, asset)?)
                != orchid::signature(&asset_body)
        {
            return self.withhold_changed(reference, "prefab or asset changed during capture");
        }
        if position
            .iter()
            .any(|value| !value.is_finite() || value.abs() > 10_000_000.0)
        {
            return Err("Pumpkin prefab contains invalid coordinates.".into());
        }
        if self.changed_captures.contains_key(&reference.address) {
            self.stable_this_survey.insert(reference.address);
        }
        Ok(Some(JadeObject {
            instance: reference,
            position,
            prop: None,
        }))
    }

    fn pumpkin_in_membership(
        &mut self,
        reader: &AtlasReader,
        reference: PawnRef,
        array_address: u64,
    ) -> Result<Option<JadeObject>, String> {
        // A later probe in this pass must not republish a sample already rejected here.
        if self.changed_this_survey.contains(&reference.address) {
            return Ok(None);
        }
        match self.pumpkin(reader, reference) {
            Ok(object) => Ok(object),
            Err(error) if self.changed_this_survey.contains(&reference.address) => Err(error),
            Err(error) => match self.array(reader, array_address, 131072)? {
                Some(current) if current.contains(&reference.address) => Err(error),
                _ => {
                    crate::log_info!("Pumpkin actor removed or membership changing during identity read; withholding its marker.");
                    Ok(None)
                }
            },
        }
    }

    fn active_green_props(
        &mut self,
        reader: &AtlasReader,
        world: PawnRef,
        persistent: u64,
        persistent_signature: [u8; 36],
        objects: Vec<JadeObject>,
    ) -> Result<Vec<JadeObject>, String> {
        let array_address = persistent + const { EncodedOffset::new(0xB4) }.get();
        let Some(members) = self.array(reader, array_address, 131072)? else {
            return Ok(Vec::new());
        };
        let mut props = Vec::new();
        for address in members
            .iter()
            .copied()
            .filter(|entry| orchid::pointer(entry.get()))
        {
            let header = match orchid::read::<64>(reader, self.pid, address.get()) {
                Ok(header) => header,
                Err(error) => match self.array(reader, array_address, 131072)? {
                    Some(current) if current.contains(&address) => return Err(error),
                    _ => {
                        crate::log_info!("Pumpkin prop membership changed during index read; withholding removed actor.");
                        continue;
                    }
                },
            };
            if orchid::kind(&header, self.base.get()) != PROP {
                continue;
            }
            let body = orchid::read::<0x300>(reader, self.pid, address.get())?;
            let reference = PawnRef {
                address,
                signature: orchid::signature(&header),
            };
            if orchid::signature(&body) != reference.signature
                || orchid::u64_at(&body, const { EncodedOffset::new(0x2E0) }.get() as usize)
                    != world.address.get()
            {
                continue;
            }
            let position: [f64; 3] = std::array::from_fn(|index| {
                let offset = const { EncodedOffset::new(0x88) }.get() as usize + index * 4;
                f32::from_le_bytes(body[offset..offset + 4].try_into().unwrap()) as f64
            });
            if position.iter().any(|value| !value.is_finite()) {
                return Err("Pumpkin prop position invalid.".into());
            }
            let active = orchid::u32_at(&body, const { EncodedOffset::new(0x7C) }.get() as usize)
                & 0x4000
                == 0
                && orchid::u32_at(&body, const { EncodedOffset::new(0x84) }.get() as usize) & 0x80
                    == 0;
            if orchid::typed(reader, self.pid, self.base.get(), address.get(), PROP)?
                != reference.signature
            {
                return Err("Pumpkin prop instance changed during capture.".into());
            }
            props.push((reference, position, active));
        }
        let Some(after_members) = self.array(reader, array_address, 131072)? else {
            return Ok(Vec::new());
        };
        props.retain(|(reference, _, _)| after_members.contains(&reference.address));
        if orchid::typed(reader, self.pid, self.base.get(), persistent, LEVEL)?
            != persistent_signature
            || orchid::typed(
                reader,
                self.pid,
                self.base.get(),
                world.address.get(),
                orchid::WORLD,
            )? != world.signature
            || orchid::field(
                reader,
                self.pid,
                world.address.get() + const { EncodedOffset::new(0x44) }.get(),
            )? != persistent
        {
            return Err("Pumpkin availability world binding changed.".into());
        }
        let mut eligible = Vec::new();
        let mut unmatched = 0;
        for mut object in objects {
            let asset = orchid::field(
                reader,
                self.pid,
                object.instance.address.get() + const { EncodedOffset::new(0x308) }.get(),
            )?;
            if !orchid::pointer(asset) {
                crate::log_info!(
                    "Pumpkin asset detached during availability capture; marker withheld."
                );
                continue;
            }
            let asset_header = orchid::read::<64>(reader, self.pid, asset)?;
            if asset_color(reader, self.pid, self.base.get(), &asset_header)?.is_none() {
                continue;
            }
            let mut matches = props.iter().filter(|(_, position, _)| {
                let expected = [
                    object.position[0],
                    object.position[1],
                    object.position[2] - 1.0,
                ];
                position
                    .iter()
                    .zip(expected)
                    .map(|(actual, expected)| (actual - expected).powi(2))
                    .sum::<f64>()
                    <= 9.0
            });
            let first = matches.next();
            if matches.next().is_some() {
                unmatched += 1;
                continue;
            }
            match first {
                Some((reference, _, true)) => {
                    object.prop = Some(*reference);
                    eligible.push(object);
                }
                Some(_) => {}
                None => unmatched += 1,
            }
        }
        // Logged on change only; the survey repeats several times per second.
        if unmatched != 0 && unmatched != self.last_unmatched {
            crate::log_info!(
                "Withheld {unmatched} pumpkin locations without a unique current prop association."
            );
        }
        self.last_unmatched = unmatched;
        Ok(eligible)
    }

    pub fn refresh(
        &mut self,
        reader: &AtlasReader,
        world: PawnRef,
    ) -> Result<Vec<JadeObject>, String> {
        let base = self.base.get();
        if orchid::typed(reader, self.pid, base, world.address.get(), orchid::WORLD)?
            != world.signature
        {
            return Err("Pumpkin survey WorldInfo instance changed.".into());
        }
        if self.world != Some(world) {
            self.levels.clear();
            self.next_level = 0;
            self.world = Some(world);
            self.changed_captures.clear();
        }
        self.changed_this_survey.clear();
        self.stable_this_survey.clear();
        let persistent = orchid::field(
            reader,
            self.pid,
            world.address.get() + const { EncodedOffset::new(0x44) }.get(),
        )?;
        let persistent_signature = orchid::typed(reader, self.pid, base, persistent, LEVEL)?;
        let owner = orchid::field(
            reader,
            self.pid,
            persistent + const { EncodedOffset::new(0x44) }.get(),
        )?;
        let owner_signature = orchid::typed(reader, self.pid, base, owner, OWNER)?;
        let registry_address = owner + const { EncodedOffset::new(0xEC) }.get();
        let Some(registry) = self.array(reader, registry_address, 4096)? else {
            self.levels.clear();
            self.scanned_levels = 0;
            return Ok(Vec::new());
        };
        if !registry.iter().any(|entry| entry.get() == persistent) {
            return Err("Pumpkin level registry does not contain the persistent level.".into());
        }
        let loaded: HashSet<_> = registry
            .iter()
            .copied()
            .filter(|entry| orchid::pointer(entry.get()))
            .collect();
        self.levels.retain(|level, _| loaded.contains(level));
        self.total_levels = loaded.len();
        let mut objects = Vec::new();
        // Bound discovery work; cached levels still receive fresh membership and instance checks.
        let selected: HashSet<_> = (0..registry.len().min(8))
            .map(|offset| registry[(self.next_level + offset) % registry.len()])
            .collect();
        if !registry.is_empty() {
            self.next_level = (self.next_level + 8) % registry.len();
        }
        for level in registry
            .iter()
            .copied()
            .filter(|entry| orchid::pointer(entry.get()))
        {
            if !selected.contains(&level) && !self.levels.contains_key(&level) {
                continue;
            }
            let level_signature = orchid::typed(reader, self.pid, base, level.get(), LEVEL)?;
            let array_address = level.get() + const { EncodedOffset::new(0xB4) }.get();
            let Some(members) = self.array(reader, array_address, 131072)? else {
                continue;
            };
            let cache = self
                .levels
                .get(&level)
                .filter(|cache| cache.signature == level_signature);
            // Membership changes schedule discovery; they do not invalidate unrelated live pumpkins.
            let changed = cache.map_or(true, |cache| cache.members != members);
            let retry = cache.is_some_and(|cache| {
                cache
                    .members
                    .iter()
                    .any(|address| self.changed_captures.contains_key(address))
            });
            let discover = (changed && selected.contains(&level)) || retry;
            let mut scanned_members = cache.map(|cache| cache.members.clone()).unwrap_or_default();
            let mut pumpkins: Vec<_> = cache
                .map(|cache| {
                    cache
                        .pumpkins
                        .iter()
                        .copied()
                        .filter(|reference| members.contains(&reference.address))
                        .collect()
                })
                .unwrap_or_default();
            if discover {
                for member in members
                    .iter()
                    .copied()
                    .filter(|entry| orchid::pointer(entry.get()))
                {
                    let header = match orchid::read::<64>(reader, self.pid, member.get()) {
                        Ok(header) => header,
                        Err(error) => match self.array(reader, array_address, 131072)? {
                            Some(current) if current.contains(&member) => return Err(error),
                            _ => {
                                crate::log_info!("Pumpkin actor removed or membership changing during read; excluding actor from this capture.");
                                continue;
                            }
                        },
                    };
                    if orchid::kind(&header, base) == PREFAB {
                        let reference = PawnRef {
                            address: member,
                            signature: orchid::signature(&header),
                        };
                        if self
                            .pumpkin_in_membership(reader, reference, array_address)?
                            .is_some()
                        {
                            if !pumpkins.contains(&reference) {
                                pumpkins.push(reference);
                            }
                        }
                    } else {
                        if self.changed_captures.contains_key(&member) {
                            self.stable_this_survey.insert(member);
                        }
                    }
                }
                scanned_members = members.clone();
            }
            let mut current = Vec::new();
            for reference in &pumpkins {
                if let Some(object) =
                    self.pumpkin_in_membership(reader, *reference, array_address)?
                {
                    current.push(object);
                }
            }
            let Some(after_members) = self.array(reader, array_address, 131072)? else {
                crate::log_info!(
                    "Pumpkin level has no stable final membership; withholding its markers until next survey."
                );
                continue;
            };
            if after_members != members {
                crate::log_info!("Pumpkin level membership changed; rechecking individual retained instances, scheduling new-object discovery.");
                current.retain(|object| after_members.contains(&object.instance.address));
                let mut refreshed = Vec::with_capacity(current.len());
                for object in current {
                    if let Some(fresh) =
                        self.pumpkin_in_membership(reader, object.instance, array_address)?
                    {
                        refreshed.push(fresh);
                    }
                }
                current = refreshed;
            }
            if orchid::typed(reader, self.pid, base, level.get(), LEVEL)? != level_signature {
                return Err("Pumpkin level instance changed during survey.".into());
            }
            pumpkins.retain(|reference| current.iter().any(|object| object.instance == *reference));
            self.levels.insert(
                level,
                LevelCache {
                    signature: level_signature,
                    members: scanned_members,
                    pumpkins,
                },
            );
            objects.extend(current);
        }
        if self.array(reader, registry_address, 4096)?.as_ref() != Some(&registry) {
            self.levels.clear();
            self.scanned_levels = 0;
            crate::log_info!(
                "Pumpkin loaded-level registry changed; withholding markers until next survey."
            );
            return Ok(Vec::new());
        }
        if orchid::typed(reader, self.pid, base, owner, OWNER)? != owner_signature
            || orchid::typed(reader, self.pid, base, persistent, LEVEL)? != persistent_signature
            || orchid::field(
                reader,
                self.pid,
                persistent + const { EncodedOffset::new(0x44) }.get(),
            )? != owner
            || orchid::field(
                reader,
                self.pid,
                world.address.get() + const { EncodedOffset::new(0x44) }.get(),
            )? != persistent
            || orchid::typed(reader, self.pid, base, world.address.get(), orchid::WORLD)?
                != world.signature
        {
            return Err("Pumpkin world registry changed during survey.".into());
        }
        self.scanned_levels = self.levels.len();
        self.changed_captures.retain(|address, _| {
            let stable = self.stable_this_survey.contains(address);
            if stable {
                crate::log_info!("Pumpkin at 0x{:X}: capture stabilized or became ineligible; retry state cleared.", address.get());
            }
            !stable && self.levels.values().any(|cache| cache.members.contains(address))
        });
        objects.retain(|object| !self.changed_this_survey.contains(&object.instance.address));
        self.active_green_props(reader, world, persistent, persistent_signature, objects)
    }
}
