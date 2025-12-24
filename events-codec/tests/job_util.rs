use radroots_events::job::{JobFeedbackStatus, JobInputType};
use radroots_events_codec::job::error::JobParseError;
use radroots_events_codec::job::util::{
    feedback_status_from_tag, feedback_status_tag, job_input_type_from_tag, job_input_type_tag,
    parse_amount_tag_sat, parse_bid_tag_sat, parse_bool_encrypted, parse_i_tags, parse_params,
    push_amount_tag_msat, push_bid_tag_sat,
};

#[test]
fn parse_bool_encrypted_detects_tag() {
    let tags = vec![vec!["encrypted".to_string()]];
    assert!(parse_bool_encrypted(&tags));
    assert!(!parse_bool_encrypted(&[]));
}

#[test]
fn input_type_tag_roundtrip() {
    let t = job_input_type_tag(JobInputType::Url);
    assert_eq!(job_input_type_from_tag(t), Some(JobInputType::Url));
    assert_eq!(job_input_type_from_tag("unknown"), None);
}

#[test]
fn feedback_status_tag_roundtrip() {
    let t = feedback_status_tag(JobFeedbackStatus::Processing);
    assert_eq!(feedback_status_from_tag(t), Some(JobFeedbackStatus::Processing));
    assert_eq!(feedback_status_from_tag("unknown"), None);
}

#[test]
fn parse_i_tags_handles_multiple_shapes() {
    let tags = vec![
        vec!["i".to_string(), "https://example.com".to_string()],
        vec!["i".to_string(), "note1abcdef".to_string()],
        vec![
            "i".to_string(),
            "job-id".to_string(),
            "job".to_string(),
            "wss://relay".to_string(),
            "marker".to_string(),
        ],
    ];

    let inputs = parse_i_tags(&tags);
    assert_eq!(inputs.len(), 3);

    assert_eq!(inputs[0].data, "https://example.com");
    assert_eq!(inputs[0].input_type, JobInputType::Url);
    assert!(inputs[0].relay.is_none());
    assert!(inputs[0].marker.is_none());

    assert_eq!(inputs[1].data, "note1abcdef");
    assert_eq!(inputs[1].input_type, JobInputType::Event);

    assert_eq!(inputs[2].data, "job-id");
    assert_eq!(inputs[2].input_type, JobInputType::Job);
    assert_eq!(inputs[2].relay.as_deref(), Some("wss://relay"));
    assert_eq!(inputs[2].marker.as_deref(), Some("marker"));
}

#[test]
fn parse_params_extracts_key_value_pairs() {
    let tags = vec![
        vec!["param".to_string(), "k".to_string(), "v".to_string()],
        vec!["param".to_string(), "skip".to_string()],
    ];

    let params = parse_params(&tags);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].key, "k");
    assert_eq!(params[0].value, "v");
}

#[test]
fn parse_amount_tag_sat_accepts_msat_and_bolt11() {
    let tags = vec![vec![
        "amount".to_string(),
        "1000".to_string(),
        "bolt11".to_string(),
    ]];

    let parsed = parse_amount_tag_sat(&tags).unwrap().unwrap();
    assert_eq!(parsed.0, 1);
    assert_eq!(parsed.1.as_deref(), Some("bolt11"));
}

#[test]
fn parse_amount_tag_sat_rejects_non_whole_sats() {
    let tags = vec![vec!["amount".to_string(), "1500".to_string()]];
    let err = parse_amount_tag_sat(&tags).unwrap_err();
    assert!(matches!(err, JobParseError::NonWholeSats("amount")));
}

#[test]
fn parse_amount_tag_sat_rejects_overflow() {
    let overflow = ((u32::MAX as u64) + 1) * 1000;
    let tags = vec![vec!["amount".to_string(), overflow.to_string()]];
    let err = parse_amount_tag_sat(&tags).unwrap_err();
    assert!(matches!(err, JobParseError::AmountOverflow("amount")));
}

#[test]
fn push_amount_tag_msat_writes_msat() {
    let mut tags = Vec::new();
    push_amount_tag_msat(&mut tags, 12, Some("bolt".to_string()));
    assert_eq!(
        tags[0],
        vec!["amount".to_string(), "12000".to_string(), "bolt".to_string()]
    );
}

#[test]
fn parse_bid_tag_sat_accepts_sat() {
    let tags = vec![vec!["bid".to_string(), "2".to_string()]];
    let bid = parse_bid_tag_sat(&tags).unwrap().unwrap();
    assert_eq!(bid, 2);
}

#[test]
fn parse_bid_tag_sat_rejects_non_numeric() {
    let tags = vec![vec!["bid".to_string(), "not-a-number".to_string()]];
    let err = parse_bid_tag_sat(&tags).unwrap_err();
    assert!(matches!(err, JobParseError::InvalidNumber("bid", _)));
}

#[test]
fn parse_bid_tag_sat_rejects_overflow() {
    let overflow = (u32::MAX as u64) + 1;
    let tags = vec![vec!["bid".to_string(), overflow.to_string()]];
    let err = parse_bid_tag_sat(&tags).unwrap_err();
    assert!(matches!(err, JobParseError::AmountOverflow("bid")));
}

#[test]
fn push_bid_tag_sat_writes_sat() {
    let mut tags = Vec::new();
    push_bid_tag_sat(&mut tags, 7);
    assert_eq!(tags[0], vec!["bid".to_string(), "7".to_string()]);
}
