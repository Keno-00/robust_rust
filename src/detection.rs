#[derive(Debug, Clone, Copy, Default)]
pub struct Target {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub seen: bool,
}
