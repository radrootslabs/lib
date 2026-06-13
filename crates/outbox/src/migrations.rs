#![forbid(unsafe_code)]

pub const OUTBOX_MIGRATION_UP: &str = include_str!("../migrations/0001_outbox.up.sql");
pub const OUTBOX_MIGRATION_DOWN: &str = include_str!("../migrations/0001_outbox.down.sql");
