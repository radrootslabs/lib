use crate::{Window, listing::FulfillmentMode};
use alloc::string::String;
use core::fmt;
use zeroize::Zeroizing;

/// Explicit private fulfillment data, without agreement or logistics authority.
/// Address, instructions and contact are zeroizing owned text; ordinary debug
/// and errors disclose none of it. Wire encoding belongs to private protocols.
#[derive(Clone, Eq, PartialEq)]
pub struct PrivateFulfillment {
    mode: FulfillmentMode,
    window: Window,
    address: Zeroizing<String>,
    instructions: Zeroizing<String>,
    contact: Option<Zeroizing<String>>,
}
impl PrivateFulfillment {
    /// Bounds exact private claims before copying. Contact is explicitly absent
    /// or provided; no endpoint, delivery or address-verification trust is minted.
    pub fn new(
        mode: FulfillmentMode,
        window: Window,
        address: &str,
        instructions: &str,
        contact: Option<&str>,
    ) -> Result<Self, FulfillmentError> {
        text(address, 2048)?;
        text(instructions, 2048)?;
        if let Some(contact) = contact {
            text(contact, 256)?;
        }
        Ok(Self {
            mode,
            window,
            address: Zeroizing::new(address.into()),
            instructions: Zeroizing::new(instructions.into()),
            contact: contact.map(|v| Zeroizing::new(v.into())),
        })
    }
    /// Returns the explicitly selected private mode.
    pub const fn mode(&self) -> FulfillmentMode {
        self.mode
    }
    /// Returns the exact required window without ambient time conversion.
    pub const fn window(&self) -> Window {
        self.window
    }
    /// Explicitly borrows private address text; callers own any disclosure.
    pub fn address(&self) -> &str {
        &self.address
    }
    /// Explicitly borrows private instructions.
    pub fn instructions(&self) -> &str {
        &self.instructions
    }
    /// Explicitly borrows the optional private contact claim.
    pub fn contact(&self) -> Option<&str> {
        self.contact.as_ref().map(|v| v.as_str())
    }
}
impl fmt::Debug for PrivateFulfillment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PrivateFulfillment(<redacted>)")
    }
}
fn text(value: &str, maximum: usize) -> Result<(), FulfillmentError> {
    if value.len() > maximum * 4 || value.chars().count() > maximum {
        return Err(FulfillmentError::InvalidText);
    }
    Ok(())
}
/// Input-free private fulfillment construction failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FulfillmentError {
    /// Private text exceeds its declared limit.
    InvalidText,
}
impl fmt::Display for FulfillmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid private fulfillment text")
    }
}
impl core::error::Error for FulfillmentError {}
