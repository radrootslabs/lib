use core::fmt;

#[derive(Debug, Clone)]
pub struct WireEventParts {
    pub kind: u32,
    pub content: String,
    pub tags: Vec<Vec<String>>,
}

#[derive(Debug)]
pub enum JobEncodeError {
    MissingProvidersForEncrypted,
    InvalidKind(u32),
    EmptyRequiredField(&'static str),
}

impl fmt::Display for JobEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JobEncodeError::MissingProvidersForEncrypted => {
                write!(f, "encrypted=true requires at least one provider ('p') tag")
            }
            JobEncodeError::InvalidKind(k) => write!(f, "invalid job event kind: {}", k),
            JobEncodeError::EmptyRequiredField(n) => write!(f, "empty required field: {}", n),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for JobEncodeError {}

pub fn canonicalize_tags(tags: &mut Vec<Vec<String>>) {
    tags.retain(|t| t.first().map(|s| !s.trim().is_empty()).unwrap_or(false));
    for t in tags.iter_mut() {
        for s in t.iter_mut() {
            *s = s.trim().to_string();
        }
    }
    tags.sort_by(|a, b| a.first().cmp(&b.first()).then_with(|| a.cmp(b)));
    tags.dedup();
}

pub fn empty_content() -> String {
    String::new()
}

#[cfg(feature = "serde_json")]
pub fn json_content<T: serde::Serialize>(value: &T) -> Result<String, JobEncodeError> {
    serde_json::to_string(value).map_err(|_| JobEncodeError::EmptyRequiredField("content-json"))
}

#[derive(Debug, Clone)]
pub struct EventDraft {
    pub kind: u32,
    pub created_at: u32,
    pub author: String,
    pub content: String,
    pub tags: Vec<Vec<String>>,
}

pub fn to_draft(parts: WireEventParts, author: impl Into<String>, created_at: u32) -> EventDraft {
    EventDraft {
        kind: parts.kind,
        created_at,
        author: author.into(),
        content: parts.content,
        tags: parts.tags,
    }
}

pub fn push_status_tag(tags: &mut Vec<Vec<String>>, status: &str, extra: Option<&str>) {
    let mut v = vec!["status".into(), status.into()];
    if let Some(e) = extra {
        v.push(e.into());
    }
    tags.push(v);
}

pub fn push_provider_tag(tags: &mut Vec<Vec<String>>, p: &str) {
    tags.push(vec!["p".into(), p.into()]);
}

pub fn push_relay_tag(tags: &mut Vec<Vec<String>>, r: &str) {
    tags.push(vec!["relays".into(), r.into()]);
}

pub fn assert_no_inputs_when_encrypted(tags: &[Vec<String>]) -> bool {
    !tags
        .iter()
        .any(|t| t.get(0).map(|s| s == "i").unwrap_or(false))
}
