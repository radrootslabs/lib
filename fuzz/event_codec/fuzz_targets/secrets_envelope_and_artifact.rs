#![no_main]

use libfuzzer_sys::fuzz_target;
use radroots_secrets::EncryptedEnvelope;
use radroots_storage::private_artifact::PrivateArtifactMetadata;

fuzz_target!(|data: &[u8]| {
    if let Ok(envelope) = EncryptedEnvelope::decode(data) {
        let encoded = envelope.encode().expect("decoded envelope must re-encode");
        assert_eq!(encoded, data);
    }
    let _ = serde_json::from_slice::<PrivateArtifactMetadata>(data);
});
