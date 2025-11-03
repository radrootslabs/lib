use radroots_events::job::result::models::RadrootsJobResult;

use crate::job::encode::{
    assert_no_inputs_when_encrypted, canonicalize_tags, JobEncodeError, WireEventParts,
};
use crate::job::util::{job_input_type_tag, push_amount_tag_msat};

pub fn job_result_build_tags(res: &RadrootsJobResult) -> Vec<Vec<String>> {
    let mut tags: Vec<Vec<String>> = Vec::new();

    let mut e = vec!["e".to_string(), res.request_event.id.clone()];
    if let Some(r) = &res.request_event.relays {
        e.push(r.clone());
    }
    tags.push(e);

    if let Some(j) = &res.request_json {
        tags.push(vec!["request".into(), j.clone()]);
    }

    if !res.encrypted {
        for i in &res.inputs {
            let mut t = vec!["i".to_string(), i.data.clone()];
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
    if !(6000..=6999).contains(&kind) {
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
