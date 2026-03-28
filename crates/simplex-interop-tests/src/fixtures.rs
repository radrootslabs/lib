use alloc::vec::Vec;
use radroots_simplex_chat_proto::prelude::{RadrootsSimplexChatMessage, decode_messages};
use radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpQueueUri;

pub const fn synthetic_fixture_id() -> &'static str {
    "rr-synth/stack/duplex-v1"
}

pub const fn synthetic_connection_id() -> &'static str {
    "rr-synth-conn-001"
}

pub fn synthetic_invitation_queue() -> RadrootsSimplexSmpQueueUri {
    RadrootsSimplexSmpQueueUri::parse(
        "smp://cnItc3ludGg@relay.synthetic.invalid/aW52aXRl#/?v=4&dh=cnItc3ludGgtZGg&q=m",
    )
    .unwrap()
}

pub fn synthetic_reply_queue() -> RadrootsSimplexSmpQueueUri {
    RadrootsSimplexSmpQueueUri::parse(
        "smp://cnItc3ludGg@reply.synthetic.invalid/cmVwbHk#/?v=4&dh=cnItc3ludGgtcmVwbHk&q=m",
    )
    .unwrap()
}

pub fn synthetic_chat_messages() -> Vec<RadrootsSimplexChatMessage> {
    decode_messages(
        br#"[{
            "v":"1-16",
            "msgId":"AQ",
            "event":"x.msg.new",
            "params":{
                "content":{"type":"text","text":"hello from rr-synth"}
            }
        }]"#,
    )
    .unwrap()
}
