pub const APP_NAME: &str = "syslua";
pub const AUTHOR: &str = "Spirit-Led Software";
pub const TOP_LEVEL_DOMAIN: &str = "com";

/// Length of truncated hash prefixes used as manifest keys and in store paths.
/// 20 hex characters = 80 bits of entropy, providing collision resistance
/// up to ~48 billion items at 0.1% probability.
pub const OBJ_HASH_PREFIX_LEN: usize = 20;
