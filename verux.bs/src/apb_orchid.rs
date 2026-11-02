//! APB character ownership and mission fields for the hash-gated executable.
//! Fingerprints are instance checks, not persistent account identities.
use crate::lattice_encoding::{EncodedAddress, EncodedOffset, ObjectType};
use crate::atlas_reader::AtlasReader;

// 2026-10-18 capture and operator checks: docs/APB_CORE_UPDATE_20261018.md.
pub const PAWN: ObjectType = ObjectType::new(0x2AB7090, 5778);
pub const CAMERA: ObjectType = ObjectType::new(0x2B67088, 16929);
pub(crate) const WORLD: ObjectType = ObjectType::new(0x316FF28, 430);
const CONTROLLER: ObjectType = ObjectType::new(0x2A87258, 998);
pub(crate) const VEHICLE: ObjectType = ObjectType::new(0x2B320A8, 6064);
const COLLISION_COMPONENT: ObjectType = ObjectType::new(0x31A54F8, 3269);
// User-calibrated APB displacement: 8000 world units = 80 meters.
pub const APB_UNITS_PER_METER: f64 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PawnRef {
    pub address: EncodedAddress,
    pub signature: [u8; 36],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionRole {
    Opponent,
    Teammate,
    OutsideMission,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissionFields {
    pub token: u32,
    pub side: u32,
    relations: [u32; 2],
}

impl MissionFields {
    fn state(self) -> Option<bool> {
        if self.token == u32::MAX || [0x80, 0xC9, 0xD9].contains(&self.side) {
            Some(false)
        } else if self.side <= 1 && self.token & 0x80000000 != 0 {
            Some(true)
        } else {
            None
        }
    }
    pub fn classify(self, remote: Self) -> MissionRole {
        match (self.state(), remote.state()) {
            (None, _) | (_, None) => MissionRole::Unknown,
            (Some(false), _) | (_, Some(false)) => MissionRole::OutsideMission,
            _ if self.token != remote.token => MissionRole::OutsideMission,
            _ if self.side == remote.side => MissionRole::Teammate,
            _ => MissionRole::Opponent,
        }
    }
}

pub fn pointer(value: u64) -> bool {
    (0x10000..0x800000000000).contains(&value) && value % 8 == 0
}
pub fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
pub fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}
pub fn signature(bytes: &[u8]) -> [u8; 36] {
    let mut value = [0; 36];
    value[..8].copy_from_slice(&bytes[..8]);
    value[8..].copy_from_slice(&bytes[0x24..0x40]);
    crate::lattice_encoding::encode_fingerprint(value)
}
pub fn kind(bytes: &[u8], base: u64) -> (u64, u32) {
    (u64_at(bytes, 0).wrapping_sub(base), u32_at(bytes, 0x3C))
}
pub fn read<const N: usize>(
    reader: &AtlasReader,
    pid: u32,
    address: u64,
) -> Result<[u8; N], String> {
    let mut bytes = [0; N];
    reader.read_exact(pid, address, &mut bytes)?;
    Ok(bytes)
}
pub fn field(reader: &AtlasReader, pid: u32, address: u64) -> Result<u64, String> {
    Ok(u64::from_le_bytes(read(reader, pid, address)?))
}
pub fn typed(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    address: u64,
    expected: ObjectType,
) -> Result<[u8; 36], String> {
    if !pointer(address) {
        return Err(format!("APB ownership pointer unavailable: 0x{address:X}"));
    }
    let header = read::<64>(reader, pid, address)?;
    if kind(&header, base) != expected {
        return Err(format!(
            "APB object 0x{address:X} changed type; expected {expected:X?}"
        ));
    }
    Ok(signature(&header))
}
pub fn mission(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    pawn: PawnRef,
) -> Result<MissionFields, String> {
    if typed(reader, pid, base, pawn.address.get(), PAWN)? != pawn.signature {
        return Err("Pawn changed before mission read.".into());
    }
    let bytes = read::<16>(reader, pid, pawn.address.get() + const { EncodedOffset::new(0x990) }.get())?;
    if read::<16>(reader, pid, pawn.address.get() + const { EncodedOffset::new(0x990) }.get())? != bytes
        || typed(reader, pid, base, pawn.address.get(), PAWN)? != pawn.signature
    {
        return Err("Pawn or mission fields changed during sample.".into());
    }
    Ok(MissionFields {
        token: u32_at(&bytes, 0),
        side: u32_at(&bytes, 4),
        relations: [u32_at(&bytes, 8), u32_at(&bytes, 12)],
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalOwnership {
    pub world: EncodedAddress,
    pub controller: EncodedAddress,
    pub possessed: EncodedAddress,
    pub pawn: Option<PawnRef>,
    world_signature: [u8; 36],
    controller_signature: [u8; 36],
    vehicle_signature: Option<[u8; 36]>,
}

impl LocalOwnership {
    pub fn world_instance(self) -> PawnRef {
        PawnRef {
            address: self.world,
            signature: self.world_signature,
        }
    }
}

pub fn world_instance(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    address: u64,
) -> Result<PawnRef, String> {
    Ok(PawnRef {
        address: EncodedAddress::new(address)?,
        signature: typed(reader, pid, base, address, WORLD)?,
    })
}

pub fn local(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    camera: u64,
) -> Result<Option<LocalOwnership>, String> {
    let world = field(reader, pid, camera + const { EncodedOffset::new(0x2E0) }.get())?;
    if world == 0 {
        return Ok(None);
    }
    let world_signature = typed(reader, pid, base, world, WORLD)?;
    let controller = field(reader, pid, world + const { EncodedOffset::new(0x358) }.get())?;
    if controller == 0 {
        return Ok(None);
    }
    let controller_signature = typed(reader, pid, base, controller, CONTROLLER)?;
    let possessed = field(reader, pid, controller + const { EncodedOffset::new(0x790) }.get())?;
    let mut pawn = None;
    let mut vehicle_signature = None;
    if possessed != 0 {
        if !pointer(possessed) {
            return Err("APB possessed pointer is invalid.".into());
        }
        let header = read::<64>(reader, pid, possessed)?;
        let address = match kind(&header, base) {
            value if value == PAWN => {
                if field(reader, pid, possessed + const { EncodedOffset::new(0x3E0) }.get())? != controller {
                    return Err("Local pawn/controller link changed.".into());
                }
                possessed
            }
            value if value == VEHICLE => {
                vehicle_signature = Some(signature(&header));
                let driver = field(reader, pid, controller + const { EncodedOffset::new(0x1380) }.get())?;
                if !pointer(driver)
                    || field(reader, pid, controller + const { EncodedOffset::new(0x1388) }.get())? != driver
                    || field(reader, pid, driver + const { EncodedOffset::new(0x178) }.get())? != possessed
                    || field(reader, pid, possessed + const { EncodedOffset::new(0x3E0) }.get())? != controller
                {
                    return Err("APB driver/character/vehicle links do not agree.".into());
                }
                driver
            }
            other => return Err(format!("Unsupported APB possession type: {other:X?}.")),
        };
        pawn = Some(PawnRef {
            address: EncodedAddress::new(address)?,
            signature: typed(reader, pid, base, address, PAWN)?,
        });
    }
    let value = LocalOwnership {
        world: EncodedAddress::new(world)?,
        controller: EncodedAddress::new(controller)?,
        possessed: EncodedAddress::new(possessed)?,
        pawn,
        world_signature,
        controller_signature,
        vehicle_signature,
    };
    if !local_unchanged(reader, pid, base, camera, value)? {
        return Err("Local APB ownership changed during sample.".into());
    }
    Ok(Some(value))
}

pub fn local_unchanged(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    camera: u64,
    value: LocalOwnership,
) -> Result<bool, String> {
    if field(reader, pid, camera + const { EncodedOffset::new(0x2E0) }.get())? != value.world.get()
        || field(reader, pid, value.world.get() + const { EncodedOffset::new(0x358) }.get())? != value.controller.get()
        || field(reader, pid, value.controller.get() + const { EncodedOffset::new(0x790) }.get())? != value.possessed.get()
        || typed(reader, pid, base, value.world.get(), WORLD)? != value.world_signature
        || typed(reader, pid, base, value.controller.get(), CONTROLLER)? != value.controller_signature
    {
        return Ok(false);
    }
    if let Some(pawn) = value.pawn {
        if typed(reader, pid, base, pawn.address.get(), PAWN)? != pawn.signature {
            return Ok(false);
        }
        if let Some(vehicle_signature) = value.vehicle_signature {
            if typed(reader, pid, base, value.possessed.get(), VEHICLE)? != vehicle_signature
                || field(reader, pid, value.controller.get() + const { EncodedOffset::new(0x1380) }.get())? != pawn.address.get()
                || field(reader, pid, value.controller.get() + const { EncodedOffset::new(0x1388) }.get())? != pawn.address.get()
                || field(reader, pid, pawn.address.get() + const { EncodedOffset::new(0x178) }.get())? != value.possessed.get()
                || field(reader, pid, value.possessed.get() + const { EncodedOffset::new(0x3E0) }.get())? != value.controller.get()
            {
                return Ok(false);
            }
        } else if field(reader, pid, pawn.address.get() + const { EncodedOffset::new(0x3E0) }.get())? != value.controller.get() {
            return Ok(false);
        }
    }
    Ok(true)
}

pub struct OwnedPosition {
    pub position: [f32; 3],
    pub controller: EncodedAddress,
    pub world: EncodedAddress,
}

/// The local pawn already has reciprocal controller/possession proof from
/// `local`; do not enumerate its components again just to measure distance.
pub fn local_position(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    local: LocalOwnership,
) -> Result<Option<[f32; 3]>, String> {
    let Some(pawn) = local.pawn else {
        return Ok(None);
    };
    let body = read::<0x2E8>(reader, pid, pawn.address.get())?;
    if kind(&body, base) != PAWN
        || signature(&body) != pawn.signature
        || u64_at(&body, 0x2E0) != local.world.get()
    {
        return Err("Local pawn instance/World changed during distance read.".into());
    }
    let position: [f32; 3] = std::array::from_fn(|i| {
        f32::from_le_bytes(body[0x88 + i * 4..0x8C + i * 4].try_into().unwrap())
    });
    if !position
        .iter()
        .all(|v| v.is_finite() && v.abs() < 1_000_000.0)
    {
        return Err("Local pawn XYZ is invalid for distance measurement.".into());
    }
    if typed(reader, pid, base, pawn.address.get(), PAWN)? != pawn.signature
        || field(reader, pid, pawn.address.get() + const { EncodedOffset::new(0x2E0) }.get())? != local.world.get()
    {
        return Err("Local pawn changed during distance measurement.".into());
    }
    Ok(Some(position))
}

#[derive(Debug, Clone, Copy)]
pub struct CollisionBounds {
    pub origin: [f32; 3],
    pub extent: [f32; 3],
}

pub fn collision_bounds(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    pawn: PawnRef,
) -> Result<CollisionBounds, String> {
    if typed(reader, pid, base, pawn.address.get(), PAWN)? != pawn.signature {
        return Err("Pawn changed before bounds read.".into());
    }
    let component = field(reader, pid, pawn.address.get() + const { EncodedOffset::new(0x248) }.get())?;
    let stamp = typed(reader, pid, base, component, COLLISION_COMPONENT)?;
    let body = read::<0x240>(reader, pid, component)?;
    if signature(&body) != stamp
        || u64_at(&body, 0x44) != pawn.address.get()
        || u64_at(&body, 0x7C) != pawn.address.get()
    {
        return Err("Collision bounds component/pawn ownership changed.".into());
    }
    let float = |offset| f32::from_le_bytes(body[offset..offset + 4].try_into().unwrap());
    let origin: [f32; 3] = std::array::from_fn(|i| float(0x108 + i * 4));
    let extent: [f32; 3] = std::array::from_fn(|i| float(0x114 + i * 4));
    let sphere = float(0x120);
    let half_height = float(0x234);
    let radius = float(0x238);
    if !origin
        .iter()
        .all(|v| v.is_finite() && v.abs() < 1_000_000.0)
        || !extent
            .iter()
            .chain([&sphere, &half_height, &radius])
            .all(|v| v.is_finite() && *v > 0.0)
        || !(10.0..=300.0).contains(&half_height)
        || !(1.0..=150.0).contains(&radius)
        || (extent[0] - radius).abs() > 0.05
        || (extent[1] - radius).abs() > 0.05
        || (extent[2] - half_height).abs() > 0.05
        || (sphere - extent.iter().map(|v| v * v).sum::<f32>().sqrt()).abs() > 0.05
    {
        return Err("Collision bounds/height/radius do not match the recorded layout.".into());
    }
    if field(reader, pid, pawn.address.get() + const { EncodedOffset::new(0x248) }.get())? != component
        || typed(reader, pid, base, pawn.address.get(), PAWN)? != pawn.signature
        || typed(reader, pid, base, component, COLLISION_COMPONENT)? != stamp
        || field(reader, pid, component + const { EncodedOffset::new(0x44) }.get())? != pawn.address.get()
        || field(reader, pid, component + const { EncodedOffset::new(0x7C) }.get())? != pawn.address.get()
        || read::<8>(reader, pid, component + const { EncodedOffset::new(0x234) }.get())? != body[0x234..0x23C]
    {
        return Err("Collision bounds ownership or stance changed during sample.".into());
    }
    Ok(CollisionBounds { origin, extent })
}
fn collision_owner(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    pawn: PawnRef,
    component: u64,
) -> Result<PawnRef, String> {
    if !pointer(component) {
        return Err(
            "Canonical collision component is unavailable; character ownership withheld.".into(),
        );
    }
    let body = read::<0x84>(reader, pid, component)?;
    if kind(&body, base) != COLLISION_COMPONENT
        || u64_at(&body, 0x44) != pawn.address.get()
        || u64_at(&body, 0x7C) != pawn.address.get()
    {
        return Err("Canonical collision component/pawn ownership does not agree.".into());
    }
    Ok(PawnRef {
        address: EncodedAddress::new(component)?,
        signature: signature(&body),
    })
}

pub fn owned_position(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    pawn: PawnRef,
) -> Result<OwnedPosition, String> {
    let body = read::<0x410>(reader, pid, pawn.address.get())?;
    if kind(&body, base) != PAWN || signature(&body) != pawn.signature {
        return Err("Pawn changed during ownership read.".into());
    }
    let attachment = u64_at(&body, 0x178);
    let controller = u64_at(&body, 0x3E0);
    let world = u64_at(&body, 0x2E0);
    // Component registration changes when characters enter/leave vehicles.
    // The canonical collision slot has independently verified owner backlinks;
    // use the same character ownership route on both sides of that transition.
    // Attachment is rechecked as state, not dereferenced as ownership proof.
    let collision = collision_owner(reader, pid, base, pawn, u64_at(&body, 0x248))?;
    let position = std::array::from_fn(|i| {
        f32::from_le_bytes(body[0x88 + i * 4..0x8C + i * 4].try_into().unwrap())
    });
    if !position
        .iter()
        .all(|v| v.is_finite() && v.abs() < 1_000_000.0)
    {
        return Err("Pawn XYZ is invalid.".into());
    }
    if field(reader, pid, pawn.address.get() + const { EncodedOffset::new(0x178) }.get())? != attachment
        || field(reader, pid, pawn.address.get() + const { EncodedOffset::new(0x3E0) }.get())? != controller
        || field(reader, pid, pawn.address.get() + const { EncodedOffset::new(0x2E0) }.get())? != world
        || typed(reader, pid, base, pawn.address.get(), PAWN)? != pawn.signature
    {
        return Err("Pawn ownership changed during sample.".into());
    }
    if field(reader, pid, pawn.address.get() + const { EncodedOffset::new(0x248) }.get())? != collision.address.get()
        || collision_owner(reader, pid, base, pawn, collision.address.get())? != collision
    {
        return Err("Canonical collision ownership changed during sample.".into());
    }
    Ok(OwnedPosition {
        position,
        controller: EncodedAddress::new(controller)?,
        world: EncodedAddress::new(world)?,
    })
}
