#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputBackend {
    Vigem,
    Arduino,
    MouseEvent,
    Ghub,
}
