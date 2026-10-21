use super::Error;
use js_sys::JsString;
use radroots_domain::{CorrelationId, OperationId, PublicKey, SourceId, ThreadId, UnixSeconds};
use wasm_bindgen::prelude::*;

/// Pure untrusted first-issue descriptor, captured only against a real scope.
#[wasm_bindgen]
pub struct CommandRequest {
    operation: OperationId,
    deadline: UnixSeconds,
    correlation: Option<CorrelationId>,
}
#[wasm_bindgen]
impl CommandRequest {
    #[wasm_bindgen(constructor)]
    pub fn new(
        operation_id: JsString,
        first_issue_not_after: JsString,
        correlation_id: Option<JsString>,
    ) -> Result<CommandRequest, JsValue> {
        Ok(Self {
            operation: bounded_string(operation_id, 64, "operation_id")?
                .parse()
                .map_err(|_| Error::input("operation_id"))?,
            deadline: bounded_string(first_issue_not_after, 12, "first_issue_not_after")?
                .parse()
                .map_err(|_| Error::input("first_issue_not_after"))?,
            correlation: correlation_id
                .map(|v| {
                    bounded_string(v, 128, "correlation_id").and_then(|v| {
                        CorrelationId::new(v).map_err(|_| Error::input("correlation_id"))
                    })
                })
                .transpose()?,
        })
    }
}
impl CommandRequest {
    pub(super) fn capture(
        &self,
        scope: &crate::WorkspaceSession,
    ) -> Result<crate::CommandContext, Error> {
        scope
            .command_context_with_correlation(
                self.operation,
                None,
                None,
                self.deadline,
                self.correlation.clone(),
            )
            .map_err(Error::from)
    }
}
/// Shared bounded initial listing input. Parsing has no host effects.
#[wasm_bindgen]
pub struct ListingInput {
    pub(super) inner: radroots_protocol::listing::PublishInput,
}
#[wasm_bindgen]
impl ListingInput {
    pub fn parse(input: js_sys::Uint8Array) -> Result<ListingInput, JsValue> {
        if !input.is_instance_of::<js_sys::Uint8Array>() {
            return Err(Error::input("listing_input").into());
        }
        if input.length() as usize > radroots_protocol::command::COMMAND_MAX_BYTES {
            return Err(Error::command(
                "listing_input",
                radroots_protocol::command::CommandError::LimitExceeded,
            )
            .into());
        }
        let inner = radroots_protocol::listing::PublishInput::parse(&input.to_vec())
            .map_err(|v| Error::command("listing_input", v))?;
        Ok(Self { inner })
    }
}
/// Canonical public reference descriptor, without event or terms authentication.
#[wasm_bindgen]
pub struct ListingReference {
    inner: radroots_execution::inbox::ListingRevisionRef,
}
#[wasm_bindgen]
impl ListingReference {
    #[wasm_bindgen(constructor)]
    pub fn new(
        author: JsString,
        discriminator: JsString,
        event_id: JsString,
    ) -> Result<ListingReference, JsValue> {
        let author = public_key(
            &bounded_string(author, 64, "listing_author")?,
            "listing_author",
        )?;
        let discriminator = bounded_string(discriminator, 32, "listing_discriminator")?;
        let event: SourceId = bounded_string(event_id, 64, "listing_event_id")?
            .parse()
            .map_err(|_| Error::input("listing_event_id"))?;
        let inner =
            radroots_execution::inbox::ListingRevisionRef::new(author, &discriminator, event)
                .map_err(|v| Error::command("listing_reference", v))?;
        Ok(Self { inner })
    }
}
/// Private input retains the shared zeroizing owner, with no foreign text cache.
#[wasm_bindgen]
pub struct InquiryInput {
    inner: radroots_execution::inbox::InquireInput,
}
#[wasm_bindgen]
impl InquiryInput {
    #[wasm_bindgen(constructor)]
    pub fn new(
        thread_id: JsString,
        peer: JsString,
        source: Option<ListingReference>,
        text: JsString,
    ) -> Result<InquiryInput, JsValue> {
        let text = zeroize::Zeroizing::new(bounded_string(
            text,
            radroots_domain::CHAT_MAX_BYTES,
            "inquiry_input",
        )?);
        let thread: ThreadId = bounded_string(thread_id, 64, "thread_id")?
            .parse()
            .map_err(|_| Error::input("thread_id"))?;
        let peer = public_key(&bounded_string(peer, 64, "peer")?, "peer")?;
        let inner = radroots_execution::inbox::InquireInput::new(
            thread,
            peer,
            source.map(|v| v.inner),
            &text,
        )
        .map_err(|v| Error::command("inquiry_input", v))?;
        Ok(Self { inner })
    }
}
impl InquiryInput {
    pub(super) fn captured_copy(&self) -> Result<radroots_execution::inbox::InquireInput, Error> {
        radroots_execution::inbox::InquireInput::new(
            self.inner.thread_id(),
            self.inner.peer(),
            self.inner.source().cloned(),
            self.inner.text(),
        )
        .map_err(|v| Error::command("inquiry_input", v))
    }
}
// Check UTF-16 size before allocating a Rust copy, reject malformed surrogate
// sequences instead of replacing them, then enforce exact UTF-8 size. Shared
// parsers still own every semantic invariant. Partial private copies zeroize.
pub(super) fn bounded_string(
    value: JsString,
    max: usize,
    field: &'static str,
) -> Result<String, Error> {
    if !value.is_string() {
        return Err(Error::input(field));
    }
    let units = value.length() as usize;
    let limit = || {
        Error::command(
            field,
            radroots_protocol::command::CommandError::LimitExceeded,
        )
    };
    if units > max {
        return Err(limit());
    }
    let mut output = zeroize::Zeroizing::new(String::with_capacity(units));
    for value in char::decode_utf16(value.iter()) {
        let value = value.map_err(|_| Error::input(field))?;
        if output.len() + value.len_utf8() > max {
            return Err(limit());
        }
        output.push(value);
    }
    Ok(std::mem::take(&mut *output))
}
fn public_key(value: &str, field: &'static str) -> Result<PublicKey, Error> {
    let key = PublicKey::from_hex(value).map_err(|_| Error::input(field))?;
    if key.to_hex() != value {
        return Err(Error::input(field));
    }
    Ok(key)
}
