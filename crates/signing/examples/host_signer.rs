use radroots_event::{EventDraft, contract::AuthorRole};
use radroots_identity::PublicKey;
use radroots_protocol::runtime::v1::OperationId;
use radroots_signing::{
    Actor, Error, SignReceipt, SignRequest, Signer, SignerStatus,
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
    let draft = EventDraft::new(
        "radroots.social.geochat.v1",
        20_000,
        1_700_000_000,
        Vec::new(),
        "host-composed signing",
        public_key.to_hex(),
    )
    .expect("frozen draft");
    let policy = SignPolicy::new(1_700_000_030, CancellationPolicy::PreservePublishedRequest)
        .expect("bounded policy");
    let request =
        SignRequest::new(OperationId::SyncPush, actor, draft, policy).expect("authorized request");

    let signer: &dyn Signer = &HostSigner;
    let future = signer.sign(request);
    drop(future); // The composing host chooses and drives its async executor.
}
