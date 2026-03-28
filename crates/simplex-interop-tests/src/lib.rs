#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![doc = r#"
`radroots-simplex-interop-tests` owns the synthetic fixture policy for the rr-rs
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
        RadrootsSimplexSmpQueueAuthorizationMaterial, RadrootsSimplexSmpQueueAuthorizationScope,
    };
    use radroots_simplex_smp_proto::prelude::{
        RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION, RadrootsSimplexSmpCommand,
        RadrootsSimplexSmpCommandTransmission, RadrootsSimplexSmpCorrelationId,
        RadrootsSimplexSmpMessageFlags, RadrootsSimplexSmpSendCommand,
    };
    use radroots_simplex_smp_transport::prelude::RadrootsSimplexSmpTransportBlock;

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
            b"rr-synth-queue-key".to_vec(),
            b"rr-synth-server-key".to_vec(),
        )
        .unwrap();
        assert_eq!(auth.nonce, [7_u8; 24]);

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
            .join_connection(invitation, synthetic_reply_queue(), 20)
            .unwrap();
        runtime
            .allow_connection(&joined, b"rr-synth-info".to_vec(), 30)
            .unwrap();
        let message_id = runtime
            .send_message(&joined, b"rr-synth-chat".to_vec(), 40)
            .unwrap();
        assert_eq!(message_id, 1);
        runtime.reconnect_connection(&joined, 50).unwrap();
        assert!(!runtime.retry_pending(50 + 5_000, 64).is_empty());
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
