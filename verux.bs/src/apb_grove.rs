//! Bounded APB World pawn-list traversal for the captured executable build.
//! Pointer fields can be four-byte aligned even when their targets are aligned to eight.

use crate::lattice_encoding::EncodedAddress;
use crate::apb_orchid::{kind, pointer, signature, u64_at, PawnRef, PAWN, VEHICLE, WORLD};
use std::collections::HashSet;

pub const WORLD_PAWN_HEAD: usize = 0x48C;
pub const PAWN_NEXT: usize = 0x350;
const PAWN_WORLD: usize = 0x2E0;
const MAX_NODES: usize = 512;
const CHANGE_POLL_NODES: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct RosterLink {
    pub node: PawnRef,
    pub next: EncodedAddress,
}

/// Mutation hints only: every change still requires a complete guarded roster.
/// A bounded round-robin avoids walking the entire district on every poll.
#[derive(Default)]
pub struct RosterChangeMonitor {
    cursor: usize,
}

impl RosterChangeMonitor {
    pub fn reset(&mut self) {
        self.cursor = 0;
    }

    pub fn changed(
        &mut self,
        base: u64,
        world: PawnRef,
        head: u64,
        links: &[RosterLink],
        mut read: impl FnMut(u64, &mut [u8]) -> Result<(), String>,
    ) -> Result<bool, String> {
        let mut root = [0; WORLD_PAWN_HEAD + 8];
        read(world.address.get(), &mut root)?;
        if kind(&root, base) != WORLD
            || signature(&root) != world.signature
            || u64_at(&root, WORLD_PAWN_HEAD) != head
        {
            return Ok(true);
        }
        for _ in 0..links.len().min(CHANGE_POLL_NODES) {
            self.cursor %= links.len();
            let link = links[self.cursor];
            self.cursor += 1;
            let mut body = [0; PAWN_NEXT + 8];
            read(link.node.address.get(), &mut body)?;
            if signature(&body) != link.node.signature
                || u64_at(&body, PAWN_WORLD) != world.address.get()
                || u64_at(&body, PAWN_NEXT) != link.next.get()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[derive(Debug)]
pub struct PawnRoster {
    pub actors: Vec<PawnRef>,
    pub head: EncodedAddress,
    pub nodes: usize,
    pub vehicles: usize,
    pub links: Vec<RosterLink>,
}

pub enum RosterRead {
    Complete(PawnRoster),
    Changed(String),
}

/// The reader is shared by the live EchoDriver path and the saved-canvas probe.
/// Changed links reject the entire traversal; no partial roster is returned.
pub fn read_roster(
    base: u64,
    world: PawnRef,
    mut read: impl FnMut(u64, &mut [u8]) -> Result<(), String>,
) -> Result<RosterRead, String> {
    if !pointer(world.address.get()) {
        return Err("APB roster World pointer is invalid.".into());
    }
    let mut root = [0; WORLD_PAWN_HEAD + 8];
    read(world.address.get(), &mut root)?;
    if kind(&root, base) != WORLD || signature(&root) != world.signature {
        return Ok(RosterRead::Changed(
            "World instance changed before traversal".into(),
        ));
    }
    let head = u64_at(&root, WORLD_PAWN_HEAD);
    let mut next = head;
    let mut visited = HashSet::new();
    let mut nodes = Vec::new();
    let mut actors = Vec::new();
    let mut vehicles = 0;
    while next != 0 {
        if nodes.len() >= MAX_NODES {
            return Err(format!(
                "APB rooted roster exceeded {MAX_NODES} nodes at 0x{next:X}; targets withheld. Inspect verux_apb.log and recapture the pawn-list layout."
            ));
        }
        if !pointer(next) || !visited.insert(next) {
            return Err(format!(
                "APB rooted roster has an invalid pointer or cycle at 0x{next:X}; targets withheld. Inspect verux_apb.log and recapture the pawn-list layout."
            ));
        }
        let address = next;
        let mut body = [0; PAWN_NEXT + 8];
        read(address, &mut body)?;
        let node_type = kind(&body, base);
        match node_type {
            value if value == PAWN => actors.push(PawnRef { address: EncodedAddress::new(address)?, signature: signature(&body) }),
            value if value == VEHICLE => vehicles += 1,
            other => return Err(format!(
                "APB rooted roster node 0x{address:X} has unsupported type {other:X?}; targets withheld. Capture this node's class and list fields before using Memory coordinates."
            )),
        }
        if u64_at(&body, PAWN_WORLD) != world.address.get() {
            return Ok(RosterRead::Changed(format!(
                "Pawn 0x{address:X} changed World during traversal"
            )));
        }
        next = u64_at(&body, PAWN_NEXT);
        nodes.push((
            PawnRef {
                address: EncodedAddress::new(address)?,
                signature: signature(&body),
            },
            node_type,
            next,
        ));
    }
    // Recheck every edge and fingerprint, rather than treating an unchanged head
    // as proof that the interior of a dynamic linked list stayed unchanged.
    for (node, node_type, next) in &nodes {
        let mut body = [0; PAWN_NEXT + 8];
        read(node.address.get(), &mut body)?;
        if kind(&body, base) != *node_type
            || signature(&body) != node.signature
            || u64_at(&body, PAWN_WORLD) != world.address.get()
            || u64_at(&body, PAWN_NEXT) != *next
        {
            return Ok(RosterRead::Changed(format!(
                "Pawn 0x{:X} or its next link changed during traversal",
                node.address.get()
            )));
        }
    }
    read(world.address.get(), &mut root)?;
    if kind(&root, base) != WORLD
        || signature(&root) != world.signature
        || u64_at(&root, WORLD_PAWN_HEAD) != head
    {
        return Ok(RosterRead::Changed(
            "World or pawn-list head changed during traversal".into(),
        ));
    }
    actors.sort_unstable_by_key(|pawn| pawn.address.get());
    Ok(RosterRead::Complete(PawnRoster {
        actors,
        head: EncodedAddress::new(head)?,
        nodes: nodes.len(),
        vehicles,
        links: nodes
            .into_iter()
            .map(|(node, _, next)| Ok(RosterLink { node, next: EncodedAddress::new(next)? }))
            .collect::<Result<Vec<_>, String>>()?,
    }))
}
