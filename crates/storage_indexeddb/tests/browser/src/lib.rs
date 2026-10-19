use futures::FutureExt;
use radroots_storage::event::SourceGeneration;
use radroots_storage_indexeddb::{
    Batch, Database, Error, Mutation, Namespace, OpenMode, RecordKey,
};
use wasm_bindgen::prelude::*;

mod protected;
pub use protected::protected_round_trip;
mod persistent;
pub use persistent::Persistent;
mod workspace;
pub use workspace::{ObtainedEnvelope, ObtainedObservation, ObtainedSeal, WorkspaceFixture};
mod snapshot;
pub use snapshot::Frozen;

fn failure(error: Error) -> JsValue {
    JsValue::from_str(error.code())
}
fn key(n: u8) -> RecordKey {
    RecordKey::new([n; 32])
}
fn batch(n: u8, old: Option<Vec<u8>>, new: Option<Vec<u8>>) -> Result<Batch, JsValue> {
    Batch::new(vec![Mutation::new(key(n), old, new).map_err(failure)?]).map_err(failure)
}
#[wasm_bindgen]
pub struct Fixture {
    db: Database,
}
#[wasm_bindgen]
impl Fixture {
    #[wasm_bindgen(js_name = open)]
    pub async fn open(n: u8, create: bool) -> Result<Fixture, JsValue> {
        let namespace = Namespace::new(
            SourceGeneration::new([n; 32]).map_err(|_| JsValue::from_str("invalid_generation"))?,
        );
        Ok(Self {
            db: Database::open(
                namespace,
                if create {
                    OpenMode::CreateNew
                } else {
                    OpenMode::Existing
                },
            )
            .await
            .map_err(failure)?,
        })
    }
    pub async fn write(
        &self,
        n: u8,
        old: Option<Vec<u8>>,
        new: Option<Vec<u8>>,
    ) -> Result<usize, JsValue> {
        Ok(self
            .db
            .compare_exchange(batch(n, old, new)?)
            .await
            .map_err(failure)?
            .records())
    }
    pub async fn read(&self, n: u8) -> Result<JsValue, JsValue> {
        Ok(self
            .db
            .get(key(n))
            .await
            .map_err(failure)?
            .map(|bytes| js_sys::Uint8Array::from(bytes.as_slice()).into())
            .unwrap_or(JsValue::UNDEFINED))
    }
    pub async fn atomic_pair(&self, second_expected: u8) -> Result<usize, JsValue> {
        let entries = vec![
            Mutation::new(key(1), Some(vec![1]), Some(vec![3])).map_err(failure)?,
            Mutation::new(key(2), Some(vec![second_expected]), Some(vec![4])).map_err(failure)?,
        ];
        Ok(self
            .db
            .compare_exchange(Batch::new(entries).map_err(failure)?)
            .await
            .map_err(failure)?
            .records())
    }
    pub async fn refused_pair(&self) -> Result<(), JsValue> {
        let n = std::cell::Cell::new(0usize);
        let entries = vec![
            Mutation::new(key(1), Some(vec![1]), Some(vec![3])).map_err(failure)?,
            Mutation::new(key(2), Some(vec![2]), Some(vec![4])).map_err(failure)?,
        ];
        self.db
            .compare_exchange_checked(Batch::new(entries).map_err(failure)?, move || {
                n.set(n.get() + 1);
                n.get() == 1
            })
            .await
            .map(|_| ())
            .map_err(failure)
    }
    pub fn abandon_write(&self, n: u8) -> Result<(), JsValue> {
        let result = self
            .db
            .compare_exchange(batch(n, None, Some(vec![n]))?)
            .now_or_never();
        if result.is_some() {
            return Err(JsValue::from_str("unexpected_immediate_completion"));
        }
        Ok(())
    }
    pub fn abandon_reads(&self, count: usize) -> Result<usize, JsValue> {
        for index in 0..count {
            if let Some(result) = self.db.get(key(1)).now_or_never() {
                return match result {
                    Err(Error::Capacity) => Ok(index),
                    _ => Err(JsValue::from_str("unexpected_admission_result")),
                };
            }
        }
        Ok(count)
    }
    pub fn abandon_close(&self) -> Result<(), JsValue> {
        if self.db.close().now_or_never().is_some() {
            return Err(JsValue::from_str("unexpected_immediate_close"));
        }
        Ok(())
    }
    pub async fn close(&self) -> Result<(), JsValue> {
        self.db.close().await.map_err(failure)
    }
}
