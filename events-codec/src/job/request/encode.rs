use radroots_events::job::request::models::RadrootsJobRequest;

use crate::job::encode::{canonicalize_tags, JobEncodeError, WireEventParts};
use crate::job::util::{job_input_type_tag, push_bid_tag_msat};

pub fn job_request_build_tags(req: &RadrootsJobRequest) -> Vec<Vec<String>> {
    let mut tags: Vec<Vec<String>> = Vec::new();

    for i in &req.inputs {
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

    if let Some(out) = &req.output {
        tags.push(vec!["output".into(), out.clone()]);
    }

    for p in &req.params {
        tags.push(vec!["param".into(), p.key.clone(), p.value.clone()]);
    }

    if let Some(bid_sat) = req.bid_sat {
        push_bid_tag_msat(&mut tags, bid_sat);
    }

    for r in &req.relays {
        tags.push(vec!["relays".into(), r.clone()]);
    }

    for p in &req.providers {
        tags.push(vec!["p".into(), p.clone()]);
    }

    for t in &req.topics {
        tags.push(vec!["t".into(), t.clone()]);
    }

    if req.encrypted {
        tags.push(vec!["encrypted".into()]);
    }

    tags
}

pub fn to_wire_parts(
    req: &RadrootsJobRequest,
    content: &str,
) -> Result<WireEventParts, JobEncodeError> {
    let kind = req.kind as u32;
    if !(5000..=5999).contains(&kind) {
        return Err(JobEncodeError::InvalidKind(kind));
    }
    if req.encrypted && req.providers.is_empty() {
        return Err(JobEncodeError::MissingProvidersForEncrypted);
    }

    let mut tags = job_request_build_tags(req);
    canonicalize_tags(&mut tags);

    Ok(WireEventParts {
        kind,
        content: content.to_string(),
        tags,
    })
}
