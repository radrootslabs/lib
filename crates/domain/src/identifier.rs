use core::{fmt, str::FromStr};

/// An identifier representation failure with no retained input or context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum IdentifierError {
    /// The string does not contain exactly 64 bytes.
    InvalidLength,
    /// A character is outside lowercase ASCII hexadecimal.
    InvalidEncoding,
}

impl fmt::Display for IdentifierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLength => "identifier must contain 64 hexadecimal characters",
            Self::InvalidEncoding => "identifier must use lowercase ASCII hexadecimal",
        })
    }
}

impl core::error::Error for IdentifierError {}

fn nibble(byte: u8) -> Result<u8, IdentifierError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(IdentifierError::InvalidEncoding),
    }
}

fn parse(input: &str) -> Result<[u8; 32], IdentifierError> {
    if input.len() != 64 {
        return Err(IdentifierError::InvalidLength);
    }
    let mut bytes = [0; 32];
    // Validated width gives exactly 32 complete pairs, with no remainder.
    let (pairs, _) = input.as_bytes().as_chunks::<2>();
    for (target, &[high, low]) in bytes.iter_mut().zip(pairs) {
        *target = nibble(high)? << 4 | nibble(low)?;
    }
    Ok(bytes)
}

macro_rules! identifier {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Constructs the fixed-width representation; generation belongs to
            /// the execution owner's qualified entropy boundary.
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            /// Borrows the exact identifier bytes without conversion.
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            /// Returns the exact identifier bytes without conversion.
            pub const fn into_bytes(self) -> [u8; 32] {
                self.0
            }
        }

        impl FromStr for $name {
            type Err = IdentifierError;

            fn from_str(input: &str) -> Result<Self, Self::Err> {
                parse(input).map(Self)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                for byte in self.0 {
                    write!(f, "{byte:02x}")?;
                }
                Ok(())
            }
        }

        #[cfg(feature = "serde")]
        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        #[cfg(feature = "serde")]
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct IdentifierVisitor;
                impl serde::de::Visitor<'_> for IdentifierVisitor {
                    type Value = $name;

                    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("a canonical lowercase hexadecimal identifier")
                    }

                    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<$name, E> {
                        value.parse().map_err(E::custom)
                    }
                }
                deserializer.deserialize_str(IdentifierVisitor)
            }
        }
    };
}

identifier!(OrderId, "An exact 256-bit bilateral order identifier.");
identifier!(
    ListingOptionId,
    "An exact 256-bit public sale-option identifier."
);
identifier!(
    AgreementLineId,
    "An exact 256-bit line identity within an agreement lineage."
);
identifier!(
    FulfillmentGroupId,
    "An exact 256-bit private fulfillment-group identity."
);
identifier!(ThreadId, "An exact 256-bit private thread identifier.");
identifier!(
    WorkspaceId,
    "An exact 256-bit participant workspace identifier."
);
identifier!(ExecutorId, "An exact 256-bit selected executor identifier.");
identifier!(
    OperationId,
    "An exact 256-bit retained operation identifier."
);
identifier!(
    TransactionId,
    "An exact 256-bit retained transaction identity."
);
identifier!(
    MessageId,
    "An unsigned NIP-17 rumor identity, distinct from its signed carriers."
);
identifier!(
    EnvelopeEventId,
    "A signed NIP-01 artifact identity, distinct from a logical record or rumor."
);
identifier!(
    GrantId,
    "An exact 256-bit scoped grant identifier; not a credential."
);
identifier!(
    RecordId,
    "A structured record content digest, distinct from rumor and envelope identities."
);
identifier!(
    SourceId,
    "A missing authenticated-source digest; provenance identifies its event or record family."
);
identifier!(
    CommandDigest,
    "A domain-separated exact command commitment."
);
identifier!(
    RouteDigest,
    "A commitment to one explicitly selected execution route."
);
identifier!(
    CheckpointDigest,
    "A checkpoint commitment; its bytes alone establish no coverage."
);
identifier!(
    ApprovalIntentDigest,
    "A commitment to the exact intent presented for approval."
);

identifier!(
    SigningAttemptId,
    "An exact 256-bit retained signing invocation identity, distinct from an operation or artifact."
);
