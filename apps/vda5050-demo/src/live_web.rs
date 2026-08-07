use std::{collections::BTreeMap, io::Read, path::Path};

use serde_json::json;
use thiserror::Error;
use vda5050_local_fs::{LocalPathError, TrustedDirectory};

const SECURITY_HEADERS: [(&str, &str); 5] = [
    ("Cache-Control", "no-store"),
    (
        "Content-Security-Policy",
        "default-src 'self'; connect-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self'",
    ),
    ("Referrer-Policy", "no-referrer"),
    ("X-Content-Type-Options", "nosniff"),
    ("X-Frame-Options", "DENY"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebBody {
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebResponse {
    pub status: u16,
    pub content_type: &'static str,
    pub headers: BTreeMap<&'static str, &'static str>,
    pub body: WebBody,
}

#[derive(Debug, Error)]
pub enum LiveWebError {
    #[error("web data directory is unsafe: {0}")]
    Directory(#[from] LocalPathError),
    #[error("web file limit must be greater than zero")]
    ZeroLimit,
}

#[derive(Debug)]
pub struct LiveWebStore {
    artifacts: TrustedDirectory,
    static_files: TrustedDirectory,
    max_file_bytes: u64,
}

impl LiveWebStore {
    /// Binds the gateway to fixed artifact and static directories.
    ///
    /// # Errors
    ///
    /// Returns an error if either directory is unsafe or the file limit is zero.
    pub fn new(
        artifact_dir: &Path,
        static_dir: &Path,
        max_file_bytes: u64,
    ) -> Result<Self, LiveWebError> {
        if max_file_bytes == 0 {
            return Err(LiveWebError::ZeroLimit);
        }
        Ok(Self {
            artifacts: TrustedDirectory::open(artifact_dir)?,
            static_files: TrustedDirectory::open(static_dir)?,
            max_file_bytes,
        })
    }

    #[must_use]
    pub fn resolve(&self, method: &str, path: &str) -> WebResponse {
        if method != "GET" {
            return response(
                405,
                "application/json; charset=utf-8",
                error_json("method_not_allowed"),
            );
        }
        match path {
            "/" => self.static_response("index.html", "text/html; charset=utf-8"),
            "/app.js" => self.static_response("app.js", "text/javascript; charset=utf-8"),
            "/model.js" => self.static_response("model.js", "text/javascript; charset=utf-8"),
            "/styles.css" => self.static_response("styles.css", "text/css; charset=utf-8"),
            "/api/status" => self.status_response(),
            "/api/plan" => self.artifact_response_first(
                &["run-manifest.json", "lsmart-run-manifest.json"],
                "application/json; charset=utf-8",
            ),
            "/api/map" => {
                self.artifact_response("lsmart-map.json", "application/json; charset=utf-8")
            }
            "/api/sim-events" => self.artifact_response_first(
                &["sim-events.jsonl", "lsmart-sim-events.jsonl"],
                "application/x-ndjson; charset=utf-8",
            ),
            "/api/wire-events" => {
                self.artifact_response("wire-events.jsonl", "application/x-ndjson; charset=utf-8")
            }
            "/api/report/passive" => {
                self.artifact_response("doctor-passive.json", "application/json; charset=utf-8")
            }
            "/api/report/evidence" => {
                self.artifact_response("doctor-evidence.json", "application/json; charset=utf-8")
            }
            _ => response(
                404,
                "application/json; charset=utf-8",
                error_json("not_found"),
            ),
        }
    }

    fn static_response(&self, name: &str, content_type: &'static str) -> WebResponse {
        file_response(&self.static_files, name, content_type, self.max_file_bytes)
    }

    fn artifact_response(&self, name: &str, content_type: &'static str) -> WebResponse {
        file_response(&self.artifacts, name, content_type, self.max_file_bytes)
    }

    fn artifact_response_first(&self, names: &[&str], content_type: &'static str) -> WebResponse {
        for name in names {
            match read_bounded(&self.artifacts, Path::new(name), self.max_file_bytes) {
                Ok(body) => return response(200, content_type, body),
                Err(ReadError::TooLarge) => {
                    return response(
                        413,
                        "application/json; charset=utf-8",
                        error_json("artifact_too_large"),
                    );
                }
                Err(ReadError::Missing) => {}
            }
        }
        response(
            404,
            "application/json; charset=utf-8",
            error_json("artifact_pending"),
        )
    }

    fn status_response(&self) -> WebResponse {
        let available = |name: &str| self.artifacts.open_file(Path::new(name)).is_ok();
        let available_any = |names: &[&str]| names.iter().any(|name| available(name));
        let body = serde_json::to_vec(&json!({
            "synthetic": true,
            "network_role": "READ_ONLY_ARTIFACT_GATEWAY",
            "artifacts": {
                "plan": available_any(&["run-manifest.json", "lsmart-run-manifest.json"]),
                "map": available("lsmart-map.json"),
                "sim_events": available_any(&["sim-events.jsonl", "lsmart-sim-events.jsonl"]),
                "wire_events": available("wire-events.jsonl"),
                "passive_report": available("doctor-passive.json"),
                "evidence_report": available("doctor-evidence.json")
            }
        }))
        .expect("fixed status JSON is serializable");
        response(200, "application/json; charset=utf-8", body)
    }
}

fn file_response(
    directory: &TrustedDirectory,
    name: &str,
    content_type: &'static str,
    max_file_bytes: u64,
) -> WebResponse {
    match read_bounded(directory, Path::new(name), max_file_bytes) {
        Ok(body) => response(200, content_type, body),
        Err(ReadError::Missing) => response(
            404,
            "application/json; charset=utf-8",
            error_json("artifact_pending"),
        ),
        Err(ReadError::TooLarge) => response(
            413,
            "application/json; charset=utf-8",
            error_json("artifact_too_large"),
        ),
    }
}

enum ReadError {
    Missing,
    TooLarge,
}

fn read_bounded(
    directory: &TrustedDirectory,
    reference: &Path,
    max_file_bytes: u64,
) -> Result<Vec<u8>, ReadError> {
    let mut file = directory
        .open_file(reference)
        .map_err(|_| ReadError::Missing)?;
    let metadata = file.metadata().map_err(|_| ReadError::Missing)?;
    if metadata.len() > max_file_bytes {
        return Err(ReadError::TooLarge);
    }
    let mut body = Vec::new();
    file.by_ref()
        .take(max_file_bytes.saturating_add(1))
        .read_to_end(&mut body)
        .map_err(|_| ReadError::Missing)?;
    if u64::try_from(body.len()).unwrap_or(u64::MAX) > max_file_bytes {
        Err(ReadError::TooLarge)
    } else {
        Ok(body)
    }
}

fn error_json(code: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"error": code})).expect("fixed error JSON is serializable")
}

fn response(status: u16, content_type: &'static str, body: Vec<u8>) -> WebResponse {
    WebResponse {
        status,
        content_type,
        headers: SECURITY_HEADERS.into_iter().collect(),
        body: WebBody::Bytes(body),
    }
}
