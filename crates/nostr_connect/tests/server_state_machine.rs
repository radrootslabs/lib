use radroots_nostr_connect::message::{RequestId, RequestMessage};
use radroots_nostr_connect::permission::{Permission, Permissions};
use radroots_nostr_connect::{Error, Method, Request, Response, Server};
use std::str::FromStr;

fn request_json(id: &str, request: Request) -> String {
    serde_json::to_string(&RequestMessage::try_new(id, request).expect("request"))
        .expect("request JSON")
}

#[test]
fn server_exposes_permission_evaluation_without_owning_policy() {
    let mut server = Server::new();
    let request = server
        .parse(
            "event-permission",
            &request_json(
                "request-permission",
                Request::Nip44Encrypt {
                    public_key: radroots_identity::PublicKey::from_hex(
                        "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
                    )
                    .expect("public key"),
                    plaintext: "payload".to_owned(),
                },
            ),
        )
        .expect("server request");
    assert_eq!(
        request.required_permission(),
        Some(&Permission::new(Method::Nip44Encrypt))
    );
    assert!(!request.is_allowed_by(&Permissions::new()));
    assert!(
        request.is_allowed_by(
            &Permissions::try_from_vec(vec![Permission::new(Method::Nip44Encrypt)])
                .expect("permissions")
        )
    );
}

#[test]
fn server_rejects_unsupported_extensions_and_malformed_requests() {
    let mut server = Server::new();
    let extension = Method::from_str("vendor_action").expect("extension");
    assert_eq!(
        server
            .parse(
                "event-extension",
                &request_json(
                    "request-extension",
                    Request::Custom {
                        method: extension.clone(),
                        params: Vec::new(),
                    },
                ),
            )
            .expect_err("unsupported extension"),
        Error::UnsupportedMethod(extension)
    );
    assert!(matches!(
        server.parse("event-malformed", "not JSON"),
        Err(Error::Json(_))
    ));
}

#[test]
fn configured_extension_is_admitted_with_a_permission_input() {
    let extension = Method::from_str("vendor_action").expect("extension");
    let mut server = Server::with_supported_extensions([extension.clone()]).expect("server");
    let request = server
        .parse(
            "event-extension",
            &request_json(
                "request-extension",
                Request::Custom {
                    method: extension.clone(),
                    params: Vec::new(),
                },
            ),
        )
        .expect("extension request");
    assert_eq!(
        request.required_permission(),
        Some(&Permission::new(extension))
    );
}

#[test]
fn server_constructs_correlated_plaintext_for_host_signing() {
    let mut server = Server::new();
    let request = server
        .parse(
            "event-response",
            &request_json("request-response", Request::Ping),
        )
        .expect("request");
    assert_eq!(
        request.request_id(),
        &RequestId::parse("request-response").expect("request id")
    );
    let response = request.respond(Response::Pong).expect("response");
    assert_eq!(
        response.envelope().request_id().expect("response id"),
        RequestId::parse("request-response").expect("request id")
    );
    assert!(response.as_json().contains("\"pong\""));
    assert_eq!(format!("{response:?}"), "ServerResponse(<redacted>)");
}

#[test]
fn server_rejects_fingerprint_and_request_id_replays() {
    let mut server = Server::new();
    let first = request_json("request-replay", Request::Ping);
    server.parse("event-replay", &first).expect("first request");
    assert_eq!(
        server
            .parse(
                "event-replay",
                &request_json("request-other", Request::Ping)
            )
            .expect_err("fingerprint replay"),
        Error::ReplayedRequest
    );
    assert_eq!(
        server
            .parse("event-other", &first)
            .expect_err("request id replay"),
        Error::ReplayedRequest
    );
}
