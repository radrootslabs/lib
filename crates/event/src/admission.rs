//! Event admission and verification-state model.
//!
//! The final typestate surface is introduced by the ordered event refactor.

/// Internal marker for the target module while its typestates are introduced.
#[allow(dead_code)]
pub(crate) struct ModuleScaffold;
