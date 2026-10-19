use super::{Fixture, batch, failure, key};
use futures::FutureExt;
use radroots_storage_indexeddb::{Error, ReadSnapshot, RecordPrefix};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Frozen {
    value: ReadSnapshot,
}
#[wasm_bindgen]
impl Frozen {
    pub fn len(&self) -> u32 {
        self.value.len() as u32
    }
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
    pub fn redacted(&self) -> bool {
        format!("{:?}", self.value) == "ReadSnapshot(<redacted>)"
    }
    pub fn verify(&self, n: u8, actual: Option<Vec<u8>>) -> Result<(), JsValue> {
        self.value
            .verify(key(n), actual.as_deref())
            .map_err(failure)
    }
}
#[wasm_bindgen]
impl Fixture {
    pub async fn snapshot(&self, prefix: u8) -> Result<Frozen, JsValue> {
        self.db
            .read_snapshot(RecordPrefix::new([prefix; 16]))
            .await
            .map(|value| Frozen { value })
            .map_err(failure)
    }
    pub async fn guarded_write(
        &self,
        frozen: Frozen,
        n: u8,
        old: Option<Vec<u8>>,
        new: Option<Vec<u8>>,
    ) -> Result<usize, JsValue> {
        self.db
            .compare_exchange_snapshot_checked(batch(n, old, new)?, frozen.value, || true)
            .await
            .map(|commit| commit.records())
            .map_err(failure)
    }
    pub async fn guarded_refusal(
        &self,
        frozen: Frozen,
        n: u8,
        old: Option<Vec<u8>>,
        new: Option<Vec<u8>>,
    ) -> Result<(), JsValue> {
        self.db
            .compare_exchange_snapshot_checked(batch(n, old, new)?, frozen.value, || false)
            .await
            .map(|_| ())
            .map_err(failure)
    }
    pub fn abandon_snapshots(&self, prefix: u8, count: u32) -> Result<u32, JsValue> {
        for n in 0..count {
            if let Some(result) = self
                .db
                .read_snapshot(RecordPrefix::new([prefix; 16]))
                .now_or_never()
            {
                return if matches!(result, Err(Error::Capacity)) {
                    Ok(n)
                } else {
                    Err(JsValue::from_str("unexpected_immediate_snapshot"))
                };
            }
        }
        Ok(count)
    }
}
