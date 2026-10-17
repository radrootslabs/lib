use core::fmt;

macro_rules! diagnostic {
    ($name:ident, $description:literal, {$($variant:ident => $value:literal),+ $(,)?}) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[non_exhaustive]
        pub enum $name {
            $(#[doc = $value] $variant,)+
        }

        impl $name {
            /// Every supported diagnostic value.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Returns the fixed, input-free diagnostic string.
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $value,)+ }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

diagnostic!(Field, "A diagnostic field category, never a caller-supplied path.", {
    Input => "input",
    Currency => "currency",
    Unit => "unit",
    Source => "source",
    History => "history",
    Approval => "approval",
    Authority => "authority",
    Stock => "stock",
    Operation => "operation",
    Storage => "storage",
    Signer => "signer",
    Delivery => "delivery",
    Retention => "retention",
});

diagnostic!(Reason, "An input-free diagnostic reason.", {
    Missing => "missing",
    OutOfRange => "out_of_range",
    InvalidEncoding => "invalid_encoding",
    Unsupported => "unsupported",
    Conflict => "conflict",
    Unavailable => "unavailable",
    Unknown => "unknown",
});

/// Fixed-size redacted details with no arbitrary message, key or payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Detail {
    field: Field,
    reason: Reason,
}

impl Detail {
    /// Constructs one bounded diagnostic from supported categories.
    pub const fn new(field: Field, reason: Reason) -> Self {
        Self { field, reason }
    }

    /// Returns the field category.
    pub const fn field(self) -> Field {
        self.field
    }

    /// Returns the diagnostic reason.
    pub const fn reason(self) -> Reason {
        self.reason
    }
}
