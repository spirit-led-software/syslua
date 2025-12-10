//! sys-lua: Lua configuration evaluation for sys.lua
//!
//! This crate provides the Lua runtime and API for evaluating sys.lua configurations.

mod error;
mod eval;
mod globals;
mod types;

pub use error::LuaError;
pub use eval::{EvalContext, evaluate_config, evaluate_config_with_inputs};
pub use types::{
    // Core primitives
    ActivateAction, ActivateDecl, ActivateInput, DeriveDecl, DeriveInput, DeriveRef,
    // Higher-level declarations
    EnvDecl, EnvMergeStrategy, EnvValue, FileDecl, InputDecl,
};

/// Result type for Lua operations
pub type Result<T> = std::result::Result<T, LuaError>;
