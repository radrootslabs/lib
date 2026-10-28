use core::{fmt, marker::PhantomData};
use serde::{
    Deserialize, Deserializer,
    de::{self, SeqAccess, Visitor},
};
use serde_json::value::RawValue;

// Allocation-free structural preflight, followed by the actual closed serde
// codec. Fixed identifier arrays contain 32 bytes; descriptor lists use the
// stricter sixteen-element visitor below.
pub(in crate::workspace) fn bounded(raw: &[u8], array_limit: usize) -> bool {
    let mut stack = [(0u8, false, 0usize); 16];
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for byte in raw.iter().copied() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
            continue;
        }
        if byte.is_ascii_whitespace() || byte == b':' {
            continue;
        }
        if byte == b',' {
            if depth > 0 && stack[depth - 1].0 == b'[' {
                stack[depth - 1].1 = true;
            }
            continue;
        }
        if matches!(byte, b'}' | b']') {
            if depth == 0 {
                return false;
            }
            depth -= 1;
            if stack[depth].0 != if byte == b'}' { b'{' } else { b'[' } {
                return false;
            }
            continue;
        }
        if depth > 0 && stack[depth - 1].0 == b'[' && stack[depth - 1].1 {
            stack[depth - 1].2 += 1;
            stack[depth - 1].1 = false;
            if stack[depth - 1].2 > array_limit {
                return false;
            }
        }
        if matches!(byte, b'{' | b'[') {
            if depth == stack.len() {
                return false;
            }
            stack[depth] = (byte, byte == b'[', 0);
            depth += 1;
        } else if byte == b'"' {
            quoted = true;
        }
    }
    !quoted && !escaped && depth == 0
}

pub(in crate::workspace) fn small_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Items<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for Items<T> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("at most sixteen bounded entries")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
            let mut result = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(16));
            while let Some(raw) = sequence.next_element::<&'de RawValue>()? {
                if result.len() == 16 || raw.get().len() > 16384 {
                    return Err(de::Error::custom("private delivery entry limit"));
                }
                result.push(serde_json::from_str(raw.get()).map_err(de::Error::custom)?);
            }
            Ok(result)
        }
    }
    deserializer.deserialize_seq(Items(PhantomData))
}

pub(in crate::workspace) fn targets<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<radroots_transport::TargetSet, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Wire {
        #[serde(deserialize_with = "small_vec")]
        targets: Vec<radroots_transport::Target>,
    }
    let wire = Wire::deserialize(deserializer)?;
    radroots_transport::TargetSet::new(wire.targets).map_err(de::Error::custom)
}

pub(in crate::workspace) fn policy<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<radroots_transport::policy::SatisfactionPolicy, D::Error> {
    let raw = <&RawValue>::deserialize(deserializer)?;
    if raw.get().len() > 16384 || !bounded(raw.get().as_bytes(), 16) {
        return Err(de::Error::custom("private delivery policy limit"));
    }
    serde_json::from_str(raw.get()).map_err(de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn structural_preflight_and_typed_lists_bound_untrusted_private_records() {
        for raw in [
            br#"{"a":["escaped\\\"[]",null,{}]}"#.as_slice(),
            b"[]",
            b"{}",
            br#"{"a":1,"b":2}"#,
            b"[ [1], [2] ]",
        ] {
            assert!(bounded(raw, 3));
        }
        for raw in [
            b"]".as_slice(),
            b"[}",
            b"\"unfinished",
            b"\"escape\\",
            b"[1,2,3]",
            b"[[1]",
        ] {
            assert!(!bounded(raw, 2));
        }
        assert!(!bounded(
            format!("{}0{}", "[".repeat(17), "]".repeat(17)).as_bytes(),
            32
        ));
        #[derive(Deserialize)]
        struct List {
            #[serde(deserialize_with = "small_vec")]
            items: Vec<String>,
        }
        let accepted: List = serde_json::from_str(r#"{"items":["one","two"]}"#).unwrap();
        assert_eq!(accepted.items, ["one", "two"]);
        for value in [
            serde_json::json!({"items":vec!["item";17]}),
            serde_json::json!({"items":["x".repeat(16385)]}),
            serde_json::json!({"items":false}),
        ] {
            assert!(serde_json::from_str::<List>(&value.to_string()).is_err());
        }
        #[derive(Deserialize)]
        struct Policy {
            #[serde(deserialize_with = "policy")]
            value: radroots_transport::policy::SatisfactionPolicy,
        }
        let raw = serde_json::json!({"value":{"class":"accepted","targets":"all"}}).to_string();
        let decoded = serde_json::from_str::<Policy>(&raw).unwrap();
        assert!(decoded.value.targets().is_all());
        for raw in [
            format!("{{\"value\":\"{}\"}}", "x".repeat(16385)),
            format!("{{\"value\":[{}]}}", vec!["0"; 17].join(",")),
            String::from("{\"value\":false}"),
        ] {
            assert!(serde_json::from_str::<Policy>(&raw).is_err());
        }
    }
}
