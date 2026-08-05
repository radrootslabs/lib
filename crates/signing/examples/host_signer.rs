use radroots_event::{GenericEventDraft, contract::AuthorRole};
use radroots_event_codec::authoring::AuthoredEventPlan;
use radroots_identity::PublicKey;
use radroots_protocol::runtime::v1::OperationId;
use radroots_signing::{
    Actor, AuthoredArtifactId, Error, SignReceipt, SignRequest, Signer, SignerStatus,
    SigningIntentId, SigningOperationId,
    actor::ActorSource,
    error::Kind,
    request::{CancellationPolicy, SignPolicy},
    signer::BoxFuture,
};

struct HostSigner;

impl Signer for HostSigner {
    fn status(&self) -> BoxFuture<'_, Result<SignerStatus, Error>> {
        Box::pin(async { Ok(SignerStatus::unavailable()) })
    }

    fn sign(&self, _request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, Error>> {
        Box::pin(async { Err(Error::new(Kind::SignerUnavailable)) })
    }
}

fn main() {
    let public_key =
        PublicKey::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .expect("canonical public key");
    let actor = Actor::new(
        public_key,
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Any],
    )
    .expect("validated actor");
    let draft = GenericEventDraft::new(
        "radroots.social.geochat.v1",
        20_000,
        1_700_000_000,
        Vec::new(),
        "host-composed signing",
        public_key.to_hex(),
    )
    .expect("validated generic draft");
    let plan = AuthoredEventPlan::from_generic(draft).expect("exact authored plan");
    let intent_id = SigningIntentId::new(
        SigningOperationId::new([1; 16]).expect("operation ID"),
        AuthoredArtifactId::new([2; 16]).expect("artifact ID"),
    );
    let policy = SignPolicy::new(
        1_700_000_030_000,
        CancellationPolicy::PreservePublishedRequest,
    )
    .expect("bounded policy");
    let request = SignRequest::new(OperationId::SyncPush, intent_id, actor, plan, policy)
        .expect("authorized request");

    let signer: &dyn Signer = &HostSigner;
    let future = signer.sign(request);
    drop(future); // The composing host chooses and drives its async executor.
}
