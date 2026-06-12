#[cfg(not(feature = "std"))]
use alloc::string::String;

use crate::social::{RadrootsReportFileTarget, RadrootsReportType, RadrootsSocialTarget};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsReport {
    pub reported_pubkey: String,
    pub report_type: RadrootsReportType,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub event: Option<RadrootsSocialTarget>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub file: Option<RadrootsReportFileTarget>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_model_requires_reported_pubkey_field() {
        let report = RadrootsReport {
            reported_pubkey: "a".repeat(64),
            report_type: RadrootsReportType::Spam,
            event: Some(RadrootsSocialTarget::Event {
                id: "b".repeat(64),
                author: Some("a".repeat(64)),
                event_kind: Some(1),
                relays: None,
            }),
            file: None,
            content: Some("repeated spam".to_string()),
        };

        assert_eq!(report.reported_pubkey.len(), 64);
        assert_eq!(report.report_type, RadrootsReportType::Spam);
        assert!(matches!(
            report.event,
            Some(RadrootsSocialTarget::Event { .. })
        ));
    }

    #[test]
    fn report_model_supports_file_targets_with_required_pubkey() {
        let report = RadrootsReport {
            reported_pubkey: "a".repeat(64),
            report_type: RadrootsReportType::Malware,
            event: None,
            file: Some(RadrootsReportFileTarget {
                sha256: Some("b".repeat(64)),
                url: Some("https://example.test/file".to_string()),
                magnet: None,
            }),
            content: None,
        };

        assert_eq!(report.reported_pubkey.len(), 64);
        assert_eq!(report.file.expect("file").sha256.expect("hash").len(), 64);
    }
}
