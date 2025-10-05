#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Light {
    Red,
    Yellow,
    Green,
}

#[derive(Debug, Clone)]
pub struct NostrConnectionSnapshot {
    pub light: Light,
    pub connected: usize,
    pub connecting: usize,
    pub last_error: Option<String>,
}
