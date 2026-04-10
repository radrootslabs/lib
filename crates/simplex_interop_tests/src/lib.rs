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
        RadrootsSimplexSmpCommandAuthorization, RadrootsSimplexSmpQueueAuthorizationMaterial,
        RadrootsSimplexSmpQueueAuthorizationScope, RadrootsSimplexSmpX25519Keypair,
    };
    use radroots_simplex_smp_proto::prelude::{
        RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION, RadrootsSimplexSmpBrokerMessage,
        RadrootsSimplexSmpBrokerTransmission, RadrootsSimplexSmpCommand,
        RadrootsSimplexSmpCommandTransmission, RadrootsSimplexSmpCorrelationId,
        RadrootsSimplexSmpMessageFlags, RadrootsSimplexSmpQueueIdsResponse,
        RadrootsSimplexSmpQueueMode, RadrootsSimplexSmpSendCommand,
    };
    use radroots_simplex_smp_transport::prelude::{
        RadrootsSimplexSmpCommandTransport, RadrootsSimplexSmpTransportBlock,
        RadrootsSimplexSmpTransportRequest, RadrootsSimplexSmpTransportResponse,
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
            link_id: None,
            service_id: None,
            server_notification_credentials: None,
        })
    }

    #[derive(Default)]
    struct ScriptedTransport {
        responses: VecDeque<RadrootsSimplexSmpBrokerMessage>,
    }

    impl ScriptedTransport {
        fn with_responses(responses: Vec<RadrootsSimplexSmpBrokerMessage>) -> Self {
            Self {
                responses: responses.into(),
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
                ciphertext: encoded_decrypted.clone(),
            });
        let decoded_envelope = decode_envelope(&encode_envelope(&envelope).unwrap()).unwrap();
        let RadrootsSimplexAgentEnvelope::Message(payload) = decoded_envelope else {
            panic!("expected message envelope");
        };
        let decoded_decrypted = decode_decrypted_message(&payload.ciphertext).unwrap();
        let RadrootsSimplexAgentDecryptedMessage::Message(decoded_frame_from_envelope) =
            decoded_decrypted
        else {
            panic!("expected message frame");
        };
        let RadrootsSimplexAgentMessage::UserMessage(encoded_chat_again) =
            decoded_frame_from_envelope.message
        else {
            panic!("expected user message");
        };
        assert_eq!(decode_messages(&encoded_chat_again).unwrap(), chat_messages);
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
        let invitation = events
            .into_iter()
            .find_map(|event| match event {
                RadrootsSimplexAgentRuntimeEvent::InvitationReady { invitation, .. } => {
                    Some(invitation)
                }
                _ => None,
            })
            .expect("invitation event");

        let joined = runtime
            .join_connection(invitation, synthetic_reply_queue(), 30)
            .unwrap();
        let mut join_transport = ScriptedTransport::with_responses(vec![
            RadrootsSimplexSmpBrokerMessage::Ok,
            ids_response(b"recipient-2", b"sender-2", b"server-dh-2"),
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Ok,
        ]);
        runtime
            .execute_ready_commands(&mut join_transport, 40, 16)
            .unwrap();
        runtime
            .allow_connection(&joined, b"rr-synth-info".to_vec(), 50)
            .unwrap();
        let message_id = runtime
            .send_message(&joined, b"rr-synth-chat".to_vec(), 60)
            .unwrap();
        assert_eq!(message_id, 1);
        runtime.reconnect_connection(&joined, 70).unwrap();
        assert!(!runtime.retry_pending(70 + 5_000, 64).is_empty());
        assert!(created.starts_with("conn-"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn local_upstream_contract_is_opt_in() {
        let Some(target) = RadrootsSimplexInteropLocalUpstream::from_env() else {
            return;
        };
        target.assert_reachable().unwrap();
    }
}
