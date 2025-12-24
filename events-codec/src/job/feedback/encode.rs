use radroots_events::{job_feedback::RadrootsJobFeedback, kinds::KIND_JOB_FEEDBACK};

use crate::job::encode::{JobEncodeError, WireEventParts, canonicalize_tags};
use crate::job::util::{feedback_status_tag, push_amount_tag_msat};

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec, vec::Vec};

pub fn job_feedback_build_tags(fb: &RadrootsJobFeedback) -> Vec<Vec<String>> {
    let mut tags: Vec<Vec<String>> = Vec::with_capacity(
        2
            + usize::from(fb.customer_pubkey.is_some())
            + usize::from(fb.payment.is_some())
            + usize::from(fb.encrypted),
    );

    let mut st = Vec::with_capacity(3);
    st.push("status".to_string());
    st.push(feedback_status_tag(fb.status).to_string());
    if let Some(info) = &fb.extra_info {
        st.push(info.clone());
    }
    tags.push(st);

    if let Some(pay) = &fb.payment {
        push_amount_tag_msat(&mut tags, pay.amount_sat, pay.bolt11.clone());
    }

    let mut e = Vec::with_capacity(3);
    e.push("e".to_string());
    e.push(fb.request_event.id.clone());
    if let Some(r) = &fb.request_event.relays {
        e.push(r.clone());
    }
    tags.push(e);

    if let Some(p) = &fb.customer_pubkey {
        tags.push(vec!["p".into(), p.clone()]);
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
    if kind != KIND_JOB_FEEDBACK {
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
