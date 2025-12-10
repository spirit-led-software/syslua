//! Error types for sys-core

use thiserror::Error;

/// Errors that can occur in core operations
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Lua evaluation error: {0}")]
    Lua(#[from] sys_lua::LuaError),

    #[error("Platform error: {0}")]
    Platform(#[from] sys_platform::PlatformError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File operation failed for '{path}': {message}")]
    FileOperation { path: String, message: String },

    #[error("Symlink target does not exist: {0}")]
    SymlinkTargetMissing(String),

    #[error("Cannot overwrite existing file without --force: {0}")]
    FileExists(String),

    // Store errors
    #[error("Store not initialized: {0}")]
    StoreNotInitialized(String),

    #[error("Store object not found: {0}")]
    ObjectNotFound(String),

    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    // Derivation errors
    #[error("Derivation build failed for '{name}': {message}")]
    BuildFailed { name: String, message: String },

    #[error("Missing required input '{input}' for derivation '{name}'")]
    MissingInput { name: String, input: String },

    #[error("Invalid derivation spec: {0}")]
    InvalidDerivationSpec(String),

    // Fetch errors
    #[error("Fetch failed for URL '{url}': {message}")]
    FetchFailed { url: String, message: String },

    #[error("Archive extraction failed: {0}")]
    ExtractionFailed(String),

    // Input errors
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    // JSON/serialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    // Snapshot errors
    #[error("Snapshot not found: {0}")]
    SnapshotNotFound(String),

    #[error("No snapshots available")]
    NoSnapshots,

    #[error("Snapshot operation failed: {0}")]
    SnapshotError(String),

    #[error("Rollback failed: {0}")]
    RollbackFailed(String),
}
