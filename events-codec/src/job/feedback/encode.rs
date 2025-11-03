use radroots_events::job::feedback::models::RadrootsJobFeedback;

use crate::job::encode::{canonicalize_tags, JobEncodeError, WireEventParts};
use crate::job::util::{feedback_status_tag, push_amount_tag_msat};

pub fn job_feedback_build_tags(fb: &RadrootsJobFeedback) -> Vec<Vec<String>> {
    let mut tags: Vec<Vec<String>> = Vec::new();

    let mut st = vec![
        "status".to_string(),
        feedback_status_tag(fb.status).to_string(),
    ];
    if let Some(info) = &fb.extra_info {
        st.push(info.clone());
    }
    tags.push(st);

    let mut e = vec!["e".to_string(), fb.request_event.id.clone()];
    if let Some(r) = &fb.request_event.relays {
        e.push(r.clone());
    }
    tags.push(e);

    if let Some(p) = &fb.customer_pubkey {
        tags.push(vec!["p".into(), p.clone()]);
    }

    if let Some(pay) = &fb.payment {
        push_amount_tag_msat(&mut tags, pay.amount_sat, pay.bolt11.clone());
    }

    if fb.encrypted {
        tags.push(vec!["encrypted".into()]);
    }

    tags
}

pub fn to_wire_parts(
    fb: &RadrootsJobFeedback,
    content: &str,
) -> Result<WireEventParts, JobEncodeError> {
    let kind = fb.kind as u32;
    if kind != 7000 {
        return Err(JobEncodeError::InvalidKind(kind));
    }

    let mut tags = job_feedback_build_tags(fb);
    canonicalize_tags(&mut tags);

    Ok(WireEventParts {
        kind,
        content: content.to_string(),
        tags,
    })
}
