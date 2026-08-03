//! Side-effect-free client capability reporting.

use std::collections::{BTreeMap, BTreeSet};

/// Stable runtime identity for an SDK capability.
///
/// These values intentionally describe behavior rather than Cargo features.
/// Construction is private so every reported ID comes from the governed
/// catalog below.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CapabilityId(&'static str);

impl CapabilityId {
    /// Canonical storage operations.
    pub const CANONICAL_STORAGE: Self = Self("storage.canonical");
    /// Persistent storage operations.
    pub const PERSISTENT_STORAGE: Self = Self("storage.persistent");
    /// Storage backup and restore operations.
    pub const BACKUP_RESTORE: Self = Self("storage.backup-restore");
    /// Local signing.
    pub const LOCAL_SIGNING: Self = Self("signing.local");
    /// NIP-46 remote signing.
    pub const NIP46_SIGNING: Self = Self("signing.nip46");
    /// Nostr event fetch.
    pub const NOSTR_FETCH: Self = Self("transport.nostr.fetch");
    /// Nostr event delivery.
    pub const NOSTR_DELIVERY: Self = Self("transport.nostr.delivery");
    /// Reticulum event fetch preview.
    pub const RETICULUM_FETCH: Self = Self("transport.reticulum.fetch");
    /// Reticulum event delivery preview.
    pub const RETICULUM_DELIVERY: Self = Self("transport.reticulum.delivery");
    /// Mesh transport preview.
    pub const MESH_TRANSPORT: Self = Self("transport.mesh");
    /// SimpleX transport experiment.
    pub const SIMPLEX_TRANSPORT: Self = Self("transport.simplex");
    /// Daemon-mediated event delivery.
    pub const DAEMON_DELIVERY: Self = Self("transport.daemon.delivery");
    /// Inbound synchronization.
    pub const SYNC_PULL: Self = Self("sync.pull");
    /// Outbound synchronization.
    pub const SYNC_PUSH: Self = Self("sync.push");
    /// Farm event publication.
    pub const FARM_PUBLICATION: Self = Self("product.farm.publish");
    /// Listing event publication.
    pub const LISTING_PUBLICATION: Self = Self("product.listing.publish");
    /// Trade command execution.
    pub const TRADE_COMMANDS: Self = Self("product.trade.command");
    /// Trade queries.
    pub const TRADE_QUERIES: Self = Self("product.trade.query");
    /// Knowledge event support.
    pub const KNOWLEDGE_EVENTS: Self = Self("event.knowledge");

    /// Returns the stable presentation-independent identity.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for CapabilityId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Product maturity independent of runtime availability.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Maturity {
    /// Supported Release V1 behavior.
    Stable,
    /// Preserved pre-stable behavior with an explicit compatibility warning.
    Preview,
    /// Exploratory behavior without a compatibility commitment.
    Experimental,
}

/// Current runtime availability independent of compilation and configuration.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Availability {
    /// The configured capability is ready.
    Available,
    /// The configured capability is usable with reduced functionality.
    Degraded,
    /// The capability is compiled but not currently usable.
    Unavailable,
    /// The capability is not supported by this build.
    Unsupported,
}

/// One immutable capability observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityStatus {
    id: CapabilityId,
    compiled: bool,
    configured: bool,
    availability: Availability,
    maturity: Maturity,
}

impl CapabilityStatus {
    /// Returns the stable runtime identity.
    #[must_use]
    pub const fn id(self) -> CapabilityId {
        self.id
    }

    /// Returns whether support was compiled into this package.
    #[must_use]
    pub const fn is_compiled(self) -> bool {
        self.compiled
    }

    /// Returns whether the host configured the capability for this client.
    #[must_use]
    pub const fn is_configured(self) -> bool {
        self.configured
    }

    /// Returns the current side-effect-free availability observation.
    #[must_use]
    pub const fn availability(self) -> Availability {
        self.availability
    }

    /// Returns the independent product maturity classification.
    #[must_use]
    pub const fn maturity(self) -> Maturity {
        self.maturity
    }
}

/// Complete deterministic capability report for one client observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityReport {
    statuses: Vec<CapabilityStatus>,
}

impl CapabilityReport {
    /// Returns all known capabilities in stable catalog order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &CapabilityStatus> {
        self.statuses.iter()
    }

    /// Finds one capability by stable runtime identity.
    #[must_use]
    pub fn get(&self, id: CapabilityId) -> Option<CapabilityStatus> {
        self.statuses.iter().copied().find(|status| status.id == id)
    }
}

#[derive(Clone, Copy)]
struct Definition {
    id: CapabilityId,
    maturity: Maturity,
    compiled: bool,
}

const CATALOG: &[Definition] = &[
    Definition::stable(CapabilityId::CANONICAL_STORAGE, true),
    Definition::stable(CapabilityId::PERSISTENT_STORAGE, cfg!(feature = "sqlite")),
    Definition::stable(CapabilityId::BACKUP_RESTORE, true),
    Definition::stable(CapabilityId::LOCAL_SIGNING, cfg!(feature = "local-signing")),
    Definition::stable(CapabilityId::NIP46_SIGNING, cfg!(feature = "nip46")),
    Definition::stable(CapabilityId::NOSTR_FETCH, cfg!(feature = "nostr")),
    Definition::stable(CapabilityId::NOSTR_DELIVERY, cfg!(feature = "nostr")),
    Definition::preview(CapabilityId::RETICULUM_FETCH),
    Definition::preview(CapabilityId::RETICULUM_DELIVERY),
    Definition::experimental(CapabilityId::MESH_TRANSPORT),
    Definition::experimental(CapabilityId::SIMPLEX_TRANSPORT),
    Definition::stable(CapabilityId::DAEMON_DELIVERY, cfg!(feature = "radrootsd")),
    Definition::stable(CapabilityId::SYNC_PULL, cfg!(feature = "sync")),
    Definition::stable(CapabilityId::SYNC_PUSH, cfg!(feature = "sync")),
    Definition::stable(CapabilityId::FARM_PUBLICATION, true),
    Definition::stable(CapabilityId::LISTING_PUBLICATION, true),
    Definition::stable(CapabilityId::TRADE_COMMANDS, true),
    Definition::stable(CapabilityId::TRADE_QUERIES, true),
    Definition::stable(CapabilityId::KNOWLEDGE_EVENTS, cfg!(feature = "knowledge")),
];

impl Definition {
    const fn stable(id: CapabilityId, compiled: bool) -> Self {
        Self {
            id,
            maturity: Maturity::Stable,
            compiled,
        }
    }

    const fn preview(id: CapabilityId) -> Self {
        Self {
            id,
            maturity: Maturity::Preview,
            compiled: false,
        }
    }

    const fn experimental(id: CapabilityId) -> Self {
        Self {
            id,
            maturity: Maturity::Experimental,
            compiled: false,
        }
    }
}

pub(crate) struct Context<'a> {
    pub(crate) storage: bool,
    pub(crate) signer: bool,
    pub(crate) source: bool,
    pub(crate) sink: bool,
    pub(crate) sync: bool,
    pub(crate) lifecycle_availability: Availability,
    pub(crate) explicitly_configured: &'a BTreeSet<CapabilityId>,
    pub(crate) overrides: &'a BTreeMap<CapabilityId, Availability>,
}

pub(crate) fn report(context: Context<'_>) -> CapabilityReport {
    let statuses = CATALOG
        .iter()
        .map(|definition| {
            let configured = configured(definition.id, &context);
            let availability = if !definition.compiled {
                Availability::Unsupported
            } else if !configured {
                Availability::Unavailable
            } else if context.lifecycle_availability != Availability::Available {
                context.lifecycle_availability
            } else {
                context
                    .overrides
                    .get(&definition.id)
                    .copied()
                    .unwrap_or(context.lifecycle_availability)
            };
            CapabilityStatus {
                id: definition.id,
                compiled: definition.compiled,
                configured,
                availability,
                maturity: definition.maturity,
            }
        })
        .collect();
    CapabilityReport { statuses }
}

fn configured(id: CapabilityId, context: &Context<'_>) -> bool {
    match id {
        CapabilityId::CANONICAL_STORAGE
        | CapabilityId::BACKUP_RESTORE
        | CapabilityId::TRADE_QUERIES => context.storage,
        CapabilityId::SYNC_PULL => context.sync && context.source,
        CapabilityId::SYNC_PUSH => context.sync && context.sink,
        CapabilityId::FARM_PUBLICATION
        | CapabilityId::LISTING_PUBLICATION
        | CapabilityId::TRADE_COMMANDS => context.signer && context.sink,
        CapabilityId::KNOWLEDGE_EVENTS => cfg!(feature = "knowledge"),
        CapabilityId::RETICULUM_FETCH
        | CapabilityId::RETICULUM_DELIVERY
        | CapabilityId::MESH_TRANSPORT
        | CapabilityId::SIMPLEX_TRANSPORT => false,
        _ => context.explicitly_configured.contains(&id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_are_unique_and_not_feature_names() {
        for (index, definition) in CATALOG.iter().enumerate() {
            assert!(
                !CATALOG[..index]
                    .iter()
                    .any(|candidate| candidate.id == definition.id),
                "duplicate capability {}",
                definition.id
            );
            assert!(definition.id.as_str().contains('.'));
        }
    }

    #[test]
    fn maturity_and_unsupported_preview_states_are_independent() {
        let overrides = BTreeMap::new();
        let explicitly_configured = BTreeSet::new();
        let report = report(Context {
            storage: true,
            signer: false,
            source: false,
            sink: false,
            sync: false,
            lifecycle_availability: Availability::Available,
            explicitly_configured: &explicitly_configured,
            overrides: &overrides,
        });
        let storage = report
            .get(CapabilityId::CANONICAL_STORAGE)
            .expect("storage");
        assert!(storage.is_compiled());
        assert!(storage.is_configured());
        assert_eq!(storage.availability(), Availability::Available);
        assert_eq!(storage.maturity(), Maturity::Stable);

        let knowledge = report
            .get(CapabilityId::KNOWLEDGE_EVENTS)
            .expect("knowledge");
        assert_eq!(knowledge.is_compiled(), cfg!(feature = "knowledge"));
        assert_eq!(knowledge.is_configured(), cfg!(feature = "knowledge"));

        let reticulum = report
            .get(CapabilityId::RETICULUM_FETCH)
            .expect("reticulum");
        assert!(!reticulum.is_compiled());
        assert!(!reticulum.is_configured());
        assert_eq!(reticulum.availability(), Availability::Unsupported);
        assert_eq!(reticulum.maturity(), Maturity::Preview);

        let mesh = report.get(CapabilityId::MESH_TRANSPORT).expect("mesh");
        assert_eq!(mesh.maturity(), Maturity::Experimental);
    }
}
