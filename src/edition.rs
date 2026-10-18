//! Compile-time edition capability. This does not grant a license entitlement.
pub const fn memory_available() -> bool {
    cfg!(feature = "memory")
}

pub const fn id() -> &'static str {
    if memory_available() { "color-memory" } else { "color" }
}
