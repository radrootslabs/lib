use radroots_events::{job_result::RadrootsJobResult, kinds::is_result_kind};

use crate::job::encode::{
    JobEncodeError, WireEventParts, assert_no_inputs_when_encrypted, canonicalize_tags,
};
use crate::job::util::{job_input_type_tag, push_amount_tag_msat};

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec, vec::Vec};

pub fn job_result_build_tags(res: &RadrootsJobResult) -> Vec<Vec<String>> {
    let mut tags: Vec<Vec<String>> = Vec::with_capacity(
        2
            + res.inputs.len()
            + usize::from(res.customer_pubkey.is_some())
            + usize::from(res.payment.is_some())
            + usize::from(res.encrypted),
    );

    let mut e = Vec::with_capacity(3);
    e.push("e".to_string());
    e.push(res.request_event.id.clone());
    if let Some(r) = &res.request_event.relays {
        e.push(r.clone());
    }
    tags.push(e);

    if let Some(j) = &res.request_json {
        tags.push(vec!["request".into(), j.clone()]);
    }

    if !res.encrypted {
        for i in &res.inputs {
            let mut t = Vec::with_capacity(5);
            t.push("i".to_string());
            t.push(i.data.clone());
            t.push(job_input_type_tag(i.input_type).to_string());
            if let Some(relay) = &i.relay {
                t.push(relay.clone());
            }
            if let Some(marker) = &i.marker {
                t.push(marker.clone());
            }
            tags.push(t);
        }
    }

    if let Some(p) = &res.customer_pubkey {
        tags.push(vec!["p".into(), p.clone()]);
    }

    if let Some(pay) = &res.payment {
        push_amount_tag_msat(&mut tags, pay.amount_sat, pay.bolt11.clone());
    }

    if res.encrypted {
        tags.push(vec!["encrypted".into()]);
    }

    tags
}

pub fn to_wire_parts(
    res: &RadrootsJobResult,
    content: &str,
) -> Result<WireEventParts, JobEncodeError> {
    let kind = res.kind as u32;
    if !is_result_kind(kind) {
        return Err(JobEncodeError::InvalidKind(kind));
    }

    let mut tags = job_result_build_tags(res);

    if res.encrypted && !assert_no_inputs_when_encrypted(&tags) {
        return Err(JobEncodeError::EmptyRequiredField("inputs-when-encrypted"));
    }

    canonicalize_tags(&mut tags);

    Ok(WireEventParts {
        kind,
        content: content.to_string(),
        tags,
    })
}
