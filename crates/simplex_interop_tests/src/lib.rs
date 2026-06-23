#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![doc = r#"
`radroots_simplex_interop_tests` owns the synthetic fixture policy for the rr-rs
SimpleX stack.

Rules:
- committed fixtures must use the `rr-synth/*` namespace.
- committed server hosts must stay in obviously synthetic domains such as
  `.invalid`, `.example`, or `.test`.
- committed tests must not copy or derive realistic queue URIs, certificates,
  ciphertext, or traffic from `refs/*` or external captures.
- black-box local upstream checks are opt-in through environment variables and
  are never required for the default workspace verify lane.
"#]

extern crate alloc;

pub mod fixtures;
pub mod policy;

#[cfg(test)]
mod tests {
    use crate::fixtures::{
        synthetic_chat_messages, synthetic_connection_id, synthetic_fixture_id,
        synthetic_invitation_queue, synthetic_reply_queue,
    };
    use crate::policy::{RadrootsSimplexInteropFixturePolicy, RadrootsSimplexInteropLocalUpstream};
    use alloc::collections::VecDeque;
    use radroots_simplex_agent_proto::prelude::{
        RadrootsSimplexAgentDecryptedMessage, RadrootsSimplexAgentEncryptedPayload,
        RadrootsSimplexAgentEnvelope, RadrootsSimplexAgentMessage,
        RadrootsSimplexAgentMessageFrame, RadrootsSimplexAgentMessageHeader,
        decode_agent_message_frame, decode_decrypted_message, decode_envelope,
        encode_agent_message_frame, encode_decrypted_message, encode_envelope,
    };
    use radroots_simplex_agent_runtime::prelude::{
        RadrootsSimplexAgentRuntime, RadrootsSimplexAgentRuntimeBuilder,
        RadrootsSimplexAgentRuntimeEvent,
    };
    use radroots_simplex_chat_proto::prelude::{decode_messages, encode_compressed_batch};
    use radroots_simplex_smp_crypto::prelude::{
        RadrootsSimplexSmpCommandAuthorization, RadrootsSimplexSmpEd25519Keypair,
        RadrootsSimplexSmpQueueAuthorizationMaterial, RadrootsSimplexSmpQueueAuthorizationScope,
        RadrootsSimplexSmpX25519Keypair, encode_ed25519_public_key_x509,
        encode_x25519_public_key_x509,
    };
    use radroots_simplex_smp_proto::prelude::{
        RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION, RadrootsSimplexSmpBrokerMessage,
        RadrootsSimplexSmpBrokerTransmission, RadrootsSimplexSmpCommand,
        RadrootsSimplexSmpCommandTransmission, RadrootsSimplexSmpCorrelationId,
        RadrootsSimplexSmpMessageFlags, RadrootsSimplexSmpNewQueueRequest,
        RadrootsSimplexSmpQueueIdsResponse, RadrootsSimplexSmpQueueMode,
        RadrootsSimplexSmpQueueRequestData, RadrootsSimplexSmpSendCommand,
        RadrootsSimplexSmpServerAddress, RadrootsSimplexSmpSubscriptionMode,
    };
    use radroots_simplex_smp_transport::prelude::{
        RadrootsSimplexSmpCommandTransport, RadrootsSimplexSmpSubscriptionReceiveRequest,
        RadrootsSimplexSmpSubscriptionTransport, RadrootsSimplexSmpTlsCommandTransport,
        RadrootsSimplexSmpTransportBlock, RadrootsSimplexSmpTransportRequest,
        RadrootsSimplexSmpTransportResponse,
    };

    fn ids_response(
        recipient_id: &[u8],
        sender_id: &[u8],
        seed: &[u8],
    ) -> RadrootsSimplexSmpBrokerMessage {
        RadrootsSimplexSmpBrokerMessage::Ids(RadrootsSimplexSmpQueueIdsResponse {
            recipient_id: recipient_id.to_vec(),
            sender_id: sender_id.to_vec(),
            server_dh_public_key: RadrootsSimplexSmpX25519Keypair::from_seed(seed).public_key,
            queue_mode: Some(RadrootsSimplexSmpQueueMode::Messaging),
            link_id: Some(synthetic_link_id(seed)),
            service_id: None,
            server_notification_credentials: None,
        })
    }

    fn synthetic_link_id(seed: &[u8]) -> Vec<u8> {
        let mut link_id = vec![0_u8; 24];
        for (index, byte) in seed.iter().enumerate() {
            link_id[index % 24] ^= *byte;
            link_id[(index * 7 + 3) % 24] = link_id[(index * 7 + 3) % 24].wrapping_add(*byte);
        }
        link_id
    }

    fn correlation_id(byte: u8) -> RadrootsSimplexSmpCorrelationId {
        RadrootsSimplexSmpCorrelationId::new([byte; RadrootsSimplexSmpCorrelationId::LENGTH])
    }

    fn live_transport_request(
        server: RadrootsSimplexSmpServerAddress,
        correlation_id: RadrootsSimplexSmpCorrelationId,
        entity_id: Vec<u8>,
        command: RadrootsSimplexSmpCommand,
        authorization: RadrootsSimplexSmpCommandAuthorization,
    ) -> RadrootsSimplexSmpTransportRequest {
        RadrootsSimplexSmpTransportRequest {
            server,
            transport_version: RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
            correlation_id: Some(correlation_id),
            entity_id,
            command,
            authorization,
        }
    }

    #[cfg(feature = "std")]
    fn local_upstream_target() -> Option<RadrootsSimplexInteropLocalUpstream> {
        RadrootsSimplexInteropLocalUpstream::required_from_env().unwrap()
    }

    #[derive(Default)]
    struct ScriptedTransport {
        responses: VecDeque<RadrootsSimplexSmpBrokerMessage>,
        requests: Vec<RadrootsSimplexSmpTransportRequest>,
    }

    impl ScriptedTransport {
        fn with_responses(responses: Vec<RadrootsSimplexSmpBrokerMessage>) -> Self {
            Self {
                responses: responses.into(),
                requests: Vec::new(),
            }
        }
    }

    impl RadrootsSimplexSmpCommandTransport for ScriptedTransport {
        type Error = String;

        fn execute(
            &mut self,
            request: RadrootsSimplexSmpTransportRequest,
        ) -> Result<RadrootsSimplexSmpTransportResponse, Self::Error> {
            let correlation_id = request
                .correlation_id
                .ok_or_else(|| "missing scripted transport correlation id".to_owned())?;
            let scope = RadrootsSimplexSmpQueueAuthorizationScope::new(
                b"scripted-session".to_vec(),
                correlation_id,
                request.entity_id.clone(),
            )
            .map_err(|error| error.to_string())?;
            let material = RadrootsSimplexSmpQueueAuthorizationMaterial::for_command(
                &scope,
                &request.command,
                request.transport_version,
                &request.authorization,
            )
            .map_err(|error| error.to_string())?;
            let transmission = RadrootsSimplexSmpCommandTransmission {
                authorization: material.authorization,
                correlation_id: Some(correlation_id),
                entity_id: request.entity_id.clone(),
                command: request.command.clone(),
            };
            let block = RadrootsSimplexSmpTransportBlock::from_current_command_transmissions(&[
                transmission.clone(),
            ])
            .map_err(|error| error.to_string())?;
            let encoded = block.encode().map_err(|error| error.to_string())?;
            let decoded = RadrootsSimplexSmpTransportBlock::decode(&encoded)
                .map_err(|error| error.to_string())?;
            let decoded_transmissions = decoded
                .decode_command_transmissions(request.transport_version)
                .map_err(|error| error.to_string())?;
            assert_eq!(decoded_transmissions, vec![transmission.clone()]);

            let response_message = self
                .responses
                .pop_front()
                .ok_or_else(|| "missing scripted transport response".to_owned())?;
            let response_transmission = RadrootsSimplexSmpBrokerTransmission {
                authorization: Vec::new(),
                correlation_id: Some(correlation_id),
                entity_id: request.entity_id.clone(),
                message: response_message,
            };
            let response_block = RadrootsSimplexSmpTransportBlock::from_broker_transmissions(
                &[response_transmission.clone()],
                request.transport_version,
            )
            .map_err(|error| error.to_string())?;
            let response_encoded = response_block.encode().map_err(|error| error.to_string())?;
            self.requests.push(request.clone());
            Ok(RadrootsSimplexSmpTransportResponse {
                server: request.server,
                transport_version: request.transport_version,
                transmission: response_transmission,
                transport_hash: response_encoded,
            })
        }
    }

    #[test]
    fn synthetic_policy_accepts_only_rr_owned_fixtures() {
        let policy = RadrootsSimplexInteropFixturePolicy::default();
        policy.assert_fixture_id(synthetic_fixture_id()).unwrap();
        policy
            .assert_queue_uri(&synthetic_invitation_queue())
            .unwrap();
        policy.assert_queue_uri(&synthetic_reply_queue()).unwrap();

        let error = policy.assert_fixture_id("copied-from-refs");
        assert!(error.is_err());
    }

    #[test]
    fn synthetic_stack_roundtrip_exercises_smp_agent_and_chat_layers() {
        let correlation_id = RadrootsSimplexSmpCorrelationId::new([7_u8; 24]);
        let send_command = RadrootsSimplexSmpCommand::Send(RadrootsSimplexSmpSendCommand {
            flags: RadrootsSimplexSmpMessageFlags::notifications_enabled(),
            message_body: b"rr-synth-body".to_vec(),
        });
        let transmission = RadrootsSimplexSmpCommandTransmission {
            authorization: b"rr-synth-auth".to_vec(),
            correlation_id: Some(correlation_id),
            entity_id: b"rr-synth-queue".to_vec(),
            command: send_command.clone(),
        };
        let block = RadrootsSimplexSmpTransportBlock::from_current_command_transmissions(&[
            transmission.clone(),
        ])
        .unwrap();
        let decoded = RadrootsSimplexSmpTransportBlock::decode(&block.encode().unwrap())
            .unwrap()
            .decode_command_transmissions(RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION)
            .unwrap();
        assert_eq!(decoded, vec![transmission]);

        let scope = RadrootsSimplexSmpQueueAuthorizationScope::new(
            b"rr-synth-session".to_vec(),
            correlation_id,
            b"rr-synth-queue".to_vec(),
        )
        .unwrap();
        let auth = RadrootsSimplexSmpQueueAuthorizationMaterial::for_command(
            &scope,
            &send_command,
            RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
            &RadrootsSimplexSmpCommandAuthorization::None,
        )
        .unwrap();
        assert_eq!(auth.authorized_body[0], b"rr-synth-session".len() as u8);
        assert!(auth.authorization.is_empty());

        let chat_messages = synthetic_chat_messages();
        let compressed_chat = encode_compressed_batch(&chat_messages).unwrap();
        let decoded_chat = decode_messages(&compressed_chat).unwrap();
        assert_eq!(decoded_chat, chat_messages);

        let frame = RadrootsSimplexAgentMessageFrame {
            header: RadrootsSimplexAgentMessageHeader {
                message_id: 1,
                previous_message_hash: synthetic_connection_id().as_bytes().to_vec(),
            },
            message: RadrootsSimplexAgentMessage::UserMessage(compressed_chat.clone()),
            padding: Vec::new(),
        };
        let encoded_frame = encode_agent_message_frame(&frame).unwrap();
        let decoded_frame = decode_agent_message_frame(&encoded_frame).unwrap();
        assert_eq!(decoded_frame.header, frame.header);
        assert_eq!(decoded_frame.message, frame.message);

        let decrypted = RadrootsSimplexAgentDecryptedMessage::Message(frame.clone());
        let encoded_decrypted = encode_decrypted_message(&decrypted).unwrap();
        let envelope =
            RadrootsSimplexAgentEnvelope::Message(RadrootsSimplexAgentEncryptedPayload {
                ratchet_header: None,
                official_message: None,
                ciphertext: b"opaque-agent-ciphertext".to_vec(),
            });
        let decoded_envelope = decode_envelope(&encode_envelope(&envelope).unwrap()).unwrap();
        let RadrootsSimplexAgentEnvelope::Message(payload) = decoded_envelope else {
            panic!("expected message envelope");
        };
        assert_eq!(payload.ciphertext, b"opaque-agent-ciphertext".to_vec());
        let decoded_decrypted = decode_decrypted_message(&encoded_decrypted).unwrap();
        assert_eq!(decoded_decrypted, decrypted);
    }

    #[test]
    fn synthetic_runtime_flow_stays_fixture_owned() {
        let mut runtime: RadrootsSimplexAgentRuntime =
            RadrootsSimplexAgentRuntimeBuilder::new().build().unwrap();
        let created = runtime
            .create_connection(
                synthetic_invitation_queue(),
                b"rr-synth-e2e".to_vec(),
                false,
                10,
            )
            .unwrap();
        let mut invitation_transport = ScriptedTransport::with_responses(vec![ids_response(
            b"recipient",
            b"sender",
            b"server-dh",
        )]);
        runtime
            .execute_ready_commands(&mut invitation_transport, 20, 16)
            .unwrap();
        let events = runtime.drain_events(8);
        let short_invitation = events
            .into_iter()
            .find_map(|event| match event {
                RadrootsSimplexAgentRuntimeEvent::InvitationReady { invitation, .. } => {
                    Some(invitation)
                }
                _ => None,
            })
            .expect("invitation event");
        assert!(
            short_invitation
                .render()
                .unwrap()
                .starts_with("simplex:/i#")
        );
        let RadrootsSimplexSmpCommand::New(create_request) =
            &invitation_transport.requests[0].command
        else {
            panic!("first synthetic runtime command should create the invite queue");
        };
        assert!(matches!(
            create_request.queue_request_data.as_ref(),
            Some(RadrootsSimplexSmpQueueRequestData::Messaging(Some(_)))
        ));
        assert!(created.starts_with("conn-"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn local_upstream_contract_is_opt_in() {
        let Some(target) = local_upstream_target() else {
            return;
        };
        target.assert_reachable().unwrap();
    }

    #[cfg(feature = "std")]
    #[test]
    fn required_local_upstream_contract_is_enforced() {
        let Some(target) = local_upstream_target() else {
            return;
        };
        target.assert_reachable().unwrap();
        assert!(target.server_address().is_some());
    }

    #[cfg(feature = "std")]
    #[test]
    fn local_upstream_ping_round_trips_when_configured() {
        let Some(target) = local_upstream_target() else {
            return;
        };
        target.assert_reachable().unwrap();
        let Some(server) = target.server_address() else {
            return;
        };

        let response = RadrootsSimplexSmpTlsCommandTransport::new()
            .execute(live_transport_request(
                server,
                correlation_id(1),
                Vec::new(),
                RadrootsSimplexSmpCommand::Ping,
                RadrootsSimplexSmpCommandAuthorization::None,
            ))
            .unwrap();
        assert!(matches!(
            response.transmission.message,
            RadrootsSimplexSmpBrokerMessage::Pong
        ));
    }

    #[cfg(feature = "std")]
    #[test]
    fn local_upstream_create_subscribe_send_receive_ack_and_resubscribe_when_configured() {
        let Some(target) = local_upstream_target() else {
            return;
        };
        target.assert_reachable().unwrap();
        let Some(server) = target.server_address() else {
            return;
        };

        let recipient_auth = RadrootsSimplexSmpEd25519Keypair::generate().unwrap();
        let recipient_dh = RadrootsSimplexSmpX25519Keypair::generate().unwrap();
        let mut recipient_transport = RadrootsSimplexSmpTlsCommandTransport::new();
        let create_response = recipient_transport
            .execute(live_transport_request(
                server.clone(),
                correlation_id(1),
                Vec::new(),
                RadrootsSimplexSmpCommand::New(RadrootsSimplexSmpNewQueueRequest {
                    recipient_auth_public_key: encode_ed25519_public_key_x509(
                        &recipient_auth.public_key,
                    )
                    .unwrap(),
                    recipient_dh_public_key: encode_x25519_public_key_x509(
                        &recipient_dh.public_key,
                    )
                    .unwrap(),
                    basic_auth: None,
                    subscription_mode: RadrootsSimplexSmpSubscriptionMode::OnlyCreate,
                    queue_request_data: Some(RadrootsSimplexSmpQueueRequestData::Messaging(None)),
                    notifier_credentials: None,
                }),
                RadrootsSimplexSmpCommandAuthorization::Ed25519(recipient_auth.clone()),
            ))
            .unwrap();
        let RadrootsSimplexSmpBrokerMessage::Ids(ids) = create_response.transmission.message else {
            panic!("expected IDS response from live SMP queue creation");
        };

        let subscribe_response = recipient_transport
            .execute(live_transport_request(
                server.clone(),
                correlation_id(2),
                ids.recipient_id.clone(),
                RadrootsSimplexSmpCommand::Sub,
                RadrootsSimplexSmpCommandAuthorization::Ed25519(recipient_auth.clone()),
            ))
            .unwrap();
        match subscribe_response.transmission.message {
            RadrootsSimplexSmpBrokerMessage::Ok
            | RadrootsSimplexSmpBrokerMessage::Sok(_)
            | RadrootsSimplexSmpBrokerMessage::Msg(_) => {}
            other => panic!("expected live SMP subscription readiness response, got {other:?}"),
        }

        let mut sender_transport = RadrootsSimplexSmpTlsCommandTransport::new();
        let send_response = sender_transport
            .execute(live_transport_request(
                server.clone(),
                correlation_id(3),
                ids.sender_id.clone(),
                RadrootsSimplexSmpCommand::Send(RadrootsSimplexSmpSendCommand {
                    flags: RadrootsSimplexSmpMessageFlags::notifications_enabled(),
                    message_body: b"rr-synth-live-subscribe-message".to_vec(),
                }),
                RadrootsSimplexSmpCommandAuthorization::None,
            ))
            .unwrap();
        assert!(matches!(
            send_response.transmission.message,
            RadrootsSimplexSmpBrokerMessage::Ok
        ));

        let subscription_response = recipient_transport
            .receive_subscription(RadrootsSimplexSmpSubscriptionReceiveRequest {
                server: server.clone(),
            })
            .unwrap()
            .expect("expected live SMP subscription message");
        let RadrootsSimplexSmpBrokerMessage::Msg(message) =
            subscription_response.transmission.message
        else {
            panic!("expected MSG response from live SMP subscription");
        };
        assert!(!message.message_id.is_empty());
        assert!(!message.encrypted_body.is_empty());

        let ack_response = recipient_transport
            .execute(live_transport_request(
                server.clone(),
                correlation_id(4),
                ids.recipient_id.clone(),
                RadrootsSimplexSmpCommand::Ack(message.message_id),
                RadrootsSimplexSmpCommandAuthorization::Ed25519(recipient_auth.clone()),
            ))
            .unwrap();
        match ack_response.transmission.message {
            RadrootsSimplexSmpBrokerMessage::Ok => {}
            other => panic!("expected live SMP ACK response, got {other:?}"),
        }

        let mut reconnect_transport = RadrootsSimplexSmpTlsCommandTransport::new();
        let resubscribe_response = reconnect_transport
            .execute(live_transport_request(
                server,
                correlation_id(5),
                ids.recipient_id,
                RadrootsSimplexSmpCommand::Sub,
                RadrootsSimplexSmpCommandAuthorization::Ed25519(recipient_auth),
            ))
            .unwrap();
        match resubscribe_response.transmission.message {
            RadrootsSimplexSmpBrokerMessage::Ok
            | RadrootsSimplexSmpBrokerMessage::Sok(_)
            | RadrootsSimplexSmpBrokerMessage::Msg(_) => {}
            other => panic!("expected live SMP resubscription readiness response, got {other:?}"),
        }
    }
}
