//! Compile-time boundary for the Color edition.
//! Only shared interface shapes remain. Every memory operation fails explicitly;
//! the driver transport, object scans, offsets and memory targeting are absent.
fn unavailable() -> String {
    "Memory features are not included in this Color edition. Install the Color + Memory edition with an eligible license.".into()
}

pub mod apb_orchid {
    use crate::lattice_encoding::EncodedAddress;
    pub const APB_UNITS_PER_METER: f64 = 100.0;
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct PawnRef { pub address: EncodedAddress, pub signature: [u8; 36] }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MissionRole { Opponent, Teammate, OutsideMission, Unknown }
}

pub mod atlas_reader {
    pub struct AtlasReader;
    impl AtlasReader {
        pub fn connect() -> Result<Self, String> { Err(super::unavailable()) }
        pub fn read_exact(&self, _pid: u32, _address: u64, bytes: &mut [u8]) -> Result<(), String> { bytes.fill(0); Err(super::unavailable()) }
    }
}

pub mod echo_courier {
    pub fn echodriver_status() -> Result<String, String> { Err(super::unavailable()) }
    pub fn start_echodriver() -> Result<String, String> { Err(super::unavailable()) }
}

pub mod apb_spark {
    pub struct ApbCursorSource;
    impl ApbCursorSource {
        pub fn connect(_pid: u32) -> Result<Self, String> { Err(super::unavailable()) }
        pub fn should_fire(&mut self, _red: bool, _green: bool) -> Result<Option<bool>, String> { Err(super::unavailable()) }
    }
}

pub mod apb_grove {
    #[derive(Default)]
    pub struct RosterChangeMonitor;
    impl RosterChangeMonitor { pub fn reset(&mut self) {} }
}

pub mod atlas_unicorn {
    use super::apb_orchid::PawnRef;
    use crate::detection::Target;
    #[derive(Debug, Clone, Copy)]
    pub struct AtlasUnicornTarget {
        pub point: Target, pub pawn: PawnRef, pub top: f64, pub bottom: f64,
        pub body_bounds: [f64; 4], pub distance_units: Option<f64>,
    }
    #[derive(Default)]
    pub struct VerticalCorrection;
    pub fn approximate_delta(_target: AtlasUnicornTarget, _mode: crate::config::VerticalTracking, _points: [bool; 4]) -> i32 {
        panic!("{}", super::unavailable())
    }
    impl VerticalCorrection {
        pub fn reset(&mut self) {}
        pub fn delta(&mut self, _target: AtlasUnicornTarget) -> i32 { panic!("{}", super::unavailable()) }
    }
}

pub mod apb_glow {
    use super::apb_orchid::PawnRef;
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ObjectKind { Jade, Wisp }
    #[derive(Debug, Clone, Copy)]
    pub struct ObjectPoint { pub kind: ObjectKind, pub red: bool, pub instance: PawnRef, pub position: [f32; 3] }
    pub struct ObjectWorkers;
    impl ObjectWorkers {
        pub fn new(_pid: u32, _base: u64, _jade: bool, _wisps: bool) -> Result<Self, String> { Err(super::unavailable()) }
        pub fn set_world(&self, _world: Option<PawnRef>) -> Result<(), String> { Err(super::unavailable()) }
        pub fn error(&self) -> Result<(), String> { Err(super::unavailable()) }
    }
}

pub mod apb_atlas {
    use super::{apb_glow::{ObjectPoint, ObjectWorkers}, apb_grove::RosterChangeMonitor, apb_orchid::{MissionRole, PawnRef}, atlas_reader::AtlasReader};
    use crate::{atlas_lantern::CameraProjection, atlas_unicorn::AtlasUnicornTarget, config::{LanternFilter, OrbitOrder}, lattice_encoding::EncodedAddress};
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::RECT;
    pub const ROSTER_REFRESH_INTERVAL: Duration = Duration::from_millis(500);
    pub const ROSTER_RETRY_INTERVAL: Duration = Duration::from_millis(50);
    pub const ROSTER_CHANGE_POLL_INTERVAL: Duration = Duration::from_millis(10);
    #[derive(Debug, Clone)]
    pub struct Discovery {
        pub pid: u32, pub base: EncodedAddress, pub actors: Vec<PawnRef>, pub nodes: usize,
        pub vehicles: usize, pub head: Option<EncodedAddress>, pub world_instance: Option<PawnRef>,
        pub captured_at: Instant, pub waiting_reason: Option<String>,
    }
    pub fn refresh(_reader: &AtlasReader, _current: &Discovery) -> Result<Discovery, String> { Err(super::unavailable()) }
    pub fn roster_changed(_reader: &AtlasReader, _current: &Discovery, _monitor: &mut RosterChangeMonitor) -> Result<bool, String> { Err(super::unavailable()) }
    #[derive(Debug, Clone, Copy)]
    pub struct ProjectedPawn {
        pub address: EncodedAddress, pub x: f64, pub y: f64, pub role: MissionRole, pub pawn: PawnRef,
        pub body_bounds: Option<[f64; 4]>, pub distance_units: Option<f64>, pub stable_recency: bool,
    }
    #[derive(Clone, Copy)]
    pub struct AtlasFrameRequest {
        pub unicorn: Option<LanternFilter>, pub visible_only: bool, pub comet: Option<LanternFilter>,
        pub projections: Option<LanternFilter>, pub esp_boxes: bool, pub distances: bool,
    }
    pub struct CometPawn { pub position: [f32; 3], pub role: MissionRole }
    pub struct AtlasFrame {
        pub world: PawnRef, pub visible_only: bool, pub comet_pawns: Vec<CometPawn>,
        pub comet_origin: Option<[f32; 3]>, pub camera: CameraProjection,
        pub points: Vec<ProjectedPawn>, pub client_rect: RECT,
    }
    impl AtlasFrame {
        pub fn target_with_priority(&self, _w: i32, _h: i32, _x: i32, _y: i32, _priority: OrbitOrder, _filter: LanternFilter) -> Option<AtlasUnicornTarget> {
            // A Color edition can never acquire a memory frame. This is an
            // invariant violation, not a substituted targeting implementation.
            panic!("{}", super::unavailable())
        }
    }
    pub struct AtlasSource { discovery: Discovery }
    impl AtlasSource {
        pub fn connect(_pid: u32) -> Result<Self, String> { Err(super::unavailable()) }
        pub fn discovery(&self) -> &Discovery { &self.discovery }
        pub fn replace_discovery(&mut self, _discovery: Discovery) -> Result<(), String> { Err(super::unavailable()) }
        pub fn take_refresh_request(&mut self) -> bool { false }
        pub fn frame_for(&mut self, _request: AtlasFrameRequest) -> Result<Option<AtlasFrame>, String> { Err(super::unavailable()) }
        pub fn object_points(&self, _workers: &ObjectWorkers, _world: PawnRef, _camera: [f32; 3], _radius: f64) -> Result<Vec<ObjectPoint>, String> { Err(super::unavailable()) }
    }
}
