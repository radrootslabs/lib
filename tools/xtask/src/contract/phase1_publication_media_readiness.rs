use super::artifact_bundle::{
    GeneratedArtifact, read_regular_file, with_artifact_bundle_transaction,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};
use syn::{Expr, ImplItem, Item, Visibility, punctuated::Punctuated, token::Comma, visit::Visit};

const SCHEMA_VERSION: u32 = 1;
const CONTRACT_ID: &str = "radroots_event_codec.phase1_publication_media_readiness_v1";
const AUTHORITY_ID: &str = "phase1_publication_media_readiness_v1";
const HASH_ALGORITHM: &str = "sha256_bytes_v1";
const WRITE_COMMAND: &str =
    "cargo xtask contract phase1-publication-media-readiness-manifest --write";

const MANIFEST_RELATIVE: &str =
    "crates/event_codec/contracts/phase1_publication_media_readiness_v1.manifest.json";
const MANIFEST_SCHEMA_RELATIVE: &str =
    "crates/event_codec/contracts/phase1_publication_media_readiness_v1.manifest.schema.json";
const MANIFEST_SHA256_RELATIVE: &str =
    "crates/event_codec/contracts/phase1_publication_media_readiness_v1.manifest.sha256";
const GENERATED_DESCRIPTOR_RELATIVE: &str =
    "crates/event_codec/contracts/phase1_publication_media_readiness_v1.descriptor.json";
const BINDING_SCHEMA_RELATIVE: &str =
    "crates/event_codec/contracts/phase1_publication_media_readiness_binding_v1.schema.json";
const VECTOR_RELATIVE: &str =
    "contracts/conformance/vectors/publication/phase1_media_readiness.v1.json";
const VECTOR_MIRROR_RELATIVE: &str =
    "crates/event_codec/tests/fixtures/phase1_publication_media_readiness.v1.json";
const VECTOR_EXECUTOR_RELATIVE: &str = "crates/event_codec/tests/publication_media_readiness.rs";
const VECTOR_EXECUTOR_TEST: &str = "phase1_publication_media_readiness_vector_executes_every_case";
const SOURCE_RELATIVE: &str = "crates/event_codec/src/wire/publication/media_readiness.rs";
const PUBLICATION_SOURCE_RELATIVE: &str = "crates/event_codec/src/wire/publication.rs";
const EVENT_CODEC_MANIFEST_RELATIVE: &str = "crates/event_codec/Cargo.toml";
const OPERATIONS_RELATIVE: &str = "contracts/operations.toml";
const RELEASE_RELATIVE: &str = "contracts/releases/1.0.0-alpha.1.toml";
const CHANGELOG_RELATIVE: &str = "CHANGELOG.md";
const RELEASE_CHANGE_ID: &str = "phase1-publication-media-readiness";
const CHANGELOG_MARKER: &str = "<!-- release-change: phase1-publication-media-readiness -->";

const BINDING_SCHEMA_VERSION: u32 = 1;
const READINESS_POLICY_VERSION: u16 = 1;
const BINDING_MAX_BYTES: u64 = 4 * 1024 * 1024;
const EVIDENCE_MAX_COUNT: u32 = 4096;
const EVIDENCE_MAX_BYTES: u32 = 8192;
const URL_MAX_BYTES: u32 = 4096;
const RASTER_MAX_BYTES: u64 = 10_485_760;
const DIGEST_DOMAIN: &str = "radroots.phase1.publication-media-readiness.v1\0";

const BIND_OPERATION_ID: &str = "publication_media_readiness.bind";
const SERIALIZE_OPERATION_ID: &str = "publication_media_readiness.to_canonical_json";
const RELOAD_OPERATION_ID: &str = "publication_media_readiness.from_canonical_json";
const VALIDATE_OPERATION_ID: &str = "publication_media_readiness.validate";

const PUBLIC_CONSTANTS: &[&str] = &[
    "RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_MAX_BYTES",
    "RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_SCHEMA_VERSION",
];
const PUBLIC_TYPES: &[&str] = &[
    "RadrootsPhase1MediaReadyPublicationArtifact",
    "RadrootsPhase1PublicationMediaReadinessBindingDigest",
    "RadrootsPhase1PublicationMediaReadinessError",
];
const PUBLIC_FUNCTIONS: &[&str] = &[
    "bind_phase1_publication_media_readiness",
    "validate_phase1_publication_media_readiness",
];
const PUBLIC_METHODS: &[&str] = &[
    "RadrootsPhase1MediaReadyPublicationArtifact::allowlisted_artifact",
    "RadrootsPhase1MediaReadyPublicationArtifact::artifact",
    "RadrootsPhase1MediaReadyPublicationArtifact::binding_digest",
    "RadrootsPhase1MediaReadyPublicationArtifact::canonical_json",
    "RadrootsPhase1MediaReadyPublicationArtifact::evidence",
    "RadrootsPhase1MediaReadyPublicationArtifact::from_canonical_json",
    "RadrootsPhase1MediaReadyPublicationArtifact::into_allowlisted_artifact",
    "RadrootsPhase1MediaReadyPublicationArtifact::to_canonical_json",
    "RadrootsPhase1PublicationMediaReadinessBindingDigest::as_bytes",
    "RadrootsPhase1PublicationMediaReadinessBindingDigest::to_hex",
    "RadrootsPhase1PublicationMediaReadinessError::code",
];
const SEALED_TYPES: &[&str] = &[
    "RadrootsPhase1MediaReadyPublicationArtifact",
    "RadrootsPhase1PublicationMediaReadinessBindingDigest",
];
const PRIVATE_WIRE_TYPES: &[&str] = &["BindingWire"];
const WIRE_FIELD_ORDER: &[&str] = &[
    "schema_version",
    "readiness_policy_version",
    "artifact_digest",
    "evidence",
    "binding_digest",
];
const SEMANTIC_INVARIANTS: &[&str] = &[
    "allowlisted_artifact_required_v1",
    "one_evidence_per_distinct_artifact_ordered_canonical_url_v1",
    "media_free_artifact_requires_empty_evidence_v1",
    "artifact_and_binding_bytes_persisted_separately_v1",
    "closed_jpeg_png_still_webp_media_envelope_v1",
    "canonical_url_max_4096_utf8_bytes_v1",
    "nonzero_media_size_max_10485760_bytes_v1",
    "post_ask_food_authored_dimensions_equal_decoded_dimensions_v1",
    "profile_event_decoded_dimensions_persisted_without_authored_claim_v1",
    "binding_input_bounded_before_parse_v1",
    "evidence_count_max_4096_v1",
    "canonical_json_round_trip_required_v1",
    "sealed_binding_without_deserialize_v1",
    "private_bounded_deny_unknown_fields_wire_v1",
    "private_domain_separated_digest_derivation_v1",
    "no_bud11_credentials_entitlement_or_topology_persistence_v1",
];
const ERROR_CODES: &[&str] = &[
    "publication_media_readiness_binding_too_large",
    "publication_media_readiness_evidence_count_exceeded",
    "publication_media_readiness_evidence_count_mismatch",
    "publication_media_readiness_invalid_json",
    "publication_media_readiness_non_canonical_json",
    "publication_media_readiness_schema_version_unsupported",
    "publication_media_readiness_policy_version_unsupported",
    "publication_media_readiness_artifact_digest_mismatch",
    "publication_media_readiness_artifact_profile_invalid",
    "publication_media_readiness_evidence_invalid",
    "publication_media_readiness_evidence_order_mismatch",
    "publication_media_readiness_evidence_fact_mismatch",
    "publication_media_readiness_evidence_dimension_mismatch",
    "publication_media_readiness_digest_invalid",
    "publication_media_readiness_digest_mismatch",
    "publication_media_readiness_state_mismatch",
    "publication_media_readiness_allocation_failed",
    "publication_media_readiness_serialization",
];
const ARTIFACT_ERROR_CODES: &[&str] = &[
    "publication_media_inventory_mismatch",
    "publication_media_reference_invalid",
    "publication_post_profile_invalid",
];

const PUBLICATION_ARTIFACT_TRANSITIVE_PREDECESSOR_ARTIFACTS: &[ImmutableArtifactSpec] = &[
    ImmutableArtifactSpec::new(
        "crates/event_codec/contracts/phase1_publication_artifact_v1.manifest.json",
        89_464,
        "a07aace74f4747ba6e769a99acad7eadaac2d19d26aa0dd1c280ab92454519b5",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_codec/contracts/phase1_publication_artifact_v1.manifest.schema.json",
        11_972,
        "1d72cee2754e7ac45105d79b1ecf7d44251991be7a18ba106166e962000e8320",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_codec/contracts/phase1_publication_artifact_v1.manifest.sha256",
        65,
        "26acab1047ed184ff9d9b8bbac5aa0f6de35662a5375dad6f83cfa033dcfeabf",
    ),
    ImmutableArtifactSpec::new(
        "contracts/conformance/vectors/publication/phase1_artifact.v1.json",
        23_113,
        "ec18c687d5b0710a48624ddb620d89157e6b645dbea8bb91c62e3a111d20c622",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_codec/tests/fixtures/phase1_publication_artifact.v1.json",
        23_113,
        "ec18c687d5b0710a48624ddb620d89157e6b645dbea8bb91c62e3a111d20c622",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_codec/tests/publication_artifact.rs",
        34_582,
        "7a31169eac4217a38cb3ef25eb9213f2f89e11fb17e76ceaf7449b34225e98af",
    ),
];

const ALLOWLIST_PREDECESSOR_ARTIFACTS: &[ImmutableArtifactSpec] = &[
    ImmutableArtifactSpec::new(
        "crates/event_codec/contracts/phase1_publication_allowlist_v1.manifest.json",
        10_601,
        "8629b5c547e8f9daad473ab9d570b206d00db134cc22b4d8e81be10d0f3d10ec",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_codec/contracts/phase1_publication_allowlist_v1.manifest.schema.json",
        9_016,
        "638601348dece886ed9666251b5cf68d0a7f96c26dce115b65ed859a2db03d93",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_codec/contracts/phase1_publication_allowlist_v1.manifest.sha256",
        65,
        "4dd9bf6f230f02d8c2ca3c323e83fe9b77eb9f0895f708eb0949b7153b2fd6bd",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_codec/contracts/phase1_publication_allowlist_v1.descriptor.json",
        1_104,
        "d7286a1206f822382226f28e2e601dc4f12cb743f9135238467092a31bde7bee",
    ),
    ImmutableArtifactSpec::new(
        "contracts/conformance/vectors/publication/phase1_allowlist.v1.json",
        49_975,
        "2867ee401db8cfad3a77869847c57567e869623f55d3d6c9e98a7fa0a643c3d6",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_codec/tests/fixtures/phase1_publication_allowlist.v1.json",
        49_975,
        "2867ee401db8cfad3a77869847c57567e869623f55d3d6c9e98a7fa0a643c3d6",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_codec/tests/publication_allowlist.rs",
        7_562,
        "342d3d3d1100cb5d31aca94b0540b1e03761b034de23475bdb5142baabeed8fd",
    ),
];
const BLOSSOM_PREDECESSOR_ARTIFACTS: &[ImmutableArtifactSpec] = &[
    ImmutableArtifactSpec::new(
        "crates/blossom/contracts/publication_readiness_v1.manifest.json",
        13_038,
        "9359c5531548778b4a03e1a603a4048f17ba498b16dad50f7c62cca0ddb6240b",
    ),
    ImmutableArtifactSpec::new(
        "crates/blossom/contracts/publication_readiness_v1.manifest.schema.json",
        11_884,
        "e62ecf4b43bd03831f7e36e9cb4c98e2ed7e3a12d02cbca48a49e402b3feaa7d",
    ),
    ImmutableArtifactSpec::new(
        "crates/blossom/contracts/publication_readiness_v1.manifest.sha256",
        65,
        "e8c2be84eef68e965ac0e119016d6840cb9503e58d5d9c40e13a6512e1ec74b7",
    ),
    ImmutableArtifactSpec::new(
        "crates/blossom/contracts/publication_readiness_v1.descriptor.json",
        1_410,
        "4a3b33eb2b04b5a56a99b7b5bed45988a0697cb28736bfd899e012f37fc02d93",
    ),
    ImmutableArtifactSpec::new(
        "crates/blossom/contracts/publication_readiness_evidence_v1.schema.json",
        1_890,
        "c8f5f3488dd91a660f8eaa018d9aba63d03c882dc3ff22b47ac192b7189b0ce2",
    ),
    ImmutableArtifactSpec::new(
        "contracts/conformance/vectors/blossom/publication_readiness.v1.json",
        16_423,
        "6408e3b5bc4a376c7411e304833431af918a4072f196e8a8da55c3f0ef8610c9",
    ),
    ImmutableArtifactSpec::new(
        "crates/blossom/tests/fixtures/publication_readiness.v1.json",
        16_423,
        "6408e3b5bc4a376c7411e304833431af918a4072f196e8a8da55c3f0ef8610c9",
    ),
    ImmutableArtifactSpec::new(
        "crates/blossom/tests/publication_readiness.rs",
        33_431,
        "17abd5491bcfe0c717f7f67202c8c2d85912a042167d027727c07d9f07306641",
    ),
    ImmutableArtifactSpec::new(
        "contracts/conformance/vectors/blossom/publication_readiness_persistence.v1.json",
        9_264,
        "e892ff6353afe8997a151a9aff4db2fd82c96d94db7bff31bc2269333adc2512",
    ),
    ImmutableArtifactSpec::new(
        "crates/blossom/tests/fixtures/publication_readiness_persistence.v1.json",
        9_264,
        "e892ff6353afe8997a151a9aff4db2fd82c96d94db7bff31bc2269333adc2512",
    ),
    ImmutableArtifactSpec::new(
        "crates/blossom/tests/publication_readiness_persistence.rs",
        11_567,
        "aa14d20ee5747fcc979907b4b59a67687a3a02852850e33aede65e2ab0e0ca8d",
    ),
];
const ALLOWLIST_PREDECESSOR_SOURCE_SUPERSESSIONS: &[&str] = &[
    "CHANGELOG.md",
    "contracts/operations.toml",
    "contracts/releases/1.0.0-alpha.1.toml",
    "crates/event_codec/Cargo.toml",
    "crates/event_codec/src/wire/publication.rs",
    "tools/xtask/src/contract.rs",
    "tools/xtask/src/contract/nip09_reconciliation.rs",
    "tools/xtask/src/contract/phase1_publication_artifact.rs",
    "tools/xtask/src/contract/raw_source_rebuild.rs",
    "tools/xtask/src/main.rs",
];
const BLOSSOM_PREDECESSOR_SOURCE_SUPERSESSIONS: &[&str] = &[
    "CHANGELOG.md",
    "contracts/operations.toml",
    "contracts/releases/1.0.0-alpha.1.toml",
    "tools/xtask/src/contract.rs",
    "tools/xtask/src/contract/nip09_reconciliation.rs",
    "tools/xtask/src/contract/phase1_publication_artifact.rs",
    "tools/xtask/src/contract/raw_source_rebuild.rs",
    "tools/xtask/src/main.rs",
];

#[derive(Clone, Copy)]
struct ImmutableArtifactSpec {
    relative: &'static str,
    byte_length: usize,
    sha256: &'static str,
}

impl ImmutableArtifactSpec {
    const fn new(relative: &'static str, byte_length: usize, sha256: &'static str) -> Self {
        Self {
            relative,
            byte_length,
            sha256,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FileDescriptor {
    path: String,
    byte_length: u64,
    sha256: String,
    hash_algorithm: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PredecessorDescriptor {
    contract_id: String,
    immutable_artifacts: Vec<FileDescriptor>,
    source_supersessions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ProtocolSourcePin {
    id: String,
    repository: String,
    revision: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PublicApiDescriptor {
    constants: Vec<String>,
    types: Vec<String>,
    functions: Vec<String>,
    methods: Vec<String>,
    sealed_types: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct MediaEnvelopeDescriptor {
    max_url_utf8_bytes: u32,
    min_raster_bytes: u32,
    max_raster_bytes: u64,
    media_types: Vec<String>,
    exact_dimension_variants: Vec<String>,
    decoded_only_dimension_variants: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BindingDescriptor {
    schema_version: u32,
    readiness_policy_version: u16,
    schema: FileDescriptor,
    max_canonical_json_bytes: u64,
    max_evidence_count: u32,
    max_evidence_json_bytes: u32,
    wire_field_order: Vec<String>,
    digest_domain: String,
    digest_framing: String,
    bind_operation_id: String,
    serialize_operation_id: String,
    reload_operation_id: String,
    validate_operation_id: String,
    invariants: Vec<String>,
    error_codes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ResultVectorDescriptor {
    canonical_path: String,
    mirror_path: String,
    byte_length: u64,
    sha256: String,
    hash_algorithm: String,
    executor: FileDescriptor,
    executor_test: String,
    case_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct MediaReadinessManifest {
    schema_version: u32,
    contract_id: String,
    authority_id: String,
    manifest_schema: FileDescriptor,
    predecessors: Vec<PredecessorDescriptor>,
    protocol_sources: Vec<ProtocolSourcePin>,
    public_api: PublicApiDescriptor,
    media_envelope: MediaEnvelopeDescriptor,
    binding: BindingDescriptor,
    result_vector: ResultVectorDescriptor,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VectorSuite {
    suite: String,
    contract_version: String,
    vectors: Vec<VectorCase>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VectorCase {
    id: String,
    kind: String,
    input: VectorInput,
    expected: VectorExpected,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VectorInput {
    fixture: String,
    mutation: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VectorExpected {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    decision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

struct ValidatedVector {
    bytes: Vec<u8>,
    case_ids: Vec<String>,
}

pub(crate) fn write_phase1_publication_media_readiness_manifest(
    workspace_root: &Path,
) -> Result<(), String> {
    validate_predecessors(workspace_root)?;
    with_artifact_bundle_transaction(workspace_root, |transaction| {
        transaction.write(expected_artifacts(workspace_root)?)?;
        validate_manifest_under_lock(workspace_root)
    })
}

pub(crate) fn validate_phase1_publication_media_readiness_manifest(
    workspace_root: &Path,
) -> Result<(), String> {
    validate_predecessors(workspace_root)?;
    with_artifact_bundle_transaction(workspace_root, |_| {
        validate_manifest_under_lock(workspace_root)
    })
}

pub(crate) fn validate_immutable_phase1_publication_allowlist_predecessor(
    workspace_root: &Path,
) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |_| {
        validate_immutable_predecessor(
            workspace_root,
            "Phase 1 publication allowlist",
            ALLOWLIST_PREDECESSOR_ARTIFACTS,
        )
    })
}

pub(crate) fn validate_immutable_phase1_publication_artifact_predecessor(
    workspace_root: &Path,
) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |_| {
        validate_immutable_predecessor(
            workspace_root,
            "Phase 1 publication artifact",
            PUBLICATION_ARTIFACT_TRANSITIVE_PREDECESSOR_ARTIFACTS,
        )
    })
}

pub(crate) fn validate_immutable_blossom_publication_readiness_predecessor(
    workspace_root: &Path,
) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |_| {
        validate_immutable_predecessor(
            workspace_root,
            "Blossom publication readiness",
            BLOSSOM_PREDECESSOR_ARTIFACTS,
        )
    })
}

fn validate_predecessors(workspace_root: &Path) -> Result<(), String> {
    validate_source_supersessions(workspace_root, ALLOWLIST_PREDECESSOR_SOURCE_SUPERSESSIONS)?;
    validate_source_supersessions(workspace_root, BLOSSOM_PREDECESSOR_SOURCE_SUPERSESSIONS)?;
    validate_immutable_predecessor(
        workspace_root,
        "Phase 1 publication artifact",
        PUBLICATION_ARTIFACT_TRANSITIVE_PREDECESSOR_ARTIFACTS,
    )?;
    validate_immutable_predecessor(
        workspace_root,
        "Phase 1 publication allowlist",
        ALLOWLIST_PREDECESSOR_ARTIFACTS,
    )?;
    validate_immutable_predecessor(
        workspace_root,
        "Blossom publication readiness",
        BLOSSOM_PREDECESSOR_ARTIFACTS,
    )
}

fn validate_source_supersessions(workspace_root: &Path, paths: &[&str]) -> Result<(), String> {
    if paths.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("predecessor source supersessions must be sorted and unique".to_owned());
    }
    for path in paths {
        if !workspace_root.join(path).is_file() {
            return Err(format!(
                "predecessor source supersession {path} must name an existing file"
            ));
        }
    }
    Ok(())
}

fn validate_immutable_predecessor(
    workspace_root: &Path,
    label: &str,
    artifacts: &[ImmutableArtifactSpec],
) -> Result<(), String> {
    for artifact in artifacts {
        let bytes = read_regular_file(workspace_root, artifact.relative)?;
        if bytes.len() != artifact.byte_length || sha256_hex(&bytes) != artifact.sha256 {
            return Err(format!(
                "immutable {label} predecessor artifact {} drifted",
                artifact.relative
            ));
        }
    }
    Ok(())
}

fn validate_manifest_under_lock(workspace_root: &Path) -> Result<(), String> {
    for artifact in expected_artifacts(workspace_root)? {
        let actual = read_regular_file(workspace_root, artifact.relative)?;
        if actual != artifact.contents {
            return Err(format!(
                "generated Phase 1 media-readiness contract {} is stale; run {WRITE_COMMAND}",
                artifact.relative
            ));
        }
    }
    let bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest: MediaReadinessManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_RELATIVE, &bytes, &manifest)?;
    let schema_bytes = read_regular_file(workspace_root, MANIFEST_SCHEMA_RELATIVE)?;
    let schema: Value = serde_json::from_slice(&schema_bytes)
        .map_err(|error| format!("parse {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_SCHEMA_RELATIVE, &schema_bytes, &schema)?;
    validate_json_schema(
        &schema,
        &serde_json::to_value(&manifest)
            .map_err(|error| format!("serialize {MANIFEST_RELATIVE}: {error}"))?,
    )?;
    let binding_schema_bytes = read_regular_file(workspace_root, BINDING_SCHEMA_RELATIVE)?;
    let binding_schema: Value = serde_json::from_slice(&binding_schema_bytes)
        .map_err(|error| format!("parse {BINDING_SCHEMA_RELATIVE}: {error}"))?;
    validate_canonical_json(
        BINDING_SCHEMA_RELATIVE,
        &binding_schema_bytes,
        &binding_schema,
    )?;
    let sidecar = read_regular_file(workspace_root, MANIFEST_SHA256_RELATIVE)?;
    if sidecar != format!("{}\n", sha256_hex(&bytes)).as_bytes() {
        return Err(format!(
            "{MANIFEST_SHA256_RELATIVE} must authenticate the exact manifest bytes"
        ));
    }
    Ok(())
}

fn expected_artifacts(workspace_root: &Path) -> Result<Vec<GeneratedArtifact>, String> {
    validate_source_contract(workspace_root)?;
    let binding_schema_bytes = canonical_json_bytes(&binding_schema())?;
    let manifest_schema_bytes = canonical_json_bytes(&manifest_schema())?;
    let manifest = describe_manifest(
        workspace_root,
        &manifest_schema_bytes,
        &binding_schema_bytes,
    )?;
    let manifest_bytes = canonical_json_bytes(&manifest)?;
    let sidecar_bytes = format!("{}\n", sha256_hex(&manifest_bytes)).into_bytes();
    let descriptor_bytes = canonical_json_bytes(&json!({
        "schema_version": SCHEMA_VERSION,
        "contract_id": CONTRACT_ID,
        "manifest": descriptor_for_bytes(MANIFEST_RELATIVE, &manifest_bytes),
        "manifest_schema": descriptor_for_bytes(MANIFEST_SCHEMA_RELATIVE, &manifest_schema_bytes),
        "manifest_sidecar": descriptor_for_bytes(MANIFEST_SHA256_RELATIVE, &sidecar_bytes),
        "binding_schema": descriptor_for_bytes(BINDING_SCHEMA_RELATIVE, &binding_schema_bytes),
        "predecessor_contract_ids": [
            "radroots_event_codec.phase1_publication_allowlist_v1",
            "radroots_blossom.publication_readiness_v1"
        ]
    }))?;
    let vector_bytes = read_regular_file(workspace_root, VECTOR_RELATIVE)?;
    Ok(vec![
        GeneratedArtifact {
            relative: MANIFEST_RELATIVE,
            contents: manifest_bytes,
        },
        GeneratedArtifact {
            relative: MANIFEST_SCHEMA_RELATIVE,
            contents: manifest_schema_bytes,
        },
        GeneratedArtifact {
            relative: MANIFEST_SHA256_RELATIVE,
            contents: sidecar_bytes,
        },
        GeneratedArtifact {
            relative: GENERATED_DESCRIPTOR_RELATIVE,
            contents: descriptor_bytes,
        },
        GeneratedArtifact {
            relative: BINDING_SCHEMA_RELATIVE,
            contents: binding_schema_bytes,
        },
        GeneratedArtifact {
            relative: VECTOR_MIRROR_RELATIVE,
            contents: vector_bytes,
        },
    ])
}

fn describe_manifest(
    workspace_root: &Path,
    manifest_schema_bytes: &[u8],
    binding_schema_bytes: &[u8],
) -> Result<MediaReadinessManifest, String> {
    let vector = validate_vector(workspace_root)?;
    Ok(MediaReadinessManifest {
        schema_version: SCHEMA_VERSION,
        contract_id: CONTRACT_ID.to_owned(),
        authority_id: AUTHORITY_ID.to_owned(),
        manifest_schema: descriptor_for_bytes(MANIFEST_SCHEMA_RELATIVE, manifest_schema_bytes),
        predecessors: vec![
            predecessor_descriptor(
                "radroots_event_codec.phase1_publication_allowlist_v1",
                ALLOWLIST_PREDECESSOR_ARTIFACTS,
                ALLOWLIST_PREDECESSOR_SOURCE_SUPERSESSIONS,
            ),
            predecessor_descriptor(
                "radroots_blossom.publication_readiness_v1",
                BLOSSOM_PREDECESSOR_ARTIFACTS,
                BLOSSOM_PREDECESSOR_SOURCE_SUPERSESSIONS,
            ),
        ],
        protocol_sources: vec![
            ProtocolSourcePin {
                id: "nostr_nips".to_owned(),
                repository: "https://github.com/nostr-protocol/nips".to_owned(),
                revision: "bdfa7e62ef87fcfcb992b1a27aee49d36b0b4f91".to_owned(),
            },
            ProtocolSourcePin {
                id: "blossom".to_owned(),
                repository: "https://github.com/hzrd149/blossom".to_owned(),
                revision: "b5bd2801d1763aa635fc8fea7a76597e0eb18990".to_owned(),
            },
        ],
        public_api: PublicApiDescriptor {
            constants: owned(PUBLIC_CONSTANTS),
            types: owned(PUBLIC_TYPES),
            functions: owned(PUBLIC_FUNCTIONS),
            methods: owned(PUBLIC_METHODS),
            sealed_types: owned(SEALED_TYPES),
        },
        media_envelope: MediaEnvelopeDescriptor {
            max_url_utf8_bytes: URL_MAX_BYTES,
            min_raster_bytes: 1,
            max_raster_bytes: RASTER_MAX_BYTES,
            media_types: owned(&["image/jpeg", "image/png", "image/webp"]),
            exact_dimension_variants: owned(&["photo_update", "ask", "food_availability"]),
            decoded_only_dimension_variants: owned(&[
                "profile",
                "event_date",
                "event_time",
            ]),
        },
        binding: BindingDescriptor {
            schema_version: BINDING_SCHEMA_VERSION,
            readiness_policy_version: READINESS_POLICY_VERSION,
            schema: descriptor_for_bytes(BINDING_SCHEMA_RELATIVE, binding_schema_bytes),
            max_canonical_json_bytes: BINDING_MAX_BYTES,
            max_evidence_count: EVIDENCE_MAX_COUNT,
            max_evidence_json_bytes: EVIDENCE_MAX_BYTES,
            wire_field_order: owned(WIRE_FIELD_ORDER),
            digest_domain: DIGEST_DOMAIN.to_owned(),
            digest_framing: "domain_bytes_then_u32be_schema_then_u16be_policy_then_raw_artifact_digest_then_u32be_evidence_count_then_repeated_u64be_url_length_url_bytes_raw_evidence_digest_v1".to_owned(),
            bind_operation_id: BIND_OPERATION_ID.to_owned(),
            serialize_operation_id: SERIALIZE_OPERATION_ID.to_owned(),
            reload_operation_id: RELOAD_OPERATION_ID.to_owned(),
            validate_operation_id: VALIDATE_OPERATION_ID.to_owned(),
            invariants: owned(SEMANTIC_INVARIANTS),
            error_codes: owned(ERROR_CODES),
        },
        result_vector: ResultVectorDescriptor {
            canonical_path: VECTOR_RELATIVE.to_owned(),
            mirror_path: VECTOR_MIRROR_RELATIVE.to_owned(),
            byte_length: vector.bytes.len() as u64,
            sha256: sha256_hex(&vector.bytes),
            hash_algorithm: HASH_ALGORITHM.to_owned(),
            executor: descriptor_for_file(workspace_root, VECTOR_EXECUTOR_RELATIVE)?,
            executor_test: VECTOR_EXECUTOR_TEST.to_owned(),
            case_ids: vector.case_ids,
        },
    })
}

fn predecessor_descriptor(
    contract_id: &str,
    artifacts: &[ImmutableArtifactSpec],
    source_supersessions: &[&str],
) -> PredecessorDescriptor {
    PredecessorDescriptor {
        contract_id: contract_id.to_owned(),
        immutable_artifacts: artifacts
            .iter()
            .map(|artifact| FileDescriptor {
                path: artifact.relative.to_owned(),
                byte_length: artifact.byte_length as u64,
                sha256: artifact.sha256.to_owned(),
                hash_algorithm: HASH_ALGORITHM.to_owned(),
            })
            .collect(),
        source_supersessions: owned(source_supersessions),
    }
}

fn validate_source_contract(workspace_root: &Path) -> Result<(), String> {
    validate_public_api(workspace_root)?;
    validate_publication_route(workspace_root)?;
    validate_feature_authority(workspace_root)?;
    validate_operations_authority(workspace_root)?;
    validate_release_authority(workspace_root)?;
    validate_vector_executor(workspace_root)?;
    validate_vector(workspace_root)?;
    Ok(())
}

fn validate_vector_executor(workspace_root: &Path) -> Result<(), String> {
    let bytes = read_regular_file(workspace_root, VECTOR_EXECUTOR_RELATIVE)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{VECTOR_EXECUTOR_RELATIVE} must be UTF-8: {error}"))?;
    let file = syn::parse_file(source)
        .map_err(|error| format!("parse {VECTOR_EXECUTOR_RELATIVE}: {error}"))?;
    let tests = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) if function.sig.ident == VECTOR_EXECUTOR_TEST => Some(function),
            _ => None,
        })
        .collect::<Vec<_>>();
    if tests.len() != 1 {
        return Err(format!(
            "{VECTOR_EXECUTOR_RELATIVE} must define {VECTOR_EXECUTOR_TEST} exactly once"
        ));
    }
    let test = tests[0];
    if test.attrs.len() != 1
        || !test.attrs[0].path().is_ident("test")
        || test.sig.constness.is_some()
        || test.sig.asyncness.is_some()
        || test.sig.unsafety.is_some()
        || test.sig.abi.is_some()
        || !test.sig.inputs.is_empty()
        || !matches!(test.sig.output, syn::ReturnType::Default)
    {
        return Err(format!(
            "{VECTOR_EXECUTOR_RELATIVE}::{VECTOR_EXECUTOR_TEST} must be one unconditional zero-argument #[test]"
        ));
    }

    #[derive(Default)]
    struct OperationCalls(BTreeSet<String>);

    impl<'ast> Visit<'ast> for OperationCalls {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if let Expr::Path(path) = call.func.as_ref()
                && let Some(segment) = path.path.segments.last()
            {
                self.0.insert(segment.ident.to_string());
            }
            syn::visit::visit_expr_call(self, call);
        }

        fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
            self.0.insert(call.method.to_string());
            syn::visit::visit_expr_method_call(self, call);
        }
    }

    let mut calls = OperationCalls::default();
    calls.visit_file(&file);
    for required in [
        "bind_phase1_publication_media_readiness",
        "to_canonical_json",
        "from_canonical_json",
        "validate_phase1_publication_media_readiness",
    ] {
        if !calls.0.contains(required) {
            return Err(format!(
                "{VECTOR_EXECUTOR_RELATIVE} must execute the public media-readiness operation {required}"
            ));
        }
    }
    Ok(())
}

fn validate_public_api(workspace_root: &Path) -> Result<(), String> {
    let bytes = read_regular_file(workspace_root, SOURCE_RELATIVE)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{SOURCE_RELATIVE} must be UTF-8: {error}"))?;
    let file =
        syn::parse_file(source).map_err(|error| format!("parse {SOURCE_RELATIVE}: {error}"))?;
    let mut constants = BTreeSet::new();
    let mut types = BTreeSet::new();
    let mut functions = BTreeSet::new();
    let mut methods = BTreeSet::new();
    let mut sealed_without_deserialize = BTreeSet::new();
    let mut public_types_with_private_fields = BTreeSet::new();
    let mut private_wire_types = BTreeSet::new();
    let mut forbidden_deserialize = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Const(item) if is_public(&item.vis) => {
                constants.insert(item.ident.to_string());
            }
            Item::Struct(item) if is_public(&item.vis) => {
                let name = item.ident.to_string();
                types.insert(name.clone());
                if item.fields.iter().all(|field| !is_public(&field.vis)) {
                    public_types_with_private_fields.insert(name.clone());
                }
                if SEALED_TYPES.contains(&name.as_str()) && !derives_deserialize(&item.attrs)? {
                    sealed_without_deserialize.insert(name);
                }
            }
            Item::Enum(item) if is_public(&item.vis) => {
                let name = item.ident.to_string();
                types.insert(name.clone());
                if item
                    .variants
                    .iter()
                    .flat_map(|variant| variant.fields.iter())
                    .all(|field| !is_public(&field.vis))
                {
                    public_types_with_private_fields.insert(name.clone());
                }
                if SEALED_TYPES.contains(&name.as_str()) && !derives_deserialize(&item.attrs)? {
                    sealed_without_deserialize.insert(name);
                }
            }
            Item::Struct(item) => {
                let name = item.ident.to_string();
                if PRIVATE_WIRE_TYPES.contains(&name.as_str())
                    && derives_deserialize(&item.attrs)?
                    && has_serde_deny_unknown_fields(&item.attrs)?
                {
                    private_wire_types.insert(name);
                }
            }
            Item::Fn(item) if is_public(&item.vis) => {
                functions.insert(item.sig.ident.to_string());
            }
            Item::Impl(item) if item.trait_.is_none() => {
                let syn::Type::Path(self_type) = item.self_ty.as_ref() else {
                    continue;
                };
                let Some(type_name) = self_type.path.segments.last() else {
                    continue;
                };
                for method in &item.items {
                    if let ImplItem::Fn(method) = method
                        && is_public(&method.vis)
                    {
                        methods.insert(format!("{}::{}", type_name.ident, method.sig.ident));
                    }
                }
            }
            Item::Impl(item) => {
                let syn::Type::Path(self_type) = item.self_ty.as_ref() else {
                    continue;
                };
                let Some(type_name) = self_type.path.segments.last() else {
                    continue;
                };
                if item.trait_.as_ref().is_some_and(|(_, path, _)| {
                    path.segments
                        .last()
                        .is_some_and(|segment| segment.ident == "Deserialize")
                }) && SEALED_TYPES.contains(&type_name.ident.to_string().as_str())
                {
                    forbidden_deserialize.insert(type_name.ident.to_string());
                }
            }
            _ => {}
        }
    }
    for (label, actual, expected) in [
        ("constants", constants, expected_set(PUBLIC_CONSTANTS)),
        ("types", types, expected_set(PUBLIC_TYPES)),
        ("functions", functions, expected_set(PUBLIC_FUNCTIONS)),
        ("methods", methods, expected_set(PUBLIC_METHODS)),
    ] {
        if actual != expected {
            return Err(format!(
                "{SOURCE_RELATIVE} public {label} drifted: expected {expected:?}, found {actual:?}"
            ));
        }
    }
    if sealed_without_deserialize != expected_set(SEALED_TYPES) {
        return Err("sealed media-readiness types must not derive Deserialize".to_owned());
    }
    if public_types_with_private_fields != expected_set(PUBLIC_TYPES) {
        return Err("public media-readiness types must expose no public fields".to_owned());
    }
    if private_wire_types != expected_set(PRIVATE_WIRE_TYPES) {
        return Err(
            "private media-readiness wire types must derive Deserialize with deny_unknown_fields"
                .to_owned(),
        );
    }
    if !forbidden_deserialize.is_empty() {
        return Err(format!(
            "sealed media-readiness types must not implement Deserialize: {forbidden_deserialize:?}"
        ));
    }
    Ok(())
}

fn derives_deserialize(attributes: &[syn::Attribute]) -> Result<bool, String> {
    for attribute in attributes {
        if attribute.path().is_ident("derive") {
            let paths = attribute
                .parse_args_with(Punctuated::<syn::Path, Comma>::parse_terminated)
                .map_err(|error| format!("parse derive attribute: {error}"))?;
            if paths.iter().any(|path| {
                path.segments
                    .last()
                    .is_some_and(|segment| segment.ident == "Deserialize")
            }) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn has_serde_deny_unknown_fields(attributes: &[syn::Attribute]) -> Result<bool, String> {
    for attribute in attributes {
        if attribute.path().is_ident("serde") {
            let mut found = false;
            attribute
                .parse_nested_meta(|meta| {
                    if meta.path.is_ident("deny_unknown_fields") {
                        found = true;
                    }
                    Ok(())
                })
                .map_err(|error| format!("parse serde attribute: {error}"))?;
            if found {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn validate_publication_route(workspace_root: &Path) -> Result<(), String> {
    let bytes = read_regular_file(workspace_root, PUBLICATION_SOURCE_RELATIVE)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{PUBLICATION_SOURCE_RELATIVE} must be UTF-8: {error}"))?;
    let file = syn::parse_file(source)
        .map_err(|error| format!("parse {PUBLICATION_SOURCE_RELATIVE}: {error}"))?;
    let module = file.items.iter().any(|item| {
        matches!(item, Item::Mod(module) if module.ident == "media_readiness" && module.content.is_none())
    });
    if !module {
        return Err(format!(
            "{PUBLICATION_SOURCE_RELATIVE} must declare the media_readiness module"
        ));
    }
    let mut reexports = BTreeSet::new();
    let mut reexport_declarations = 0usize;
    for item in &file.items {
        if let Item::Use(item) = item
            && is_public(&item.vis)
            && let syn::UseTree::Path(path) = &item.tree
            && path.ident == "media_readiness"
        {
            reexport_declarations += 1;
            collect_use_names(&path.tree, &mut reexports);
        }
    }
    let expected = PUBLIC_CONSTANTS
        .iter()
        .chain(PUBLIC_TYPES)
        .chain(PUBLIC_FUNCTIONS)
        .map(|value| (*value).to_owned())
        .collect::<BTreeSet<_>>();
    if reexport_declarations != 1 || reexports != expected {
        return Err(format!(
            "{PUBLICATION_SOURCE_RELATIVE} media-readiness reexports drifted: expected {expected:?}, found {reexports:?}"
        ));
    }
    Ok(())
}

fn collect_use_names(tree: &syn::UseTree, names: &mut BTreeSet<String>) {
    match tree {
        syn::UseTree::Name(name) => {
            names.insert(name.ident.to_string());
        }
        syn::UseTree::Rename(rename) => {
            names.insert(rename.rename.to_string());
        }
        syn::UseTree::Path(path) => collect_use_names(&path.tree, names),
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_use_names(item, names);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

fn validate_feature_authority(workspace_root: &Path) -> Result<(), String> {
    let manifest = parse_toml(workspace_root, EVENT_CODEC_MANIFEST_RELATIVE)?;
    let features = manifest
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "event-codec features table is missing".to_owned())?;
    let serde_json =
        toml_string_array("event-codec serde_json feature", features.get("serde_json"))?;
    let expected = [
        "serde",
        "dep:hex",
        "dep:serde_json",
        "dep:sha2",
        "radroots_blossom/serde",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    if serde_json.into_iter().collect::<BTreeSet<_>>() != expected {
        return Err("event-codec serde_json feature authority drifted".to_owned());
    }
    let dependency = manifest
        .get("dependencies")
        .and_then(|dependencies| dependencies.get("serde_json"))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "event-codec serde_json dependency is missing".to_owned())?;
    let dependency_features = toml_string_array(
        "event-codec serde_json dependency features",
        dependency.get("features"),
    )?;
    if dependency
        .get("default-features")
        .and_then(toml::Value::as_bool)
        != Some(false)
        || dependency.get("optional").and_then(toml::Value::as_bool) != Some(true)
        || dependency_features.into_iter().collect::<BTreeSet<_>>()
            != ["alloc", "raw_value"]
                .into_iter()
                .map(str::to_owned)
                .collect()
    {
        return Err("event-codec bounded raw JSON dependency profile drifted".to_owned());
    }
    Ok(())
}

fn validate_operations_authority(workspace_root: &Path) -> Result<(), String> {
    let manifest = parse_toml(workspace_root, OPERATIONS_RELATIVE)?;
    let public_types = toml_string_array(
        "shared_types.public",
        manifest
            .get("shared_types")
            .and_then(|value| value.get("public")),
    )?;
    for required in PUBLIC_TYPES {
        if public_types
            .iter()
            .filter(|value| value.as_str() == *required)
            .count()
            != 1
        {
            return Err(format!(
                "shared public types must contain {required} exactly once"
            ));
        }
    }
    let operations = manifest
        .get("operations")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "operations.toml has no operations table".to_owned())?;
    for expectation in [
        OperationExpectation::new(
            "phase1_publication_media_readiness_bind",
            BIND_OPERATION_ID,
            &[
                "RadrootsPhase1AllowlistedPublicationArtifact",
                "RadrootsBlossomPublicationReadinessEvidence",
            ],
            &["RadrootsPhase1MediaReadyPublicationArtifact"],
            "validation_error",
            &[
                "publication_media_readiness.bind.valid",
                "publication_media_readiness.bind.invalid",
                "publication_media_readiness.bind.artifact_invalid",
            ],
        ),
        OperationExpectation::new(
            "phase1_publication_media_readiness_to_canonical_json",
            SERIALIZE_OPERATION_ID,
            &["RadrootsPhase1MediaReadyPublicationArtifact"],
            &["Bytes"],
            "none",
            &["publication_media_readiness.to_canonical_json.valid"],
        ),
        OperationExpectation::new(
            "phase1_publication_media_readiness_from_canonical_json",
            RELOAD_OPERATION_ID,
            &["RadrootsPhase1AllowlistedPublicationArtifact", "Bytes"],
            &["RadrootsPhase1MediaReadyPublicationArtifact"],
            "parse_error",
            &[
                "publication_media_readiness.from_canonical_json.valid",
                "publication_media_readiness.from_canonical_json.invalid",
            ],
        ),
        OperationExpectation::new(
            "phase1_publication_media_readiness_validate",
            VALIDATE_OPERATION_ID,
            &["RadrootsPhase1MediaReadyPublicationArtifact"],
            &["Unit"],
            "validation_error",
            &["publication_media_readiness.validate.valid"],
        ),
    ] {
        let operation = operations
            .get(expectation.key)
            .and_then(toml::Value::as_table)
            .ok_or_else(|| format!("operations.toml is missing {}", expectation.key))?;
        validate_operation(operation, expectation)?;
    }
    Ok(())
}

fn validate_release_authority(workspace_root: &Path) -> Result<(), String> {
    let release = parse_toml(workspace_root, RELEASE_RELATIVE)?;
    let changes = release
        .get("changes")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{RELEASE_RELATIVE} must declare changes"))?;
    let matching = changes
        .iter()
        .filter(|change| change.get("id").and_then(toml::Value::as_str) == Some(RELEASE_CHANGE_ID))
        .collect::<Vec<_>>();
    if matching.len() != 1
        || matching[0]
            .get("classification")
            .and_then(toml::Value::as_str)
            != Some("feature")
    {
        return Err(format!(
            "{RELEASE_RELATIVE} must contain exactly one feature change {RELEASE_CHANGE_ID}"
        ));
    }
    let changelog = read_regular_file(workspace_root, CHANGELOG_RELATIVE)?;
    let changelog = std::str::from_utf8(&changelog)
        .map_err(|error| format!("{CHANGELOG_RELATIVE} must be UTF-8: {error}"))?;
    if changelog.matches(CHANGELOG_MARKER).count() != 1 {
        return Err(format!(
            "{CHANGELOG_RELATIVE} must contain exactly one {CHANGELOG_MARKER}"
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct OperationExpectation {
    key: &'static str,
    id: &'static str,
    inputs: &'static [&'static str],
    outputs: &'static [&'static str],
    error_class: &'static str,
    case_kinds: &'static [&'static str],
}

impl OperationExpectation {
    const fn new(
        key: &'static str,
        id: &'static str,
        inputs: &'static [&'static str],
        outputs: &'static [&'static str],
        error_class: &'static str,
        case_kinds: &'static [&'static str],
    ) -> Self {
        Self {
            key,
            id,
            inputs,
            outputs,
            error_class,
            case_kinds,
        }
    }
}

fn validate_operation(
    operation: &toml::map::Map<String, toml::Value>,
    expectation: OperationExpectation,
) -> Result<(), String> {
    for (field, expected) in [
        ("domain", "publication"),
        ("id", expectation.id),
        ("stability", "beta"),
        ("error_class", expectation.error_class),
        ("signing", "none"),
        ("transport", "none"),
    ] {
        if operation.get(field).and_then(toml::Value::as_str) != Some(expected) {
            return Err(format!("{} {field} must be {expected}", expectation.key));
        }
    }
    if operation
        .get("deterministic")
        .and_then(toml::Value::as_bool)
        != Some(true)
        || toml_string_array("operation inputs", operation.get("inputs"))?
            != owned(expectation.inputs)
        || toml_string_array("operation outputs", operation.get("outputs"))?
            != owned(expectation.outputs)
    {
        return Err(format!(
            "{} signature or determinism drifted",
            expectation.key
        ));
    }
    let implementation = operation
        .get("implementation")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{} implementation is missing", expectation.key))?;
    let modules = toml_string_array("operation modules", implementation.get("rust_modules"))?;
    if !modules.iter().any(|module| module == SOURCE_RELATIVE) {
        return Err(format!(
            "{} must route through {SOURCE_RELATIVE}",
            expectation.key
        ));
    }
    let conformance = operation
        .get("conformance")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{} conformance is missing", expectation.key))?;
    if conformance.get("vector").and_then(toml::Value::as_str) != Some(VECTOR_RELATIVE) {
        return Err(format!("{} conformance vector drifted", expectation.key));
    }
    if toml_string_array("operation case kinds", conformance.get("case_kinds"))?
        != owned(expectation.case_kinds)
    {
        return Err(format!(
            "{} conformance case kinds drifted",
            expectation.key
        ));
    }
    Ok(())
}

fn validate_vector(workspace_root: &Path) -> Result<ValidatedVector, String> {
    let bytes = read_regular_file(workspace_root, VECTOR_RELATIVE)?;
    let suite: VectorSuite = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {VECTOR_RELATIVE}: {error}"))?;
    if suite.suite != "phase1_publication_media_readiness" || suite.contract_version != "1.0.0" {
        return Err("Phase 1 media-readiness vector identity drifted".to_owned());
    }
    validate_canonical_json(VECTOR_RELATIVE, &bytes, &suite)?;
    if suite.vectors.len() != 39 {
        return Err("Phase 1 media-readiness vector must contain exactly 39 cases".to_owned());
    }
    let mut ids = BTreeSet::new();
    let mut valid_fixtures = BTreeSet::new();
    let mut mutations = BTreeSet::new();
    for case in &suite.vectors {
        if !ids.insert(case.id.clone())
            || case.input.fixture.is_empty()
            || case.input.mutation.is_empty()
        {
            return Err(
                "Phase 1 media-readiness vector ids and inputs must be unique/nonempty".to_owned(),
            );
        }
        match case.kind.as_str() {
            "publication_media_readiness.bind.valid"
            | "publication_media_readiness.to_canonical_json.valid"
            | "publication_media_readiness.from_canonical_json.valid"
            | "publication_media_readiness.validate.valid" => {
                if case.expected.decision.as_deref() != Some("allow")
                    || case.expected.error.is_some()
                {
                    return Err(format!("{} has invalid allow expectation", case.id));
                }
                valid_fixtures.insert(case.input.fixture.clone());
            }
            "publication_media_readiness.bind.invalid"
            | "publication_media_readiness.from_canonical_json.invalid"
            | "publication_media_readiness.bind.artifact_invalid" => {
                if case.expected.decision.is_some()
                    || !case.expected.error.as_deref().is_some_and(|error| {
                        ERROR_CODES.contains(&error) || ARTIFACT_ERROR_CODES.contains(&error)
                    })
                {
                    return Err(format!("{} has invalid rejection expectation", case.id));
                }
                mutations.insert(case.input.mutation.clone());
            }
            kind => return Err(format!("{} uses unsupported kind {kind}", case.id)),
        }
    }
    let expected_fixtures = [
        "profile",
        "update",
        "photo_update",
        "ask",
        "event_date",
        "event_time",
        "food_availability",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    if valid_fixtures != expected_fixtures {
        return Err("Phase 1 media-readiness vector must execute all seven leaves".to_owned());
    }
    for required in [
        "missing",
        "extra",
        "duplicate",
        "reordered",
        "size_mismatch",
        "dimension_mismatch",
        "cross_artifact",
        "digest_invalid",
        "binding_exact_max",
        "binding_over_max",
        "evidence_count_exact_max",
        "evidence_count_over_max",
        "wire_evidence_count_exact_max",
        "wire_evidence_count_over_max",
        "nested_bud11_field",
        "url_exact_max",
        "url_over_max",
        "size_zero",
        "size_over_max",
        "mime_unsupported",
        "dimensions_over_max",
    ] {
        if !mutations.contains(required) {
            return Err(format!(
                "Phase 1 media-readiness vector is missing {required}"
            ));
        }
    }
    Ok(ValidatedVector {
        bytes,
        case_ids: ids.into_iter().collect(),
    })
}

fn manifest_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://radroots.org/schemas/event-codec/phase1-publication-media-readiness-manifest-v1.json",
        "title": "Radroots Phase 1 publication media-readiness semantic contract",
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "contract_id", "authority_id", "manifest_schema", "predecessors", "protocol_sources", "public_api", "media_envelope", "binding", "result_vector"],
        "properties": {
            "schema_version": {"const": SCHEMA_VERSION},
            "contract_id": {"const": CONTRACT_ID},
            "authority_id": {"const": AUTHORITY_ID},
            "manifest_schema": {"$ref": "#/$defs/file"},
            "predecessors": {
                "type": "array",
                "minItems": 2,
                "maxItems": 2,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["contract_id", "immutable_artifacts", "source_supersessions"],
                    "properties": {
                        "contract_id": {"type": "string", "minLength": 1},
                        "immutable_artifacts": {"type": "array", "minItems": 1, "items": {"$ref": "#/$defs/file"}},
                        "source_supersessions": {"type": "array", "minItems": 1, "items": {"type": "string", "minLength": 1}, "uniqueItems": true}
                    }
                }
            },
            "protocol_sources": {"type": "array", "minItems": 2, "maxItems": 2, "items": {"type": "object"}},
            "public_api": {"type": "object"},
            "media_envelope": {"type": "object"},
            "binding": {"type": "object"},
            "result_vector": {"type": "object"}
        },
        "$defs": {
            "file": {
                "type": "object",
                "additionalProperties": false,
                "required": ["path", "byte_length", "sha256", "hash_algorithm"],
                "properties": {
                    "path": {"type": "string", "minLength": 1},
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "hash_algorithm": {"const": HASH_ALGORITHM}
                }
            }
        }
    })
}

fn binding_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://radroots.org/schemas/event-codec/phase1-publication-media-readiness-binding-v1.json",
        "title": "Radroots Phase 1 publication media-readiness binding v1",
        "type": "object",
        "additionalProperties": false,
        "required": WIRE_FIELD_ORDER,
        "properties": {
            "schema_version": {"const": BINDING_SCHEMA_VERSION},
            "readiness_policy_version": {"const": READINESS_POLICY_VERSION},
            "artifact_digest": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
            "evidence": {
                "type": "array",
                "maxItems": EVIDENCE_MAX_COUNT,
                "items": {"type": "object"}
            },
            "binding_digest": {"type": "string", "pattern": "^[0-9a-f]{64}$"}
        }
    })
}

fn is_public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

fn expected_set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn owned(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn descriptor_for_file(workspace_root: &Path, path: &str) -> Result<FileDescriptor, String> {
    Ok(descriptor_for_bytes(
        path,
        &read_regular_file(workspace_root, path)?,
    ))
}

fn descriptor_for_bytes(path: &str, bytes: &[u8]) -> FileDescriptor {
    FileDescriptor {
        path: path.to_owned(),
        byte_length: bytes.len() as u64,
        sha256: sha256_hex(bytes),
        hash_algorithm: HASH_ALGORITHM.to_owned(),
    }
}

fn parse_toml(workspace_root: &Path, relative: &str) -> Result<toml::Value, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{relative} must be UTF-8: {error}"))?;
    toml::from_str(source).map_err(|error| format!("parse {relative}: {error}"))
}

fn toml_string_array(label: &str, value: Option<&toml::Value>) -> Result<Vec<String>, String> {
    value
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{label} must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{label} values must be strings"))
        })
        .collect()
}

fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|error| format!("serialize JSON: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn validate_canonical_json<T: Serialize>(
    relative: &str,
    bytes: &[u8],
    value: &T,
) -> Result<(), String> {
    if canonical_json_bytes(value)? != bytes {
        return Err(format!("{relative} is not canonical pretty JSON"));
    }
    Ok(())
}

fn validate_json_schema(schema: &Value, instance: &Value) -> Result<(), String> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|error| format!("compile {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    let errors = validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{MANIFEST_RELATIVE} violates its schema: {}",
            errors.join("; ")
        ))
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
