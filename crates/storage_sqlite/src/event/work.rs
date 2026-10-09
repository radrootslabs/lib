//! Private test observations of actual provider work; no production metric API.
use sqlx::{Row, sqlite::SqliteRow};
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub(super) struct Counts {
    pub bounded_requests: u64,
    pub materialization_requests: u64,
    pub aggregate_requests: u64,
    pub successful_fetches: u64,
    pub event_rows_returned: u64,
    pub event_bytes_returned: u64,
    pub aggregate_rows_returned: u64,
    pub codec_inputs: u64,
    pub codec_input_bytes: u64,
    pub codec_successes: u64,
    pub visibility_inputs: u64,
}

#[derive(Clone, Copy)]
pub(super) enum Read {
    Bounded,
    Materialize,
    Aggregate,
}

#[derive(Default)]
pub(super) struct Observer {
    counts: Mutex<Counts>,
    // Three placeholder-only shapes; bind values and payloads are never retained.
    templates: Mutex<[Option<String>; 3]>,
}

impl Observer {
    pub(super) fn request(&self, kind: Read, sql: &str) {
        let mut counts = self.counts.lock().expect("test observer");
        match kind {
            Read::Bounded => counts.bounded_requests += 1,
            Read::Materialize => counts.materialization_requests += 1,
            Read::Aggregate => counts.aggregate_requests += 1,
        }
        self.templates.lock().expect("test query templates")[kind as usize] = Some(sql.to_owned());
    }

    pub(super) fn returned(&self, rows: &[SqliteRow]) {
        let bytes = rows
            .iter()
            .filter_map(|row| row.try_get::<&[u8], _>("signed_event").ok())
            .map(|value| value.len() as u64)
            .sum::<u64>();
        let mut counts = self.counts.lock().expect("test observer");
        counts.successful_fetches += 1;
        counts.event_rows_returned += rows.len() as u64;
        counts.event_bytes_returned += bytes;
    }

    pub(super) fn aggregate_returned(&self) {
        let mut counts = self.counts.lock().expect("test observer");
        counts.successful_fetches += 1;
        counts.aggregate_rows_returned += 1;
    }

    pub(super) fn decode(&self, bytes: usize) {
        let mut counts = self.counts.lock().expect("test observer");
        counts.codec_inputs += 1;
        counts.codec_input_bytes += bytes as u64;
    }

    pub(super) fn decoded(&self) {
        self.counts.lock().expect("test observer").codec_successes += 1;
    }

    pub(super) fn visibility(&self, rows: usize) {
        self.counts.lock().expect("test observer").visibility_inputs += rows as u64;
    }

    pub(super) fn take(&self) -> Counts {
        std::mem::take(&mut *self.counts.lock().expect("test observer"))
    }

    pub(super) fn template(&self, kind: Read) -> String {
        self.templates.lock().expect("test query templates")[kind as usize]
            .clone()
            .expect("observed request")
    }
}
