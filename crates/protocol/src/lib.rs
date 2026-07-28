//! Versioned wire contracts for Radroots.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

/// Versioned capability catalog contracts.
pub mod capability {
    /// Capability contracts for generation 1.
    pub mod v1 {}
}

/// Versioned stable error-report contracts.
pub mod error {
    /// Error-report contracts for generation 1.
    pub mod v1 {}
}

/// Versioned event wire contracts.
pub mod event {
    /// Event wire contracts for generation 1.
    pub mod v1 {}
}

/// Versioned daemon protocol contracts.
pub mod radrootsd {
    /// Versioned transport-publish contracts.
    pub mod transport_publish {
        /// Transport-publish contracts for generation 5.
        pub mod v5 {}
    }
}

/// Versioned runtime operation contracts.
pub mod runtime {
    /// Runtime operation contracts for generation 1.
    pub mod v1 {}
}

/// Schema identity and structural validation contracts.
pub mod schema {}
