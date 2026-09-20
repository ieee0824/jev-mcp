use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

use serde::Serialize;

use crate::error::ErrorKind;

#[derive(Clone)]
pub struct Telemetry {
    writer: Option<Arc<Mutex<Box<dyn Write + Send>>>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CallStatus {
    Success,
    Error,
}

#[derive(Debug, Serialize)]
pub struct CallTelemetry<'a> {
    pub tool: &'a str,
    pub question_count: usize,
    pub elapsed_ms: u64,
    pub attempts: usize,
    pub status: CallStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_model: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_model: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TelemetryError>,
}

#[derive(Debug, Serialize)]
pub struct TelemetryError {
    pub kind: ErrorKind,
}

impl Telemetry {
    pub fn from_env() -> Result<Self, String> {
        match std::env::var("JEV_TELEMETRY") {
            Err(std::env::VarError::NotPresent) => Ok(Self::disabled()),
            Err(std::env::VarError::NotUnicode(_)) => {
                Err("JEV_TELEMETRY must be valid UTF-8".into())
            }
            Ok(value)
                if matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "" | "0" | "false"
                ) =>
            {
                Ok(Self::disabled())
            }
            Ok(value) if matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true") => {
                Ok(Self::with_writer(io::stderr()))
            }
            Ok(_) => Err("JEV_TELEMETRY must be one of: 1, true, 0, false".into()),
        }
    }

    pub fn disabled() -> Self {
        Self { writer: None }
    }

    #[cfg(test)]
    pub fn is_enabled(&self) -> bool {
        self.writer.is_some()
    }

    pub fn with_writer(writer: impl Write + Send + 'static) -> Self {
        Self {
            writer: Some(Arc::new(Mutex::new(Box::new(writer)))),
        }
    }

    pub fn record(&self, event: &CallTelemetry<'_>) {
        let Some(writer) = &self.writer else {
            return;
        };
        let Ok(mut writer) = writer.lock() else {
            return;
        };
        if serde_json::to_writer(&mut *writer, event).is_ok() {
            let _ = writer.write_all(b"\n");
            let _ = writer.flush();
        }
    }
}
