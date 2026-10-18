//! Append-only protected transport attempts on the existing workspace owner.

mod codec;
pub(super) mod graph;
mod json;
pub(super) mod observation;
mod observation_operations;
mod operations;

pub(super) const TABLE: &str = "CREATE TABLE chat_delivery_attempts (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, recipient BLOB NOT NULL CHECK(length(recipient)=32), request_key BLOB NOT NULL CHECK(length(request_key)=32), event_id BLOB NOT NULL CHECK(length(event_id)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), part_count INTEGER NOT NULL CHECK(part_count BETWEEN 1 AND 24), byte_count INTEGER NOT NULL CHECK(byte_count BETWEEN 1 AND 393216), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id,recipient,request_key), FOREIGN KEY(workspace,operation_id,recipient) REFERENCES chat_publication_work(workspace,operation_id,recipient)) STRICT, WITHOUT ROWID";
pub(super) const PARTS: &str = "CREATE TABLE chat_delivery_parts (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, recipient BLOB NOT NULL, request_key BLOB NOT NULL, part INTEGER NOT NULL CHECK(part BETWEEN 1 AND 24), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id,recipient,request_key,part), FOREIGN KEY(workspace,operation_id,recipient,request_key) REFERENCES chat_delivery_attempts(workspace,operation_id,recipient,request_key)) STRICT, WITHOUT ROWID";
pub(super) const TX_INDEX: &str = "CREATE UNIQUE INDEX chat_delivery_transaction_identity ON chat_delivery_attempts(workspace,transaction_id)";

const MAX_BYTES: usize = 393216;
const MAX_PROTECTED_BYTES: usize = 2 * 1024 * 1024;
const MAX_ATTEMPTS: usize = 2 * 16 * 12;
pub(super) const OBSERVATIONS: &str = observation::TABLE;
