#![cfg(feature = "json")]

use std::{borrow::Cow, fs, path::Path};

use radroots_event::GenericEventDraft;
use radroots_event_codec::authoring::{AuthoredEventPlan, PlanWireV1};
use serde::Deserialize;
use serde_json::Value;

const PACKAGED_VECTORS: &str = include_str!("fixtures/authored_plan_wire.v1.json");
const WORKSPACE_VECTOR_PATH: &str =
    "../../contracts/conformance/vectors/event/authored_plan_wire.v1.json";
const WORKSPACE_CONTRACT_MARKER_PATH: &str = "../../contracts/manifest.toml";

#[derive(Debug, Deserialize)]
struct Suite {
    suite: String,
    contract_version: String,
    vectors: Vec<Vector>,
}

#[derive(Debug, Deserialize)]
struct Vector {
    id: String,
    kind: String,
    input: Input,
    expected: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    contract_id: String,
    expected_author: String,
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
}

#[test]
fn checked_in_plan_wire_vector_executes_and_is_package_self_contained() {
    let vectors = conformance_vectors();
    let suite: Suite = serde_json::from_str(&vectors).expect("plan wire vectors must parse");
    assert_eq!(suite.suite, "authored_plan_wire");
    assert_eq!(suite.contract_version, "1.0.0");
    assert!(!suite.vectors.is_empty());

    for vector in suite.vectors {
        assert_eq!(vector.kind, "authored_plan_wire.v1", "{}", vector.id);
        let draft = GenericEventDraft::new(
            vector.input.contract_id,
            vector.input.kind,
            vector.input.created_at,
            vector.input.tags,
            vector.input.content,
            vector.input.expected_author,
        )
        .unwrap_or_else(|error| panic!("{} draft failed: {error}", vector.id));
        let plan = AuthoredEventPlan::from_generic(draft)
            .unwrap_or_else(|error| panic!("{} plan failed: {error}", vector.id));
        let encoded = PlanWireV1::from_plan(&plan)
            .to_json()
            .unwrap_or_else(|error| panic!("{} encode failed: {error}", vector.id));
        assert_eq!(
            serde_json::from_slice::<Value>(&encoded).expect("encoded value"),
            vector.expected,
            "{}",
            vector.id
        );
        assert_eq!(
            PlanWireV1::from_json(&encoded)
                .unwrap_or_else(|error| panic!("{} decode failed: {error}", vector.id))
                .into_plan(),
            plan,
            "{}",
            vector.id
        );
    }
}

fn conformance_vectors() -> Cow<'static, str> {
    let workspace_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(WORKSPACE_VECTOR_PATH);
    match fs::read_to_string(&workspace_path) {
        Ok(canonical) => {
            assert_eq!(canonical, PACKAGED_VECTORS);
            Cow::Owned(canonical)
        }
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && !Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(WORKSPACE_CONTRACT_MARKER_PATH)
                    .is_file() =>
        {
            Cow::Borrowed(PACKAGED_VECTORS)
        }
        Err(error) => panic!("failed to read {}: {error}", workspace_path.display()),
    }
}
