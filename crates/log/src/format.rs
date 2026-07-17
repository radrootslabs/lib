use std::fmt;

use chrono::{SecondsFormat, Utc};
use serde_json::{Map, Number, Value};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

use crate::LogIdentity;

#[derive(Debug, Clone)]
pub(crate) struct JsonEventFormatter {
    identity: LogIdentity,
}

impl JsonEventFormatter {
    pub(crate) fn new(identity: LogIdentity) -> Self {
        Self { identity }
    }
}

impl<S, N> FormatEvent<S, N> for JsonEventFormatter
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        _context: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let metadata = event.metadata();
        let mut fields = Map::new();
        event.record(&mut JsonVisitor(&mut fields));
        let mut document = Map::new();
        document.insert(
            "timestamp".to_owned(),
            Value::String(Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)),
        );
        document.insert(
            "level".to_owned(),
            Value::String(metadata.level().as_str().to_owned()),
        );
        document.insert(
            "target".to_owned(),
            Value::String(metadata.target().to_owned()),
        );
        document.insert(
            "service".to_owned(),
            Value::String(self.identity.service.clone()),
        );
        document.insert(
            "run_id".to_owned(),
            Value::String(self.identity.run_id.clone()),
        );
        document.insert(
            "environment".to_owned(),
            Value::String(self.identity.environment.clone()),
        );
        document.insert("fields".to_owned(), Value::Object(fields));
        let encoded = serde_json::to_string(&Value::Object(document)).map_err(|_| fmt::Error)?;
        writeln!(writer, "{encoded}")
    }
}

struct JsonVisitor<'map>(&'map mut Map<String, Value>);

impl Visit for JsonVisitor<'_> {
    fn record_f64(&mut self, field: &Field, value: f64) {
        let value = Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(format!("{value:?}")));
        self.0.insert(field.name().to_owned(), value);
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.0
            .insert(field.name().to_owned(), Value::Number(value.into()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0
            .insert(field.name().to_owned(), Value::Number(value.into()));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.0.insert(field.name().to_owned(), Value::Bool(value));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.0
            .insert(field.name().to_owned(), Value::String(value.to_owned()));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.0
            .insert(field.name().to_owned(), Value::String(value.to_string()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.0
            .insert(field.name().to_owned(), Value::String(format!("{value:?}")));
    }
}

#[cfg(test)]
mod tests {
    use super::JsonEventFormatter;
    use crate::LogIdentity;
    use std::io::{self, Write};
    use std::sync::{Arc, Mutex};
    use tracing::info;

    struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for CapturedWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .map_err(|_| io::Error::other("capture lock is poisoned"))?
                .extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn json_events_include_identity_and_typed_fields() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let capture = output.clone();
        let subscriber = tracing_subscriber::fmt()
            .event_format(JsonEventFormatter::new(LogIdentity::new(
                "global_relay",
                "run-1",
                "localhost",
            )))
            .with_writer(move || CapturedWriter(capture.clone()))
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            info!(ready = true, records = 3_u64, "service ready");
        });

        let encoded = output.lock().expect("capture").clone();
        let document: serde_json::Value = serde_json::from_slice(&encoded).expect("JSON log");
        assert_eq!(document["service"], "global_relay");
        assert_eq!(document["run_id"], "run-1");
        assert_eq!(document["environment"], "localhost");
        assert_eq!(document["fields"]["ready"], true);
        assert_eq!(document["fields"]["records"], 3);
        assert!(document["fields"]["message"].is_string());
    }
}
