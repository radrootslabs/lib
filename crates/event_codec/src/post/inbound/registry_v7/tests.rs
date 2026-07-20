use super::*;
use radroots_event::post::RadrootsAuthoredUpdate;

#[test]
fn thread_exclusion_precedes_ask_and_media_projection() {
    let projection = project_inbound_post_parts(
        KIND_POST,
        &[
            vec!["e".to_string(), "parent".to_string()],
            vec!["t".to_string(), "radroots-ask".to_string()],
            vec!["imeta".to_string(), "x malformed".to_string()],
        ],
        "reply",
    )
    .unwrap();

    assert_eq!(
        projection.classification(),
        RadrootsPostClassification::ThreadExcluded
    );
    assert!(projection.imeta().is_empty());
    assert!(projection.diagnostics().is_empty());
}

#[test]
fn every_empty_or_malformed_event_reference_is_thread_excluded() {
    for event_reference in [
        vec!["e".to_string()],
        vec!["e".to_string(), String::new()],
        vec!["e".to_string(), "not-an-event-id".to_string()],
    ] {
        let projection =
            project_inbound_post_parts(KIND_POST, &[event_reference], "candidate").unwrap();

        assert_eq!(
            projection.classification(),
            RadrootsPostClassification::ThreadExcluded
        );
        assert!(!projection.classification().is_root_card());
    }
}

#[test]
fn normalized_ask_precedes_malformed_media_and_retains_diagnostics() {
    let projection = project_inbound_post_parts(
        KIND_POST,
        &[
            vec!["t".to_string(), " RADROOTS-ASK ".to_string()],
            vec![
                "imeta".to_string(),
                "url https://cdn.example/leaf.webp".to_string(),
                "x malformed".to_string(),
            ],
        ],
        "Question https://cdn.example/leaf.webp",
    )
    .unwrap();

    assert_eq!(projection.classification(), RadrootsPostClassification::Ask);
    assert_eq!(
        diagnostic_codes(projection.diagnostics()),
        ["imeta_metadata_missing", "imeta_hash_invalid"]
    );
    assert_eq!(
        projection.ask_marker().unwrap(),
        ["t".to_string(), " RADROOTS-ASK ".to_string()]
    );
}

#[test]
fn photo_preserves_repeatable_fallbacks_and_ordered_unknown_fields() {
    let projection = project_inbound_post_parts(
        KIND_POST,
        &[qualifying_imeta(vec![
            "fallback https://cache-one.example/harvest.webp",
            "x-farm cultivar-strawberry",
            "fallback https://cache-two.example/harvest.webp",
            "future-field retained value",
        ])],
        "Harvest https://cdn.example/harvest.webp",
    )
    .unwrap();
    let media = &projection.imeta()[0];

    assert_eq!(
        projection.classification(),
        RadrootsPostClassification::PhotoUpdate
    );
    assert_eq!(
        media.fallbacks(),
        [
            "https://cache-one.example/harvest.webp".to_string(),
            "https://cache-two.example/harvest.webp".to_string(),
        ]
    );
    assert_eq!(
        media.unknown_fields(),
        [
            "x-farm cultivar-strawberry".to_string(),
            "future-field retained value".to_string(),
        ]
    );
    assert!(media.qualifies_photo());
}

#[test]
fn duplicate_singletons_and_mixed_imeta_downgrade_to_update() {
    let mut duplicate = qualifying_imeta(Vec::new());
    duplicate.insert(
        3,
        "x bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
    );
    let malformed = vec![
        "imeta".to_string(),
        "url https://cdn.example/leaf.webp".to_string(),
        "x malformed".to_string(),
    ];
    let projection = project_inbound_post_parts(
        KIND_POST,
        &[duplicate, malformed],
        "Harvest https://cdn.example/harvest.webp and https://cdn.example/leaf.webp",
    )
    .unwrap();

    assert_eq!(
        projection.classification(),
        RadrootsPostClassification::Update
    );
    assert_eq!(
        diagnostic_codes(projection.diagnostics()),
        [
            "imeta_singleton_duplicate",
            "imeta_metadata_missing",
            "imeta_hash_invalid",
        ]
    );
}

#[test]
fn duplicate_urls_and_excess_imeta_downgrade_to_update() {
    let duplicate = qualifying_imeta(Vec::new());
    let projection = project_inbound_post_parts(
        KIND_POST,
        &[duplicate.clone(), duplicate],
        "Harvest https://cdn.example/harvest.webp",
    )
    .unwrap();
    assert_eq!(
        projection.classification(),
        RadrootsPostClassification::Update
    );
    assert_eq!(
        diagnostic_codes(projection.diagnostics()),
        ["duplicate_imeta_url"]
    );

    let imeta = qualifying_imeta(Vec::new());
    let tags = vec![imeta; RADROOTS_POST_IMETA_MAX_COUNT + 1];
    let projection =
        project_inbound_post_parts(KIND_POST, &tags, "Harvest https://cdn.example/harvest.webp")
            .unwrap();
    assert_eq!(
        projection.classification(),
        RadrootsPostClassification::Update
    );
    assert_eq!(
        projection.diagnostics().first(),
        Some(&RadrootsPostDiagnostic::ImetaCountExceeded)
    );
}

#[test]
fn invalid_imeta_fields_report_stable_ordered_diagnostics() {
    let projection = project_inbound_post_parts(
        KIND_POST,
        &[vec![
            "imeta".to_string(),
            "url ftp://cdn.example/harvest.webp".to_string(),
            "x AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
            "m image/webp;quality=90".to_string(),
            "dim 01x0".to_string(),
            "size 0".to_string(),
            "alt \t".to_string(),
            "fallback file:///harvest.webp".to_string(),
        ]],
        "Harvest ftp://cdn.example/harvest.webp",
    )
    .unwrap();

    assert_eq!(
        projection.classification(),
        RadrootsPostClassification::Update
    );
    assert_eq!(
        diagnostic_codes(projection.diagnostics()),
        [
            "imeta_url_invalid",
            "imeta_hash_invalid",
            "imeta_mime_invalid",
            "imeta_dimensions_invalid",
            "imeta_size_invalid",
            "imeta_alt_invalid",
            "imeta_fallback_url_invalid",
        ]
    );
}

#[test]
fn malformed_fields_and_oversized_alt_never_qualify_photo() {
    let oversized_alt = "a".repeat(RADROOTS_POST_ALT_MAX_BYTES + 1);
    let mut tag = qualifying_imeta(Vec::new());
    tag.push("malformed".to_string());
    tag[6] = format!("alt {oversized_alt}");
    let projection = project_inbound_post_parts(
        KIND_POST,
        &[tag],
        "Harvest https://cdn.example/harvest.webp",
    )
    .unwrap();

    assert_eq!(
        projection.classification(),
        RadrootsPostClassification::Update
    );
    assert_eq!(
        diagnostic_codes(projection.diagnostics()),
        ["imeta_field_invalid", "imeta_alt_too_large"]
    );
}

#[test]
fn malformed_and_duplicate_normalized_ask_markers_are_distinct() {
    let malformed = project_inbound_post_parts(
        KIND_POST,
        &[vec![
            "t".to_string(),
            "RADROOTS-ASK".to_string(),
            "extra".to_string(),
        ]],
        "Question",
    )
    .unwrap();
    assert_eq!(
        malformed.classification(),
        RadrootsPostClassification::Update
    );
    assert_eq!(
        diagnostic_codes(malformed.diagnostics()),
        ["ask_marker_shape"]
    );

    let error = project_inbound_post_parts(
        KIND_POST,
        &[
            vec!["t".to_string(), "radroots-ask".to_string()],
            vec!["t".to_string(), " RADROOTS-ASK ".to_string()],
        ],
        "Question",
    )
    .unwrap_err();
    assert_eq!(error.code(), "ask_marker_count");
}

#[test]
fn empty_inbound_root_is_update_without_becoming_valid_authored_content() {
    let projection = project_inbound_post_parts(KIND_POST, &[], "\t").unwrap();
    assert_eq!(
        projection.classification(),
        RadrootsPostClassification::Update
    );
    assert!(RadrootsAuthoredUpdate::new("\t").is_err());
}

#[test]
fn projection_rejects_wrong_kind_and_oversized_content() {
    assert_eq!(
        project_inbound_post_parts(20, &[], "photo")
            .unwrap_err()
            .code(),
        "invalid_kind"
    );
    let oversized = "x".repeat(RADROOTS_POST_CONTENT_MAX_BYTES + 1);
    assert_eq!(
        project_inbound_post_parts(KIND_POST, &[], &oversized)
            .unwrap_err()
            .code(),
        "post_content_too_large"
    );
}

fn qualifying_imeta(extra: Vec<&str>) -> Vec<String> {
    let mut tag = vec![
        "imeta".to_string(),
        "url https://cdn.example/harvest.webp".to_string(),
        "x aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        "m image/webp".to_string(),
        "dim 1200x900".to_string(),
        "size 12345".to_string(),
        "alt Harvest".to_string(),
    ];
    tag.extend(extra.into_iter().map(str::to_string));
    tag
}

fn diagnostic_codes(diagnostics: &[RadrootsPostDiagnostic]) -> Vec<&'static str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code())
        .collect()
}
