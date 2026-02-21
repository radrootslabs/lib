#![cfg(feature = "serde_json")]

use radroots_core::{RadrootsCoreDecimal, RadrootsCoreQuantity, RadrootsCoreUnit};
use radroots_events::coop::{RadrootsCoop, RadrootsCoopEventIndex, RadrootsCoopEventMetadata};
use radroots_events::document::{
    RadrootsDocument, RadrootsDocumentEventIndex, RadrootsDocumentEventMetadata,
    RadrootsDocumentSubject,
};
use radroots_events::farm::{
    RadrootsFarm, RadrootsFarmEventIndex, RadrootsFarmEventMetadata, RadrootsFarmRef,
    RadrootsGcsLocation, RadrootsGeoJsonPoint, RadrootsGeoJsonPolygon,
};
use radroots_events::kinds::{
    KIND_COOP, KIND_DOCUMENT, KIND_FARM, KIND_PLOT, KIND_RESOURCE_AREA, KIND_RESOURCE_HARVEST_CAP,
};
use radroots_events::plot::{RadrootsPlot, RadrootsPlotEventIndex, RadrootsPlotEventMetadata};
use radroots_events::resource_area::{
    RadrootsResourceArea, RadrootsResourceAreaEventIndex, RadrootsResourceAreaEventMetadata,
    RadrootsResourceAreaLocation, RadrootsResourceAreaRef,
};
use radroots_events::resource_cap::{
    RadrootsResourceHarvestCap, RadrootsResourceHarvestCapEventIndex,
    RadrootsResourceHarvestCapEventMetadata, RadrootsResourceHarvestProduct,
};
use radroots_events::tags::TAG_D;
use radroots_events_codec::coop::decode::{
    coop_from_event, index_from_event as coop_index_from_event,
    metadata_from_event as coop_metadata_from_event,
};
use radroots_events_codec::document::decode::{
    document_from_event, index_from_event as document_index_from_event,
    metadata_from_event as document_metadata_from_event,
};
use radroots_events_codec::error::EventParseError;
use radroots_events_codec::farm::decode::{
    farm_from_event, index_from_event as farm_index_from_event,
    metadata_from_event as farm_metadata_from_event,
};
use radroots_events_codec::plot::decode::{
    index_from_event as plot_index_from_event, metadata_from_event as plot_metadata_from_event,
    plot_from_event,
};
use radroots_events_codec::resource_area::decode::{
    index_from_event as resource_area_index_from_event,
    metadata_from_event as resource_area_metadata_from_event, resource_area_from_event,
};
use radroots_events_codec::resource_cap::decode::{
    index_from_event as resource_cap_index_from_event,
    metadata_from_event as resource_cap_metadata_from_event, resource_harvest_cap_from_event,
};

const TEST_NPUB: &str = "npub1tr33s4tj2le2kk9yzhfphdtss26gyn8kv7savnnjhj794nqp333q8e7grr";
const TEST_PUBKEY_HEX: &str = "58e318557257f2ab58a415d21bb57082b4824cf667a1d64e72bcbc5acc018c62";

fn sample_gcs() -> RadrootsGcsLocation {
    RadrootsGcsLocation {
        lat: 37.0,
        lng: -122.0,
        geohash: "9q8yy".to_string(),
        point: RadrootsGeoJsonPoint {
            r#type: "Point".to_string(),
            coordinates: [-122.0, 37.0],
        },
        polygon: RadrootsGeoJsonPolygon {
            r#type: "Polygon".to_string(),
            coordinates: vec![vec![
                [-122.0, 37.0],
                [-122.0, 37.0001],
                [-122.0001, 37.0001],
                [-122.0, 37.0],
            ]],
        },
        accuracy: None,
        altitude: None,
        tag_0: None,
        label: None,
        area: None,
        elevation: None,
        soil: None,
        climate: None,
        gc_id: None,
        gc_name: None,
        gc_admin1_id: None,
        gc_admin1_name: None,
        gc_country_id: None,
        gc_country_name: None,
    }
}

fn sample_farm(d_tag: &str) -> RadrootsFarm {
    RadrootsFarm {
        d_tag: d_tag.to_string(),
        name: "Farm".to_string(),
        about: None,
        website: None,
        picture: None,
        banner: None,
        location: None,
        tags: None,
    }
}

fn sample_coop(d_tag: &str) -> RadrootsCoop {
    RadrootsCoop {
        d_tag: d_tag.to_string(),
        name: "Coop".to_string(),
        about: None,
        website: None,
        picture: None,
        banner: None,
        location: None,
        tags: None,
    }
}

fn sample_plot(d_tag: &str, farm_pubkey: &str, farm_d_tag: &str) -> RadrootsPlot {
    RadrootsPlot {
        d_tag: d_tag.to_string(),
        farm: RadrootsFarmRef {
            pubkey: farm_pubkey.to_string(),
            d_tag: farm_d_tag.to_string(),
        },
        name: "Plot".to_string(),
        about: None,
        location: None,
        tags: None,
    }
}

fn sample_document(
    d_tag: &str,
    subject_pubkey: &str,
    subject_address: Option<&str>,
) -> RadrootsDocument {
    RadrootsDocument {
        d_tag: d_tag.to_string(),
        doc_type: "charter".to_string(),
        title: "Charter".to_string(),
        version: "1.0.0".to_string(),
        summary: None,
        effective_at: None,
        body_markdown: None,
        subject: RadrootsDocumentSubject {
            pubkey: subject_pubkey.to_string(),
            address: subject_address.map(str::to_string),
        },
        tags: None,
    }
}

fn sample_resource_area(d_tag: &str) -> RadrootsResourceArea {
    RadrootsResourceArea {
        d_tag: d_tag.to_string(),
        name: "Area".to_string(),
        about: None,
        location: RadrootsResourceAreaLocation {
            primary: None,
            city: None,
            region: None,
            country: None,
            gcs: sample_gcs(),
        },
        tags: None,
    }
}

fn sample_resource_cap(d_tag: &str) -> RadrootsResourceHarvestCap {
    RadrootsResourceHarvestCap {
        d_tag: d_tag.to_string(),
        resource_area: RadrootsResourceAreaRef {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
        },
        product: RadrootsResourceHarvestProduct {
            key: "nutmeg".to_string(),
            category: Some("spice".to_string()),
        },
        start: 100,
        end: 200,
        cap_quantity: RadrootsCoreQuantity::new(
            RadrootsCoreDecimal::from(1000u32),
            RadrootsCoreUnit::MassG,
        ),
        display_amount: None,
        display_unit: None,
        display_label: None,
        tags: None,
    }
}

fn d_tag_tags(d_tag: &str) -> Vec<Vec<String>> {
    vec![vec![TAG_D.to_string(), d_tag.to_string()]]
}

#[test]
fn farm_decode_handles_success_fill_and_error_paths() {
    let d_tag = "AAAAAAAAAAAAAAAAAAAAAA";
    let tags = d_tag_tags(d_tag);
    let farm = sample_farm(d_tag);
    let content = serde_json::to_string(&farm).expect("farm content");
    let parsed = farm_from_event(KIND_FARM, &tags, &content).expect("farm parse");
    assert_eq!(parsed.d_tag, d_tag);

    let mut farm_missing = sample_farm("");
    let content_missing = serde_json::to_string(&farm_missing).expect("farm missing content");
    let filled = farm_from_event(KIND_FARM, &tags, &content_missing).expect("farm fill d");
    assert_eq!(filled.d_tag, d_tag);

    farm_missing.d_tag = "AAAAAAAAAAAAAAAAAAAAAQ".to_string();
    let mismatch_content = serde_json::to_string(&farm_missing).expect("farm mismatch content");
    let mismatch = farm_from_event(KIND_FARM, &tags, &mismatch_content).expect_err("mismatch");
    assert!(matches!(mismatch, EventParseError::InvalidTag("d")));

    let wrong_kind = farm_from_event(KIND_COOP, &tags, &content).expect_err("wrong kind");
    assert!(matches!(
        wrong_kind,
        EventParseError::InvalidKind {
            expected: "30340",
            got: KIND_COOP
        }
    ));

    let missing_d = farm_from_event(KIND_FARM, &[], &content).expect_err("missing d");
    assert!(matches!(missing_d, EventParseError::MissingTag("d")));

    let invalid_d = farm_from_event(
        KIND_FARM,
        &[vec![TAG_D.to_string(), "farm:invalid".to_string()]],
        &content,
    )
    .expect_err("invalid d");
    assert!(matches!(invalid_d, EventParseError::InvalidTag("d")));

    let invalid_json = farm_from_event(KIND_FARM, &tags, "").expect_err("invalid content");
    assert!(matches!(
        invalid_json,
        EventParseError::InvalidJson("content")
    ));
}

#[test]
fn farm_metadata_and_index_decode_roundtrip() {
    let d_tag = "AAAAAAAAAAAAAAAAAAAAAA";
    let content = serde_json::to_string(&sample_farm(d_tag)).expect("farm content");
    let tags = d_tag_tags(d_tag);
    let metadata: RadrootsFarmEventMetadata = farm_metadata_from_event(
        "id1".to_string(),
        TEST_PUBKEY_HEX.to_string(),
        55,
        KIND_FARM,
        content.clone(),
        tags.clone(),
    )
    .expect("farm metadata");
    assert_eq!(metadata.id, "id1");
    assert_eq!(metadata.farm.d_tag, d_tag);

    let index: RadrootsFarmEventIndex = farm_index_from_event(
        "id1".to_string(),
        TEST_PUBKEY_HEX.to_string(),
        55,
        KIND_FARM,
        content,
        tags,
        "sig1".to_string(),
    )
    .expect("farm index");
    assert_eq!(index.event.id, "id1");
    assert_eq!(index.metadata.farm.d_tag, d_tag);
}

#[test]
fn coop_decode_handles_success_fill_and_error_paths() {
    let d_tag = "BAAAAAAAAAAAAAAAAAAAAA";
    let tags = d_tag_tags(d_tag);
    let coop = sample_coop(d_tag);
    let content = serde_json::to_string(&coop).expect("coop content");
    let parsed = coop_from_event(KIND_COOP, &tags, &content).expect("coop parse");
    assert_eq!(parsed.d_tag, d_tag);

    let content_missing = serde_json::to_string(&sample_coop("")).expect("coop missing content");
    let filled = coop_from_event(KIND_COOP, &tags, &content_missing).expect("coop fill d");
    assert_eq!(filled.d_tag, d_tag);

    let mismatch_content =
        serde_json::to_string(&sample_coop("AAAAAAAAAAAAAAAAAAAAAQ")).expect("coop mismatch");
    let mismatch = coop_from_event(KIND_COOP, &tags, &mismatch_content).expect_err("mismatch");
    assert!(matches!(mismatch, EventParseError::InvalidTag("d")));

    let wrong_kind = coop_from_event(KIND_FARM, &tags, &content).expect_err("wrong kind");
    assert!(matches!(
        wrong_kind,
        EventParseError::InvalidKind {
            expected: "30360",
            got: KIND_FARM
        }
    ));

    let missing_d = coop_from_event(KIND_COOP, &[], &content).expect_err("missing d");
    assert!(matches!(missing_d, EventParseError::MissingTag("d")));
}

#[test]
fn coop_metadata_and_index_decode_roundtrip() {
    let d_tag = "BAAAAAAAAAAAAAAAAAAAAA";
    let content = serde_json::to_string(&sample_coop(d_tag)).expect("coop content");
    let tags = d_tag_tags(d_tag);
    let metadata: RadrootsCoopEventMetadata = coop_metadata_from_event(
        "id2".to_string(),
        TEST_PUBKEY_HEX.to_string(),
        56,
        KIND_COOP,
        content.clone(),
        tags.clone(),
    )
    .expect("coop metadata");
    assert_eq!(metadata.id, "id2");
    assert_eq!(metadata.coop.d_tag, d_tag);

    let index: RadrootsCoopEventIndex = coop_index_from_event(
        "id2".to_string(),
        TEST_PUBKEY_HEX.to_string(),
        56,
        KIND_COOP,
        content,
        tags,
        "sig2".to_string(),
    )
    .expect("coop index");
    assert_eq!(index.event.kind, KIND_COOP);
    assert_eq!(index.metadata.coop.d_tag, d_tag);
}

#[test]
fn plot_decode_handles_success_fill_and_tag_error_paths() {
    let d_tag = "AAAAAAAAAAAAAAAAAAAAAQ";
    let farm_d_tag = "AAAAAAAAAAAAAAAAAAAAAA";
    let tags = vec![
        vec![TAG_D.to_string(), d_tag.to_string()],
        vec![
            "a".to_string(),
            format!("30340:{TEST_PUBKEY_HEX}:{farm_d_tag}"),
        ],
        vec!["p".to_string(), TEST_PUBKEY_HEX.to_string()],
    ];

    let content = serde_json::to_string(&sample_plot(d_tag, TEST_PUBKEY_HEX, farm_d_tag))
        .expect("plot content");
    let parsed = plot_from_event(KIND_PLOT, &tags, &content).expect("plot parse");
    assert_eq!(parsed.farm.pubkey, TEST_PUBKEY_HEX);

    let filled_content =
        serde_json::to_string(&sample_plot("", "", "")).expect("plot missing content");
    let filled = plot_from_event(KIND_PLOT, &tags, &filled_content).expect("plot fill");
    assert_eq!(filled.d_tag, d_tag);
    assert_eq!(filled.farm.d_tag, farm_d_tag);

    let bad_a = plot_from_event(
        KIND_PLOT,
        &[
            vec![TAG_D.to_string(), d_tag.to_string()],
            vec![
                "a".to_string(),
                format!("30361:{TEST_PUBKEY_HEX}:AAAAAAAAAAAAAAAAAAAAAA"),
            ],
            vec!["p".to_string(), TEST_PUBKEY_HEX.to_string()],
        ],
        &content,
    )
    .expect_err("bad a");
    assert!(matches!(bad_a, EventParseError::InvalidTag("a")));

    let bad_p = plot_from_event(
        KIND_PLOT,
        &[
            vec![TAG_D.to_string(), d_tag.to_string()],
            vec![
                "a".to_string(),
                format!("30340:{TEST_PUBKEY_HEX}:{farm_d_tag}"),
            ],
            vec!["p".to_string(), TEST_NPUB.to_string()],
        ],
        &content,
    )
    .expect_err("bad p");
    assert!(matches!(bad_p, EventParseError::InvalidTag("p")));

    let missing_a = plot_from_event(
        KIND_PLOT,
        &[
            vec![TAG_D.to_string(), d_tag.to_string()],
            vec!["p".to_string(), TEST_PUBKEY_HEX.to_string()],
        ],
        &content,
    )
    .expect_err("missing a");
    assert!(matches!(missing_a, EventParseError::MissingTag("a")));
}

#[test]
fn plot_metadata_and_index_decode_roundtrip() {
    let d_tag = "AAAAAAAAAAAAAAAAAAAAAQ";
    let farm_d_tag = "AAAAAAAAAAAAAAAAAAAAAA";
    let tags = vec![
        vec![TAG_D.to_string(), d_tag.to_string()],
        vec![
            "a".to_string(),
            format!("30340:{TEST_PUBKEY_HEX}:{farm_d_tag}"),
        ],
        vec!["p".to_string(), TEST_PUBKEY_HEX.to_string()],
    ];
    let content = serde_json::to_string(&sample_plot(d_tag, TEST_PUBKEY_HEX, farm_d_tag))
        .expect("plot content");

    let metadata: RadrootsPlotEventMetadata = plot_metadata_from_event(
        "id3".to_string(),
        TEST_PUBKEY_HEX.to_string(),
        57,
        KIND_PLOT,
        content.clone(),
        tags.clone(),
    )
    .expect("plot metadata");
    assert_eq!(metadata.plot.d_tag, d_tag);

    let index: RadrootsPlotEventIndex = plot_index_from_event(
        "id3".to_string(),
        TEST_PUBKEY_HEX.to_string(),
        57,
        KIND_PLOT,
        content,
        tags,
        "sig3".to_string(),
    )
    .expect("plot index");
    assert_eq!(index.event.author, TEST_PUBKEY_HEX);
    assert_eq!(index.metadata.plot.d_tag, d_tag);
}

#[test]
fn document_decode_handles_subject_and_address_paths() {
    let d_tag = "EAAAAAAAAAAAAAAAAAAAAA";
    let tag_address = format!("30360:{TEST_PUBKEY_HEX}:BAAAAAAAAAAAAAAAAAAAAA");
    let tags = vec![
        vec![TAG_D.to_string(), d_tag.to_string()],
        vec!["p".to_string(), TEST_PUBKEY_HEX.to_string()],
        vec!["a".to_string(), tag_address.clone()],
    ];
    let content =
        serde_json::to_string(&sample_document(d_tag, TEST_PUBKEY_HEX, Some(&tag_address)))
            .expect("document content");
    let parsed = document_from_event(KIND_DOCUMENT, &tags, &content).expect("document parse");
    assert_eq!(parsed.subject.pubkey, TEST_PUBKEY_HEX);
    assert_eq!(
        parsed.subject.address.as_deref(),
        Some(tag_address.as_str())
    );

    let fill_content = serde_json::to_string(&sample_document("", "", None)).expect("fill");
    let filled = document_from_event(KIND_DOCUMENT, &tags, &fill_content).expect("document fill");
    assert_eq!(filled.d_tag, d_tag);
    assert_eq!(filled.subject.pubkey, TEST_PUBKEY_HEX);
    assert_eq!(
        filled.subject.address.as_deref(),
        Some(tag_address.as_str())
    );

    let missing_a_err = document_from_event(
        KIND_DOCUMENT,
        &[
            vec![TAG_D.to_string(), d_tag.to_string()],
            vec!["p".to_string(), TEST_PUBKEY_HEX.to_string()],
        ],
        &content,
    )
    .expect_err("missing a");
    assert!(matches!(missing_a_err, EventParseError::MissingTag("a")));

    let mismatch_p_content =
        serde_json::to_string(&sample_document(d_tag, TEST_NPUB, Some(&tag_address)))
            .expect("mismatch p content");
    let mismatch_p =
        document_from_event(KIND_DOCUMENT, &tags, &mismatch_p_content).expect_err("mismatch p");
    assert!(matches!(mismatch_p, EventParseError::InvalidTag("p")));

    let empty_a_content =
        serde_json::to_string(&sample_document(d_tag, TEST_PUBKEY_HEX, Some(""))).expect("empty a");
    let empty_a =
        document_from_event(KIND_DOCUMENT, &tags, &empty_a_content).expect_err("empty address");
    assert!(matches!(empty_a, EventParseError::InvalidTag("a")));
}

#[test]
fn document_metadata_and_index_decode_roundtrip() {
    let d_tag = "EAAAAAAAAAAAAAAAAAAAAA";
    let tag_address = format!("30360:{TEST_PUBKEY_HEX}:BAAAAAAAAAAAAAAAAAAAAA");
    let tags = vec![
        vec![TAG_D.to_string(), d_tag.to_string()],
        vec!["p".to_string(), TEST_PUBKEY_HEX.to_string()],
        vec!["a".to_string(), tag_address.clone()],
    ];
    let content =
        serde_json::to_string(&sample_document(d_tag, TEST_PUBKEY_HEX, Some(&tag_address)))
            .expect("document content");

    let metadata: RadrootsDocumentEventMetadata = document_metadata_from_event(
        "id4".to_string(),
        TEST_PUBKEY_HEX.to_string(),
        58,
        KIND_DOCUMENT,
        content.clone(),
        tags.clone(),
    )
    .expect("document metadata");
    assert_eq!(metadata.document.d_tag, d_tag);

    let index: RadrootsDocumentEventIndex = document_index_from_event(
        "id4".to_string(),
        TEST_PUBKEY_HEX.to_string(),
        58,
        KIND_DOCUMENT,
        content,
        tags,
        "sig4".to_string(),
    )
    .expect("document index");
    assert_eq!(index.event.kind, KIND_DOCUMENT);
    assert_eq!(index.metadata.document.d_tag, d_tag);
}

#[test]
fn resource_area_decode_handles_success_fill_and_errors() {
    let d_tag = "AAAAAAAAAAAAAAAAAAAAAw";
    let tags = d_tag_tags(d_tag);
    let area = sample_resource_area(d_tag);
    let content = serde_json::to_string(&area).expect("area content");
    let parsed = resource_area_from_event(KIND_RESOURCE_AREA, &tags, &content).expect("area");
    assert_eq!(parsed.d_tag, d_tag);

    let fill_content = serde_json::to_string(&sample_resource_area("")).expect("area fill");
    let filled =
        resource_area_from_event(KIND_RESOURCE_AREA, &tags, &fill_content).expect("area fill");
    assert_eq!(filled.d_tag, d_tag);

    let mismatch_content =
        serde_json::to_string(&sample_resource_area("AAAAAAAAAAAAAAAAAAAAAQ")).expect("mismatch");
    let mismatch =
        resource_area_from_event(KIND_RESOURCE_AREA, &tags, &mismatch_content).expect_err("m");
    assert!(matches!(mismatch, EventParseError::InvalidTag("d")));

    let wrong_kind = resource_area_from_event(KIND_FARM, &tags, &content).expect_err("wrong kind");
    assert!(matches!(
        wrong_kind,
        EventParseError::InvalidKind {
            expected: "30370",
            got: KIND_FARM
        }
    ));
}

#[test]
fn resource_area_metadata_and_index_decode_roundtrip() {
    let d_tag = "AAAAAAAAAAAAAAAAAAAAAw";
    let content = serde_json::to_string(&sample_resource_area(d_tag)).expect("area content");
    let tags = d_tag_tags(d_tag);
    let metadata: RadrootsResourceAreaEventMetadata = resource_area_metadata_from_event(
        "id5".to_string(),
        TEST_PUBKEY_HEX.to_string(),
        59,
        KIND_RESOURCE_AREA,
        content.clone(),
        tags.clone(),
    )
    .expect("area metadata");
    assert_eq!(metadata.area.d_tag, d_tag);

    let index: RadrootsResourceAreaEventIndex = resource_area_index_from_event(
        "id5".to_string(),
        TEST_PUBKEY_HEX.to_string(),
        59,
        KIND_RESOURCE_AREA,
        content,
        tags,
        "sig5".to_string(),
    )
    .expect("area index");
    assert_eq!(index.event.id, "id5");
    assert_eq!(index.metadata.area.d_tag, d_tag);
}

#[test]
fn resource_cap_decode_handles_success_fill_and_errors() {
    let d_tag = "DAAAAAAAAAAAAAAAAAAAAA";
    let tags = d_tag_tags(d_tag);
    let cap = sample_resource_cap(d_tag);
    let content = serde_json::to_string(&cap).expect("cap content");
    let parsed = resource_harvest_cap_from_event(KIND_RESOURCE_HARVEST_CAP, &tags, &content)
        .expect("cap parse");
    assert_eq!(parsed.d_tag, d_tag);

    let fill_content = serde_json::to_string(&sample_resource_cap("")).expect("cap fill");
    let filled = resource_harvest_cap_from_event(KIND_RESOURCE_HARVEST_CAP, &tags, &fill_content)
        .expect("cap fill parse");
    assert_eq!(filled.d_tag, d_tag);

    let mismatch_content =
        serde_json::to_string(&sample_resource_cap("AAAAAAAAAAAAAAAAAAAAAQ")).expect("mismatch");
    let mismatch =
        resource_harvest_cap_from_event(KIND_RESOURCE_HARVEST_CAP, &tags, &mismatch_content)
            .expect_err("cap mismatch");
    assert!(matches!(mismatch, EventParseError::InvalidTag("d")));
}

#[test]
fn resource_cap_metadata_and_index_decode_roundtrip() {
    let d_tag = "DAAAAAAAAAAAAAAAAAAAAA";
    let content = serde_json::to_string(&sample_resource_cap(d_tag)).expect("cap content");
    let tags = d_tag_tags(d_tag);
    let metadata: RadrootsResourceHarvestCapEventMetadata = resource_cap_metadata_from_event(
        "id6".to_string(),
        TEST_PUBKEY_HEX.to_string(),
        60,
        KIND_RESOURCE_HARVEST_CAP,
        content.clone(),
        tags.clone(),
    )
    .expect("cap metadata");
    assert_eq!(metadata.cap.d_tag, d_tag);

    let index: RadrootsResourceHarvestCapEventIndex = resource_cap_index_from_event(
        "id6".to_string(),
        TEST_PUBKEY_HEX.to_string(),
        60,
        KIND_RESOURCE_HARVEST_CAP,
        content,
        tags,
        "sig6".to_string(),
    )
    .expect("cap index");
    assert_eq!(index.event.sig, "sig6");
    assert_eq!(index.metadata.cap.d_tag, d_tag);
}
