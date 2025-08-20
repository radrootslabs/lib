use radroots_events::job::{
    request::models::{RadrootsJobInput, RadrootsJobParam},
    JobFeedbackStatus, JobInputType,
};

use crate::job::error::JobParseError;

fn looks_like_hex_id(s: &str) -> bool {
    let n = s.len();
    (n == 32 || n == 64) && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn looks_like_url_or_nostr(s: &str) -> bool {
    let ls = s.to_ascii_lowercase();
    ls.starts_with("http://")
        || ls.starts_with("https://")
        || ls.starts_with("nostr:")
        || ls.starts_with("note")
        || ls.starts_with("nevent")
        || ls.starts_with("naddr")
        || looks_like_hex_id(s)
}

fn looks_like_ws_relay(s: &str) -> bool {
    let ls = s.to_ascii_lowercase();
    ls.starts_with("ws://") || ls.starts_with("wss://")
}

pub fn parse_bool_encrypted(tags: &[Vec<String>]) -> bool {
    tags.iter()
        .any(|t| t.get(0).map(|s| s.as_str()) == Some("encrypted"))
}

#[inline]
pub fn job_input_type_tag(t: JobInputType) -> &'static str {
    match t {
        JobInputType::Url => "url",
        JobInputType::Event => "event",
        JobInputType::Job => "job",
        JobInputType::Text => "text",
    }
}

#[inline]
pub fn job_input_type_from_tag(s: &str) -> Option<JobInputType> {
    match s {
        "url" => Some(JobInputType::Url),
        "event" => Some(JobInputType::Event),
        "job" => Some(JobInputType::Job),
        "text" => Some(JobInputType::Text),
        _ => None,
    }
}

#[inline]
pub fn feedback_status_tag(s: JobFeedbackStatus) -> &'static str {
    match s {
        JobFeedbackStatus::PaymentRequired => "payment-required",
        JobFeedbackStatus::Processing => "processing",
        JobFeedbackStatus::Error => "error",
        JobFeedbackStatus::Success => "success",
        JobFeedbackStatus::Partial => "partial",
    }
}

#[inline]
pub fn feedback_status_from_tag(s: &str) -> Option<JobFeedbackStatus> {
    match s {
        "payment-required" => Some(JobFeedbackStatus::PaymentRequired),
        "processing" => Some(JobFeedbackStatus::Processing),
        "error" => Some(JobFeedbackStatus::Error),
        "success" => Some(JobFeedbackStatus::Success),
        "partial" => Some(JobFeedbackStatus::Partial),
        _ => None,
    }
}

pub fn parse_i_tags(tags: &[Vec<String>]) -> Vec<RadrootsJobInput> {
    let mut out = Vec::new();
    for t in tags
        .iter()
        .filter(|t| t.get(0).map(|s| s.as_str()) == Some("i"))
    {
        if t.len() < 2 {
            continue;
        }

        let mut data = String::new();
        let mut input_type = JobInputType::Text;
        let mut relay: Option<String> = None;
        let mut marker: Option<String> = None;

        match t.len() {
            2 => {
                let v = &t[1];
                if looks_like_url_or_nostr(v) {
                    data = v.clone();
                    let lv = v.to_ascii_lowercase();
                    input_type = if lv.starts_with("http://") || lv.starts_with("https://") {
                        JobInputType::Url
                    } else {
                        JobInputType::Event
                    };
                } else {
                    marker = Some(v.clone());
                }
            }
            3 => {
                data = t[1].clone();
                let v = t[2].as_str();
                if let Some(it) = job_input_type_from_tag(v) {
                    input_type = it;
                } else {
                    marker = Some(t[2].clone());
                }
            }
            4 => {
                data = t[1].clone();
                input_type = job_input_type_from_tag(t[2].as_str()).unwrap_or(JobInputType::Text);
                let v = &t[3];
                if looks_like_ws_relay(v) {
                    relay = Some(v.clone());
                } else if marker.is_none() {
                    marker = Some(v.clone());
                }
            }
            _ => {
                data = t[1].clone();
                input_type = job_input_type_from_tag(t[2].as_str()).unwrap_or(JobInputType::Text);
                if let Some(v) = t.get(3) {
                    if looks_like_ws_relay(v) {
                        relay = Some(v.clone());
                        if let Some(m) = t.get(4) {
                            marker = Some(m.clone());
                        }
                    } else {
                        marker = Some(v.clone());
                    }
                }
                if marker.is_none() {
                    if let Some(m) = t.get(4) {
                        marker = Some(m.clone());
                    }
                }
            }
        }

        out.push(RadrootsJobInput {
            data,
            input_type,
            relay,
            marker,
        });
    }
    out
}

pub fn parse_params(tags: &[Vec<String>]) -> Vec<RadrootsJobParam> {
    let mut params = Vec::new();
    for t in tags
        .iter()
        .filter(|t| t.get(0).map(|s| s.as_str()) == Some("param"))
    {
        if t.len() >= 3 {
            params.push(RadrootsJobParam {
                key: t[1].clone(),
                value: t[2].clone(),
            });
        }
    }
    params
}

pub fn parse_amount_tag_sat(
    tags: &[Vec<String>],
) -> Result<Option<(u32, Option<String>)>, JobParseError> {
    let amt = match tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("amount"))
    {
        Some(a) => a,
        None => return Ok(None),
    };
    let msat_s = amt.get(1).ok_or(JobParseError::InvalidTag("amount"))?;
    let msat_u64: u64 = msat_s
        .parse()
        .map_err(|e| JobParseError::InvalidNumber("amount", e))?;
    if msat_u64 % 1000 != 0 {
        return Err(JobParseError::NonWholeSats("amount"));
    }
    let sat_u64 = msat_u64 / 1000;
    if sat_u64 > (u32::MAX as u64) {
        return Err(JobParseError::AmountOverflow("amount"));
    }
    let bolt11 = amt.get(2).cloned();
    Ok(Some((sat_u64 as u32, bolt11)))
}

pub fn push_amount_tag_msat(tags: &mut Vec<Vec<String>>, sat: u32, bolt11: Option<String>) {
    let msat = (sat as u64) * 1000;
    let mut v = vec!["amount".into(), msat.to_string()];
    if let Some(b) = bolt11 {
        v.push(b);
    }
    tags.push(v);
}

pub fn parse_bid_tag_sat(tags: &[Vec<String>]) -> Result<Option<u32>, JobParseError> {
    let bid = match tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("bid"))
    {
        Some(b) => b,
        None => return Ok(None),
    };
    let msat_s = bid.get(1).ok_or(JobParseError::InvalidTag("bid"))?;
    let msat_u64: u64 = msat_s
        .parse()
        .map_err(|e| JobParseError::InvalidNumber("bid", e))?;
    if msat_u64 % 1000 != 0 {
        return Err(JobParseError::NonWholeSats("bid"));
    }
    let sat_u64 = msat_u64 / 1000;
    if sat_u64 > (u32::MAX as u64) {
        return Err(JobParseError::AmountOverflow("bid"));
    }
    Ok(Some(sat_u64 as u32))
}

pub fn push_bid_tag_msat(tags: &mut Vec<Vec<String>>, bid_sat: u32) {
    let msat = (bid_sat as u64) * 1000;
    tags.push(vec!["bid".into(), msat.to_string()]);
}
