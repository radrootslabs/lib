use thiserror::Error;
use wasm_bindgen::JsValue;

#[derive(Error, Debug, Clone)]
pub enum SqlWasmError {
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("{0} not found")]
    NotFound(String),
    #[error("serialization error: {0}")]
    SerializationError(String),
    #[error("invalid query: {0}")]
    InvalidQuery(String),
    #[error("internal error")]
    Internal,
}

impl SqlWasmError {
    pub fn code(&self) -> &'static str {
        match self {
            SqlWasmError::InvalidArgument(_) => "ERR_INVALID_ARGUMENT",
            SqlWasmError::NotFound(_) => "ERR_NOT_FOUND",
            SqlWasmError::SerializationError(_) => "ERR_SERIALIZATION",
            SqlWasmError::InvalidQuery(_) => "ERR_INVALID_QUERY",
            SqlWasmError::Internal => "ERR_INTERNAL",
        }
    }

    pub fn to_js_value(self) -> JsValue {
        let o = serde_json::json!({
            "code": self.code(),
            "message": self.to_string()
        });
        JsValue::from_str(&o.to_string())
    }
}
