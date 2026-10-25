//! Bounded causal checks over declared records. The caller must authenticate
//! every original source and bind complete local coverage to a store checkpoint.
//! A complete result grants no participant permission or physical stock effect.
use super::{CommerceContent, CommerceType};
use crate::{OrderId, PublicKey, RecordId, SourceId, ThreadId};
use alloc::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
    vec,
    vec::Vec,
};
use core::fmt;

/// One exact bilateral order scope, including seller quotes in its null-order
/// author chain. The scope itself is untrusted data, not evidence coverage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CausalScope {
    thread: ThreadId,
    order: OrderId,
    buyer: PublicKey,
    seller: PublicKey,
}
impl CausalScope {
    /// Requires distinct parties. Original participant authentication is separate.
    pub fn new(
        thread: ThreadId,
        order: OrderId,
        buyer: PublicKey,
        seller: PublicKey,
    ) -> Result<Self, CausalError> {
        if buyer == seller {
            return Err(CausalError::InvalidScope);
        }
        Ok(Self {
            thread,
            order,
            buyer,
            seller,
        })
    }
    /// Checks exact declared thread, participants and selected/null quote order.
    pub fn contains(self, content: &CommerceContent) -> bool {
        let header = content.header();
        header.thread_id() == self.thread
            && header.buyer() == self.buyer
            && header.seller() == self.seller
            && (header.order_id() == Some(self.order)
                || (header.order_id().is_none() && content.record_type() == CommerceType::Quote))
    }
    /// Returns the exact declared thread.
    pub const fn thread_id(self) -> ThreadId {
        self.thread
    }
    /// Returns the exact declared order.
    pub const fn order_id(self) -> OrderId {
        self.order
    }
    /// Returns the exact declared buyer.
    pub const fn buyer(self) -> PublicKey {
        self.buyer
    }
    /// Returns the exact declared seller.
    pub const fn seller(self) -> PublicKey {
        self.seller
    }
}

/// A borrowed content/identity claim. Hash, carrier signature and recipient
/// authentication must precede use; constructing this value does none of them.
#[derive(Clone, Copy)]
pub struct CausalRecord<'a> {
    id: RecordId,
    content: &'a CommerceContent,
}
impl<'a> CausalRecord<'a> {
    /// Borrows the complete content so duplicate identity comparisons cannot
    /// accidentally ignore a changed payload. No source bytes are copied.
    pub const fn new(id: RecordId, content: &'a CommerceContent) -> Self {
        Self { id, content }
    }
}
impl fmt::Debug for CausalRecord<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CausalRecord(<untrusted claim>)")
    }
}

/// Local causal classification; even Complete is not business actionability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CausalReadiness {
    /// All supplied required sources and author links are present and acyclic.
    Complete,
    /// Exact source identities are missing; fetch them before reducing actions.
    MissingHistory,
    /// An authenticated but uninterpreted source prevents a complete reduction.
    Uninterpreted,
    /// An author fork, inconsistent predecessor or causal cycle is exposed.
    Conflicted,
}
/// Deterministic, input-order-independent local coverage result. Original
/// evidence and installed agreement state remain owned by their caller/store.
#[derive(Clone, Eq, PartialEq)]
pub struct CausalAnalysis {
    readiness: CausalReadiness,
    buyer_head: Option<RecordId>,
    seller_head: Option<RecordId>,
    quote_head: Option<RecordId>,
    missing: Box<[SourceId]>,
    conflicts: Box<[RecordId]>,
    uninterpreted: Box<[SourceId]>,
    cycle: bool,
}
impl CausalAnalysis {
    /// Exact local coverage classification, without current business permission.
    pub const fn readiness(&self) -> CausalReadiness {
        self.readiness
    }
    /// Last contiguous uncontested buyer author-chain claim in the selected order.
    pub const fn buyer_head(&self) -> Option<RecordId> {
        self.buyer_head
    }
    /// Last contiguous uncontested seller author-chain claim in the selected order.
    pub const fn seller_head(&self) -> Option<RecordId> {
        self.seller_head
    }
    /// Last contiguous uncontested seller quote in the null-order chain.
    pub const fn quote_head(&self) -> Option<RecordId> {
        self.quote_head
    }
    /// Sorted unique exact missing references. No empty-state inference is made.
    pub fn missing(&self) -> &[SourceId] {
        &self.missing
    }
    /// Sorted exact conflicting records, including known-cycle descendants.
    pub fn conflicts(&self) -> &[RecordId] {
        &self.conflicts
    }
    /// Original retained uninterpreted identities that require reconciliation.
    pub fn uninterpreted(&self) -> &[SourceId] {
        &self.uninterpreted
    }
    /// Whether the supplied known-record dependency graph contains a cycle.
    pub const fn has_cycle(&self) -> bool {
        self.cycle
    }
}
impl fmt::Debug for CausalAnalysis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CausalAnalysis(<private coverage>)")
    }
}
/// Explicit resource/identity/scope failure, without private source contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CausalError {
    /// More than the contracted bounded records/source references were supplied.
    LimitExceeded,
    /// A supplied known record belongs to another thread, order or participant pair.
    InvalidScope,
    /// One asserted logical identity names different complete content.
    IdentityConflict,
}
impl fmt::Display for CausalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LimitExceeded => "causal graph limit exceeded",
            Self::InvalidScope => "causal scope differs",
            Self::IdentityConflict => "causal record identity conflict",
        })
    }
}
impl core::error::Error for CausalError {}

/// Maximum known records in one complete scoped causal evaluation. Larger
/// histories refuse explicitly; they are never silently truncated or paged as
/// though the first page were complete. Durable unrelated history is not capped.
pub const MAX_CAUSAL_RECORDS: usize = 1000;
/// Maximum separately verified external/uninterpreted source identities.
pub const MAX_CAUSAL_SOURCES: usize = MAX_CAUSAL_RECORDS * 8;

fn source(id: RecordId) -> SourceId {
    SourceId::from_bytes(id.into_bytes())
}

/// Checks the complete supplied local scoped graph with no recursion or clock.
/// External sources cannot stand in for an author predecessor. Uninterpreted
/// authenticated evidence remains a barrier, including when no known node
/// happens to reference it. Source completeness/authenticity remains a mandatory
/// store/ingress obligation; this pure function cannot discover omitted records.
pub fn analyze(
    scope: CausalScope,
    records: &[CausalRecord<'_>],
    external_sources: &[SourceId],
    uninterpreted_sources: &[SourceId],
) -> Result<CausalAnalysis, CausalError> {
    if records.len() > MAX_CAUSAL_RECORDS
        || external_sources.len() > MAX_CAUSAL_SOURCES
        || uninterpreted_sources.len() > MAX_CAUSAL_SOURCES
    {
        return Err(CausalError::LimitExceeded);
    }
    let mut unique = BTreeMap::<SourceId, CausalRecord<'_>>::new();
    for record in records {
        if !scope.contains(record.content) {
            return Err(CausalError::InvalidScope);
        }
        if let Some(previous) = unique.insert(source(record.id), *record)
            && previous.content != record.content
        {
            return Err(CausalError::IdentityConflict);
        }
    }
    // Stable identity ordering makes traversal and diagnostics independent of
    // relay arrival order and caller ordering. No timestamp wins a private fork.
    let nodes: Vec<_> = unique.values().copied().collect();
    let positions: BTreeMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (source(node.id), index))
        .collect();
    let external: BTreeSet<_> = external_sources.iter().copied().collect();
    let uninterpreted: BTreeSet<_> = uninterpreted_sources.iter().copied().collect();
    let mut slots = BTreeMap::new();
    let mut conflicts = BTreeSet::new();
    let mut missing = BTreeSet::new();
    for node in &nodes {
        let header = node.content.header();
        let slot = (
            header.author(),
            header.order_id(),
            header.author_sequence().get(),
        );
        if let Some(prior) = slots.insert(slot, node.id) {
            conflicts.insert(prior);
            conflicts.insert(node.id);
        }
        for parent in header.parents() {
            if !positions.contains_key(parent)
                && !external.contains(parent)
                && !uninterpreted.contains(parent)
            {
                missing.insert(*parent);
            }
        }
        if let Some(previous) = header.author_previous() {
            if let Some(index) = positions.get(&source(previous)) {
                let prior = nodes[*index].content.header();
                if prior.author() != header.author()
                    || prior.thread_id() != header.thread_id()
                    || prior.order_id() != header.order_id()
                    || prior.buyer() != header.buyer()
                    || prior.seller() != header.seller()
                    || prior.author_sequence().get().checked_add(1)
                        != Some(header.author_sequence().get())
                {
                    conflicts.insert(node.id);
                }
            } else {
                // A generic existing source or uninterpreted carrier cannot
                // prove the declared predecessor's author/scope/sequence.
                missing.insert(source(previous));
            }
        }
    }
    // Iterative topological traversal bounds both stack and work by the explicit
    // record budget. Missing external parents are reported separately; they do
    // not manufacture a false cycle in the known dependency subgraph.
    let mut visited = vec![false; nodes.len()];
    loop {
        let mut progress = false;
        for (index, node) in nodes.iter().enumerate() {
            if !visited[index]
                && node
                    .content
                    .header()
                    .parents()
                    .iter()
                    .all(|parent| positions.get(parent).is_none_or(|parent| visited[*parent]))
            {
                visited[index] = true;
                progress = true;
            }
        }
        if !progress {
            break;
        }
    }
    let cycle = visited.iter().any(|visited| !visited);
    for (node, visited) in nodes.iter().zip(&visited) {
        if !visited {
            conflicts.insert(node.id);
        }
    }
    let head = |author, order| {
        let mut previous = None;
        let mut sequence = 1;
        while let Some(id) = slots.get(&(author, order, sequence)).copied() {
            let node = &nodes[positions[&source(id)]];
            if conflicts.contains(&id) || node.content.header().author_previous() != previous {
                break;
            }
            previous = Some(id);
            sequence += 1;
        }
        previous
    };
    let readiness = if !conflicts.is_empty() {
        CausalReadiness::Conflicted
    } else if !uninterpreted.is_empty() {
        CausalReadiness::Uninterpreted
    } else if !missing.is_empty() {
        CausalReadiness::MissingHistory
    } else {
        CausalReadiness::Complete
    };
    Ok(CausalAnalysis {
        readiness,
        buyer_head: head(scope.buyer, Some(scope.order)),
        seller_head: head(scope.seller, Some(scope.order)),
        quote_head: head(scope.seller, None),
        missing: missing.into_iter().collect(),
        conflicts: conflicts.into_iter().collect(),
        uninterpreted: uninterpreted.into_iter().collect(),
        cycle,
    })
}
