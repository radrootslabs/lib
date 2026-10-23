//! Authenticated observed heads. Observation is neither a global current head,
//! a verified running capability nor permission to request or accept an order.
use crate::event_verify::{SignedReadError, signed_json_with_limits};
use alloc::boxed::Box;
use core::fmt;
use radroots_event::envelope::{
    EventEnvelope,
    event_head::{
        self, CurrentEventHead, EventHeadCandidateResult, EventHeadCoordinate, EventHeadDecision,
    },
};
use radroots_protocol::{
    advertisement::{self, Advertisement},
    command::CommandError,
    listing::{self, DecodedListing},
    nip01::EventFields,
    profile,
};

/// Fixed input-free public admission failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidInput,
    LimitExceeded,
    Unauthenticated,
    ScopeMismatch,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "invalid public input",
            Self::LimitExceeded => "public input limit exceeded",
            Self::Unauthenticated => "unauthenticated public event",
            Self::ScopeMismatch => "public scope mismatch",
        })
    }
}
impl core::error::Error for Error {}
impl From<SignedReadError> for Error {
    fn from(value: SignedReadError) -> Self {
        match value {
            SignedReadError::InvalidInput => Self::InvalidInput,
            SignedReadError::LimitExceeded => Self::LimitExceeded,
            SignedReadError::Unauthenticated => Self::Unauthenticated,
        }
    }
}

/// Selected authenticated evidence; no public constructor can forge provenance.
pub struct ObservedHead {
    scope: EventHeadCoordinate,
    current: Option<Selected>,
}
struct Selected {
    head: CurrentEventHead,
    event: EventEnvelope,
    raw: Box<str>,
}
impl fmt::Debug for ObservedHead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ObservedHead(<original authenticated evidence>)")
    }
}
/// Post-selection interpretation. Every non-known outcome retains the same
/// authentic head; callers cannot use an older event as an actionable fallback.
#[derive(Debug)]
pub enum Interpretation {
    Listing(Box<DecodedListing>),
    Capabilities(Advertisement),
    Unsupported,
    Invalid,
    Expired,
    ClockAhead,
}
impl ObservedHead {
    /// Captures one canonical public listing scope before copying its bounded d.
    pub fn listing(author: radroots_identity::PublicKey, d: &str) -> Result<Self, Error> {
        if d.len() != 32 {
            return Err(Error::InvalidInput);
        }
        Self::new(EventHeadCoordinate::Addressable {
            kind: listing::LISTING_KIND,
            pubkey: author,
            d_tag: d.into(),
        })
    }
    /// Captures the assigned per-author replaceable capability scope.
    pub fn capabilities(author: radroots_identity::PublicKey) -> Self {
        Self {
            scope: EventHeadCoordinate::Replaceable {
                kind: u32::from(profile::CAPABILITIES_KIND),
                pubkey: author,
            },
            current: None,
        }
    }
    /// Captures one explicit supported numeric coordinate. This allocates no
    /// network client, performs no IO and makes no observation claim.
    pub fn new(scope: EventHeadCoordinate) -> Result<Self, Error> {
        let valid = match &scope {
            EventHeadCoordinate::Replaceable { kind, .. } => {
                *kind == u32::from(profile::CAPABILITIES_KIND)
            }
            EventHeadCoordinate::Addressable { kind, d_tag, .. } => {
                *kind == listing::LISTING_KIND
                    && d_tag.len() == 32
                    && d_tag
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            }
        };
        if !valid {
            return Err(Error::InvalidInput);
        }
        Ok(Self {
            scope,
            current: None,
        })
    }
    /// Authenticate original bounded bytes before raw NIP01 ranking. Invalid
    /// signatures or foreign scopes never mutate the retained head. Only one
    /// original event and parsed representation are retained per scope.
    pub fn observe(&mut self, raw: &str) -> Result<EventHeadDecision, Error> {
        let event = signed_json_with_limits(
            raw,
            radroots_event::wire::EventWireLimits {
                max_raw_json_bytes: profile::SIGNED_EVENT_MAX_BYTES,
                max_content_bytes: profile::SIGNED_EVENT_MAX_BYTES,
                max_tag_element_bytes: profile::SIGNED_EVENT_MAX_BYTES,
                max_total_tag_bytes: profile::SIGNED_EVENT_MAX_BYTES,
                ..Default::default()
            },
        )?;
        let EventHeadCandidateResult::Candidate(candidate) =
            event_head::event_head_candidate_for_nip01_event(&event)
        else {
            return Err(Error::ScopeMismatch);
        };
        if candidate.coordinate != self.scope {
            return Err(Error::ScopeMismatch);
        }
        let decision =
            event_head::select_event_head(candidate, self.current.as_ref().map(|v| &v.head));
        if let EventHeadDecision::Applied(head) = &decision {
            self.current = Some(Selected {
                head: head.clone(),
                event,
                raw: Box::from(raw),
            });
        }
        Ok(decision)
    }
    /// Borrows the original authenticated selected object, including unsupported
    /// or invalid-profile evidence. Absence is not fabricated empty history.
    pub fn raw_json(&self) -> Option<&str> {
        self.current.as_ref().map(|v| v.raw.as_ref())
    }
    pub fn selected(&self) -> Option<&CurrentEventHead> {
        self.current.as_ref().map(|v| &v.head)
    }
    /// Evaluate only the selected head with caller-injected observation time.
    /// Any future author timestamp is flagged; no hidden skew, ambient clock,
    /// automatic purge or older-head fallback is introduced.
    pub fn interpret(&self, observed_at: advertisement::UnixSeconds) -> Option<Interpretation> {
        let Some(value) = &self.current else {
            return None;
        };
        let observed_at = observed_at.get();
        if value.event.created_at_u64() > observed_at {
            return Some(Interpretation::ClockAhead);
        }
        let event = &value.event;
        let result = match &self.scope {
            EventHeadCoordinate::Addressable { .. } => {
                let tags = event.tags_as_vec();
                let fields = EventFields {
                    author: *event.author(),
                    created_at: event.created_at_u64(),
                    kind: event.kind_u32(),
                    tags: &tags,
                    content: event.content(),
                };
                listing::decode_observed(&fields, *event.id().as_bytes())
                    .map(|v| Interpretation::Listing(Box::new(v)))
            }
            EventHeadCoordinate::Replaceable { .. } => {
                if !event.tag_slices().is_empty() {
                    return Some(Interpretation::Invalid);
                }
                advertisement::decode(event.content().as_bytes(), event.created_at_u64()).map(|v| {
                    if observed_at >= v.valid_until().get() {
                        Interpretation::Expired
                    } else {
                        Interpretation::Capabilities(v)
                    }
                })
            }
        };
        Some(match result {
            Ok(v) => v,
            Err(CommandError::UnsupportedProfile) => Interpretation::Unsupported,
            Err(_) => Interpretation::Invalid,
        })
    }
}
