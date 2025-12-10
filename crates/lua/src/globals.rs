//! Global Lua functions and the syslua table

use crate::types::{
    DerivationDecl, DerivationInput, EnvDecl, EnvMergeStrategy, EnvValue, FileDecl, InputDecl,
    PkgDecl,
};
use mlua::{Lua, Result as LuaResult, Table, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use sys_platform::Platform;

/// Shared state for collecting declarations during Lua evaluation
pub struct Declarations {
    pub files: Vec<FileDecl>,
    pub envs: Vec<EnvDecl>,
    pub derivations: Vec<DerivationDecl>,
    pub pkgs: Vec<PkgDecl>,
    pub inputs: Vec<InputDecl>,
}

impl Default for Declarations {
    fn default() -> Self {
        Self::new()
    }
}

impl Declarations {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            envs: Vec::new(),
            derivations: Vec::new(),
            pkgs: Vec::new(),
            inputs: Vec::new(),
        }
    }
}

/// Set up the syslua global table with platform information
pub fn setup_syslua_global(lua: &Lua, platform: &Platform) -> LuaResult<()> {
    let syslua = lua.create_table()?;

    // Platform information
    syslua.set("platform", platform.platform.as_str())?;
    syslua.set("os", platform.os.as_str())?;
    syslua.set("arch", platform.arch.as_str())?;
    syslua.set("hostname", platform.hostname.as_str())?;
    syslua.set("username", platform.username.as_str())?;

    // Boolean helpers
    syslua.set("is_linux", platform.is_linux())?;
    syslua.set("is_darwin", platform.is_darwin())?;
    syslua.set("is_windows", platform.is_windows())?;

    // Version
    syslua.set("version", env!("CARGO_PKG_VERSION"))?;

    lua.globals().set("syslua", syslua)?;

    Ok(())
}

/// Set up the file{} global function
///
/// ```lua
/// file { path = "~/.gitconfig", source = "./dotfiles/gitconfig" }
/// file { path = "~/.gitconfig", source = "./dotfiles/gitconfig", mutable = true }
/// file { path = "~/.config/init.lua", content = [[require("config")]] }
/// ```
pub fn setup_file_function(
    lua: &Lua,
    declarations: Rc<RefCell<Declarations>>,
    config_dir: PathBuf,
) -> LuaResult<()> {
    let file_fn = lua.create_function(move |_, spec: Table| {
        let path_str: String = spec
            .get::<String>("path")
            .map_err(|_| mlua::Error::runtime("file{} requires 'path' field"))?;

        // Expand ~ in path
        let path = sys_platform::expand_path(&path_str)
            .map_err(|e| mlua::Error::runtime(e.to_string()))?;

        // Get optional fields
        let source: Option<String> = spec.get("source").ok();
        let content: Option<String> = spec.get("content").ok();
        let mutable: bool = spec.get("mutable").unwrap_or(false);
        let mode: Option<u32> = spec.get("mode").ok();

        // Expand paths for source, resolving relative paths against config dir
        let source = source
            .map(|s| sys_platform::expand_path_with_base(&s, &config_dir))
            .transpose()
            .map_err(|e| mlua::Error::runtime(e.to_string()))?;

        let decl = FileDecl {
            path,
            source,
            content,
            mutable,
            mode,
        };

        // Validate the declaration
        decl.validate()
            .map_err(|e| mlua::Error::runtime(e.to_string()))?;

        // Add to declarations
        declarations.borrow_mut().files.push(decl);

        Ok(())
    })?;

    lua.globals().set("file", file_fn)?;

    Ok(())
}

/// Set up the env{} global function
///
/// Usage from Lua:
/// ```lua
/// env {
///     EDITOR = "nvim",              -- simple value (replaces existing)
///     PATH = { "~/.local/bin" },    -- array = prepend to PATH
///     MANPATH = { append = "/usr/share/man" },  -- explicit append
/// }
/// ```
pub fn setup_env_function(lua: &Lua, declarations: Rc<RefCell<Declarations>>) -> LuaResult<()> {
    let env_fn = lua.create_function(move |_, spec: Table| {
        for pair in spec.pairs::<String, Value>() {
            let (name, value) = pair?;

            let env_decl = parse_env_value(&name, value)?;
            declarations.borrow_mut().envs.push(env_decl);
        }

        Ok(())
    })?;

    lua.globals().set("env", env_fn)?;

    Ok(())
}

/// Parse a Lua value into an EnvDecl
fn parse_env_value(name: &str, value: Value) -> Result<EnvDecl, mlua::Error> {
    match value {
        // Simple string value: EDITOR = "nvim"
        Value::String(s) => {
            let value_str = s.to_str()?.to_string();
            // Expand ~ in the value
            let expanded = expand_env_path(&value_str);
            Ok(EnvDecl::new(name, expanded))
        }

        // Array of strings: PATH = { "~/.local/bin", "~/.cargo/bin" }
        // This means prepend these paths
        Value::Table(t) => {
            // Check if it's a table with explicit strategy keys
            // Use raw_get to check for nil explicitly
            let prepend_val: Value = t.get("prepend")?;
            if !matches!(prepend_val, Value::Nil) {
                return parse_strategy_value(name, prepend_val, EnvMergeStrategy::Prepend);
            }

            let append_val: Value = t.get("append")?;
            if !matches!(append_val, Value::Nil) {
                return parse_strategy_value(name, append_val, EnvMergeStrategy::Append);
            }

            // Otherwise treat as array of prepend values
            let mut values = Vec::new();
            for item in t.sequence_values::<String>() {
                let path = item?;
                let expanded = expand_env_path(&path);
                values.push(EnvValue::prepend(expanded));
            }

            if values.is_empty() {
                return Err(mlua::Error::runtime(format!(
                    "env var '{}' has empty array value",
                    name
                )));
            }

            Ok(EnvDecl {
                name: name.to_string(),
                values,
            })
        }

        _ => Err(mlua::Error::runtime(format!(
            "env var '{}' must be a string or table, got {:?}",
            name,
            value.type_name()
        ))),
    }
}

/// Parse a value with an explicit merge strategy
fn parse_strategy_value(
    name: &str,
    value: Value,
    strategy: EnvMergeStrategy,
) -> Result<EnvDecl, mlua::Error> {
    match value {
        Value::String(s) => {
            let value_str = s.to_str()?.to_string();
            let expanded = expand_env_path(&value_str);
            Ok(EnvDecl {
                name: name.to_string(),
                values: vec![EnvValue {
                    value: expanded,
                    strategy,
                }],
            })
        }
        Value::Table(t) => {
            let mut values = Vec::new();
            for item in t.sequence_values::<String>() {
                let path = item?;
                let expanded = expand_env_path(&path);
                values.push(EnvValue {
                    value: expanded,
                    strategy: strategy.clone(),
                });
            }
            Ok(EnvDecl {
                name: name.to_string(),
                values,
            })
        }
        _ => Err(mlua::Error::runtime(format!(
            "env var '{}' strategy value must be a string or array",
            name
        ))),
    }
}

/// Expand ~ in environment variable paths
fn expand_env_path(value: &str) -> String {
    if let Some(stripped) = value.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{}", home.display(), stripped);
        }
    }
    value.to_string()
}

/// Set up the derivation{} global function
///
/// Usage from Lua:
/// ```lua
/// local rg = derivation {
///     name = "ripgrep",
///     version = "15.1.0",
///     inputs = {
///         url = "https://github.com/.../ripgrep.tar.gz",
///         sha256 = "abc123...",
///     },
///     build = function(ctx)
///         local archive = ctx.fetch_url(ctx.inputs.url, ctx.inputs.sha256)
///         ctx.unpack(archive, ctx.out)
///     end,
/// }
/// ```
///
/// Returns a table with the derivation name and a placeholder hash that will be
/// computed when the derivation is actually built.
pub fn setup_derivation_function(
    lua: &Lua,
    declarations: Rc<RefCell<Declarations>>,
) -> LuaResult<()> {
    let derivation_fn = lua.create_function(move |lua, spec: Table| {
        // Required: name
        let name: String = spec
            .get::<String>("name")
            .map_err(|_| mlua::Error::runtime("derivation{} requires 'name' field"))?;

        // Optional: version
        let version: Option<String> = spec.get("version").ok();

        // Optional: outputs (defaults to ["out"])
        let outputs: Vec<String> = spec
            .get::<Table>("outputs")
            .map(|t| {
                t.sequence_values::<String>()
                    .filter_map(|r| r.ok())
                    .collect()
            })
            .unwrap_or_else(|_| vec!["out".to_string()]);

        // Required: inputs (table of key-value pairs)
        let inputs_table: Table = spec
            .get("inputs")
            .map_err(|_| mlua::Error::runtime("derivation{} requires 'inputs' field"))?;

        let inputs = parse_inputs_table(&inputs_table)?;

        // Required: build (function) - we store a hash of the function source
        let build_value: Value = spec
            .get("build")
            .map_err(|_| mlua::Error::runtime("derivation{} requires 'build' field"))?;

        let build_hash = match &build_value {
            Value::Function(f) => {
                // Get function info for hashing
                let info = f.info();
                let source = info.source.unwrap_or_else(|| "unknown".to_string());
                let line = info.line_defined.unwrap_or(0);
                format!("{}:{}", source, line)
            }
            _ => {
                return Err(mlua::Error::runtime(
                    "derivation{} 'build' must be a function",
                ));
            }
        };

        let decl = DerivationDecl {
            name: name.clone(),
            version: version.clone(),
            inputs,
            build_hash,
            outputs,
        };

        // Add to declarations
        declarations.borrow_mut().derivations.push(decl);

        // Return a table representing this derivation (can be passed to pkg())
        let result = lua.create_table()?;
        result.set("name", name.clone())?;
        if let Some(v) = &version {
            result.set("version", v.clone())?;
        }
        result.set("_type", "derivation")?;

        Ok(result)
    })?;

    lua.globals().set("derivation", derivation_fn)?;

    Ok(())
}

/// Parse a Lua table into a BTreeMap of DerivationInput values
fn parse_inputs_table(table: &Table) -> Result<BTreeMap<String, DerivationInput>, mlua::Error> {
    let mut inputs = BTreeMap::new();

    for pair in table.pairs::<String, Value>() {
        let (key, value) = pair?;
        let input = lua_value_to_input(value)?;
        inputs.insert(key, input);
    }

    Ok(inputs)
}

/// Convert a Lua value to a DerivationInput
fn lua_value_to_input(value: Value) -> Result<DerivationInput, mlua::Error> {
    match value {
        Value::String(s) => Ok(DerivationInput::String(s.to_str()?.to_string())),
        Value::Number(n) => Ok(DerivationInput::Number(n)),
        Value::Integer(n) => Ok(DerivationInput::Number(n as f64)),
        Value::Boolean(b) => Ok(DerivationInput::Bool(b)),
        Value::Table(t) => {
            // Check if it's an array (has integer key 1)
            // In Lua, arrays start at index 1
            let has_index_one: bool = t.contains_key(1i64).unwrap_or(false);

            if has_index_one {
                // Treat as array - iterate over sequence values
                let mut array = Vec::new();
                for item in t.sequence_values::<Value>() {
                    array.push(lua_value_to_input(item?)?);
                }
                Ok(DerivationInput::Array(array))
            } else {
                // Treat as table/map
                let mut map = BTreeMap::new();
                for pair in t.pairs::<String, Value>() {
                    let (k, v) = pair?;
                    map.insert(k, lua_value_to_input(v)?);
                }
                Ok(DerivationInput::Table(map))
            }
        }
        Value::Nil => Ok(DerivationInput::String(String::new())),
        _ => Err(mlua::Error::runtime(format!(
            "Unsupported input type: {:?}",
            value.type_name()
        ))),
    }
}

/// Set up the pkg() global function
///
/// Usage from Lua:
/// ```lua
/// local rg = derivation { ... }
/// pkg(rg)  -- Register for PATH
/// ```
pub fn setup_pkg_function(lua: &Lua, declarations: Rc<RefCell<Declarations>>) -> LuaResult<()> {
    let pkg_fn = lua.create_function(move |_, drv: Table| {
        // Verify this is a derivation table
        let type_marker: Option<String> = drv.get("_type").ok();
        if type_marker.as_deref() != Some("derivation") {
            return Err(mlua::Error::runtime(
                "pkg() requires a derivation table (created by derivation{})",
            ));
        }

        let name: String = drv
            .get("name")
            .map_err(|_| mlua::Error::runtime("Invalid derivation table: missing 'name'"))?;

        let decl = PkgDecl::new(name);

        // Add to declarations
        declarations.borrow_mut().pkgs.push(decl);

        Ok(())
    })?;

    lua.globals().set("pkg", pkg_fn)?;

    Ok(())
}

/// Set up the input{} global function
///
/// Usage from Lua:
/// ```lua
/// -- inputs.lua
/// local M = {}
///
/// M.pkgs = input { source = "sys-lua/pkgs" }           -- GitHub: owner/repo (main branch)
/// M.pkgs_v2 = input { source = "sys-lua/pkgs/v2.0.0" } -- GitHub: owner/repo/ref
/// M.local_dev = input { source = "path:./my-packages" }
///
/// return M
///
/// -- init.lua
/// local inputs = require("inputs")
/// pkg(inputs.pkgs.ripgrep)  -- loads ripgrep.lua or ripgrep/init.lua from the input
/// ```
///
/// Input source formats (Nix-like):
/// - GitHub: "owner/repo" (defaults to main) or "owner/repo/ref"
/// - Local: "path:./relative" or "path:/absolute"
///
/// The input{} function:
/// 1. Records the input declaration for later resolution by InputManager
/// 2. Returns a table with __index metatable for lazy module loading
///
/// During evaluation, the input paths are not yet resolved. The returned table
/// stores the input ID and will be resolved before actual require() calls.
/// For now, we create a placeholder that will error if accessed before resolution.
pub fn setup_input_function(
    lua: &Lua,
    declarations: Rc<RefCell<Declarations>>,
    config_dir: PathBuf,
) -> LuaResult<()> {
    // Counter for generating unique input IDs
    let counter = Rc::new(RefCell::new(0u32));
    let config_dir = Rc::new(config_dir);

    let input_fn = lua.create_function(move |lua, spec: Table| {
        // Get required source field
        let source: String = spec
            .get::<String>("source")
            .map_err(|_| mlua::Error::runtime("input{} requires 'source' field"))?;

        // Generate a unique ID for this input
        let mut count = counter.borrow_mut();
        *count += 1;
        let input_id = format!("input_{}", *count);

        // Build the input declaration
        let decl = InputDecl::new(input_id.clone(), source.clone());

        // For path: inputs, resolve immediately relative to config dir
        if let Some(path_str) = source.strip_prefix("path:") {
            let path = PathBuf::from(path_str);
            let resolved = if path.is_absolute() {
                path
            } else {
                config_dir.join(&path)
            };

            // Canonicalize to get absolute path
            let resolved = resolved.canonicalize().map_err(|e| {
                mlua::Error::runtime(format!(
                    "Failed to resolve path input '{}': {}",
                    path_str, e
                ))
            })?;

            let decl = decl.with_resolved_path(resolved.clone());

            // Record the declaration
            declarations.borrow_mut().inputs.push(decl);

            // Create a module loader table for the resolved path
            return create_input_loader(lua, &input_id, &resolved);
        }

        // For GitHub inputs (owner/repo or owner/repo/ref), we can't resolve during evaluation
        // Record the declaration and return a placeholder table
        declarations.borrow_mut().inputs.push(decl);

        // Create a placeholder table that will error on access
        // This will be replaced with a real loader after InputManager resolves it
        create_unresolved_input_placeholder(lua, &input_id, &source)
    })?;

    lua.globals().set("input", input_fn)?;

    Ok(())
}

/// Set up the input{} global function with pre-resolved inputs
///
/// This version is used when inputs have already been resolved (e.g., from a lock file).
/// The `resolved_inputs` map contains source URI -> local path mappings.
///
/// For GitHub inputs that are in the resolved map, the local path is used directly.
/// For inputs not in the map, an error placeholder is returned (same as the non-resolved version).
pub fn setup_input_function_with_resolved(
    lua: &Lua,
    declarations: Rc<RefCell<Declarations>>,
    config_dir: PathBuf,
    resolved_inputs: HashMap<String, PathBuf>,
) -> LuaResult<()> {
    // Counter for generating unique input IDs
    let counter = Rc::new(RefCell::new(0u32));
    let config_dir = Rc::new(config_dir);
    let resolved_inputs = Rc::new(resolved_inputs);

    let input_fn = lua.create_function(move |lua, spec: Table| {
        // Get required source field
        let source: String = spec
            .get::<String>("source")
            .map_err(|_| mlua::Error::runtime("input{} requires 'source' field"))?;

        // Generate a unique ID for this input
        let mut count = counter.borrow_mut();
        *count += 1;
        let input_id = format!("input_{}", *count);

        // Build the input declaration
        let decl = InputDecl::new(input_id.clone(), source.clone());

        // For path: inputs, resolve immediately relative to config dir
        if let Some(path_str) = source.strip_prefix("path:") {
            let path = PathBuf::from(path_str);
            let resolved = if path.is_absolute() {
                path
            } else {
                config_dir.join(&path)
            };

            // Canonicalize to get absolute path
            let resolved = resolved.canonicalize().map_err(|e| {
                mlua::Error::runtime(format!(
                    "Failed to resolve path input '{}': {}",
                    path_str, e
                ))
            })?;

            let decl = decl.with_resolved_path(resolved.clone());

            // Record the declaration
            declarations.borrow_mut().inputs.push(decl);

            // Create a module loader table for the resolved path
            return create_input_loader(lua, &input_id, &resolved);
        }

        // For GitHub inputs, check if we have a resolved path
        if let Some(resolved_path) = resolved_inputs.get(&source) {
            let decl = decl.with_resolved_path(resolved_path.clone());

            // Record the declaration
            declarations.borrow_mut().inputs.push(decl);

            // Create a module loader table for the resolved path
            return create_input_loader(lua, &input_id, resolved_path);
        }

        // Not resolved - record the declaration and return a placeholder
        declarations.borrow_mut().inputs.push(decl);

        // Create a placeholder table that will error on access
        create_unresolved_input_placeholder(lua, &input_id, &source)
    })?;

    lua.globals().set("input", input_fn)?;

    Ok(())
}

/// Create a module loader table for a resolved input path.
///
/// If the input directory has an `init.lua` at its root, that file is loaded
/// and its return value is returned directly. This supports inputs that export
/// a single module table.
///
/// Otherwise, returns a lazy loader table with an __index metamethod that:
/// 1. Takes the key being accessed (e.g., "ripgrep")
/// 2. Attempts to load it as a Lua module from the input directory
/// 3. Returns the loaded module (which can itself be a table with more __index)
fn create_input_loader(lua: &Lua, input_id: &str, base_path: &Path) -> LuaResult<Table> {
    // Check if there's an init.lua at the root - if so, load it directly
    let init_path = base_path.join("init.lua");
    if init_path.exists() {
        let result = load_lua_file(lua, &init_path)?;
        // If the result is a table, return it with metadata
        if let Value::Table(tbl) = result {
            // Add metadata to the loaded module (if it doesn't conflict)
            if tbl.get::<Value>("_type")?.is_nil() {
                tbl.set("_type", "input")?;
            }
            if tbl.get::<Value>("_input_id")?.is_nil() {
                tbl.set("_input_id", input_id.to_string())?;
            }
            return Ok(tbl);
        }
        // If init.lua returns a non-table, wrap it in a table
        let wrapper = lua.create_table()?;
        wrapper.set("_type", "input")?;
        wrapper.set("_input_id", input_id.to_string())?;
        wrapper.set("_value", result)?;
        return Ok(wrapper);
    }

    // No init.lua at root - create a lazy loader for submodules
    let loader = lua.create_table()?;

    // Store metadata
    loader.set("_type", "input")?;
    loader.set("_input_id", input_id.to_string())?;
    loader.set("_base_path", base_path.to_string_lossy().to_string())?;

    // Create metatable with __index
    let metatable = lua.create_table()?;

    let index_fn = lua.create_function(move |lua, (tbl, key): (Table, String)| {
        let base: String = tbl.get("_base_path")?;
        let base_path = PathBuf::from(&base);

        // Try to load the module
        load_module_from_input(lua, &base_path, &key)
    })?;

    metatable.set("__index", index_fn)?;
    loader.set_metatable(Some(metatable))?;

    Ok(loader)
}

/// Load a module from an input directory using standard Lua resolution.
///
/// Tries in order:
/// 1. `<base>/<key>.lua`
/// 2. `<base>/<key>/init.lua`
///
/// Returns the loaded module, which may be a table that also supports __index
/// for nested modules.
fn load_module_from_input(lua: &Lua, base_path: &Path, key: &str) -> LuaResult<Value> {
    // Try <key>.lua first
    let file_path = base_path.join(format!("{}.lua", key));
    if file_path.exists() {
        return load_lua_file(lua, &file_path);
    }

    // Try <key>/init.lua
    let init_path = base_path.join(key).join("init.lua");
    if init_path.exists() {
        return load_lua_file(lua, &init_path);
    }

    // Check if it's a directory without init.lua (allow traversal)
    let dir_path = base_path.join(key);
    if dir_path.is_dir() {
        let loader = create_subdir_loader(lua, &dir_path)?;
        return Ok(Value::Table(loader));
    }

    Err(mlua::Error::runtime(format!(
        "Module '{}' not found in input (tried {}.lua and {}/init.lua)",
        key, key, key
    )))
}

/// Create a loader for a subdirectory within an input.
fn create_subdir_loader(lua: &Lua, dir_path: &Path) -> LuaResult<Table> {
    let loader = lua.create_table()?;

    loader.set("_type", "input_subdir")?;
    loader.set("_base_path", dir_path.to_string_lossy().to_string())?;

    // Create metatable with __index
    let metatable = lua.create_table()?;

    let index_fn = lua.create_function(move |lua, (tbl, key): (Table, String)| {
        let base: String = tbl.get("_base_path")?;
        let base_path = PathBuf::from(&base);
        load_module_from_input(lua, &base_path, &key)
    })?;

    metatable.set("__index", index_fn)?;
    loader.set_metatable(Some(metatable))?;

    Ok(loader)
}

/// Load and execute a Lua file, returning its result.
///
/// Temporarily modifies package.path to include the file's directory,
/// allowing require() calls within the file to find sibling modules.
fn load_lua_file(lua: &Lua, file_path: &Path) -> LuaResult<Value> {
    let source = std::fs::read_to_string(file_path).map_err(|e| {
        mlua::Error::runtime(format!("Failed to read {}: {}", file_path.display(), e))
    })?;

    // Get the directory containing this file
    let file_dir = file_path.parent().map(|p| p.to_path_buf());

    // Temporarily add the file's directory to package.path
    let old_path: Option<String> = if let Some(dir) = &file_dir {
        let package: Table = lua.globals().get("package")?;
        let old: String = package.get("path")?;

        // Add dir/?.lua and dir/?/init.lua to the front of package.path
        let dir_str = dir.to_string_lossy();
        let new_path = format!("{}/?.lua;{}/?/init.lua;{}", dir_str, dir_str, old);
        package.set("path", new_path)?;

        Some(old)
    } else {
        None
    };

    // Load and execute the chunk
    let chunk = lua.load(&source).set_name(file_path.to_string_lossy());
    let result = chunk.eval();

    // Restore old package.path
    if let Some(old) = old_path {
        let package: Table = lua.globals().get("package")?;
        package.set("path", old)?;
    }

    result
}

/// Create a placeholder table for unresolved inputs (GitHub inputs).
///
/// This table will error when accessed, indicating that the input needs
/// to be resolved first via `sys update`.
fn create_unresolved_input_placeholder(
    lua: &Lua,
    input_id: &str,
    source: &str,
) -> LuaResult<Table> {
    let placeholder = lua.create_table()?;

    placeholder.set("_type", "unresolved_input")?;
    placeholder.set("_input_id", input_id.to_string())?;
    placeholder.set("_source", source.to_string())?;

    // Create metatable with __index that errors
    let metatable = lua.create_table()?;
    let source_clone = source.to_string();

    let index_fn = lua.create_function(move |_, (_tbl, key): (Table, String)| {
        Err::<Value, _>(mlua::Error::runtime(format!(
            "Cannot access '{}' on unresolved input '{}'. Run 'sys update' first to fetch inputs.",
            key, source_clone
        )))
    })?;

    metatable.set("__index", index_fn)?;
    placeholder.set_metatable(Some(metatable))?;

    Ok(placeholder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syslua_global() {
        let lua = Lua::new();
        let platform = Platform::detect().unwrap();

        setup_syslua_global(&lua, &platform).unwrap();

        let syslua: Table = lua.globals().get("syslua").unwrap();

        let os: String = syslua.get("os").unwrap();
        assert!(!os.is_empty());

        let is_darwin: bool = syslua.get("is_darwin").unwrap();
        let is_linux: bool = syslua.get("is_linux").unwrap();
        let is_windows: bool = syslua.get("is_windows").unwrap();

        // Exactly one should be true
        assert_eq!(
            [is_darwin, is_linux, is_windows]
                .iter()
                .filter(|&&x| x)
                .count(),
            1
        );
    }

    #[test]
    fn test_file_function_symlink() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));
        let config_dir = PathBuf::from("/home/user/config");

        setup_file_function(&lua, declarations.clone(), config_dir).unwrap();

        lua.load(
            r#"
            file {
                path = "~/.gitconfig",
                symlink = "./dotfiles/gitconfig",
            }
        "#,
        )
        .exec()
        .unwrap();

        let decls = declarations.borrow();
        assert_eq!(decls.files.len(), 1);

        let file = &decls.files[0];
        assert!(file.path.to_string_lossy().contains(".gitconfig"));
        assert!(file.symlink.is_some());
        assert_eq!(file.kind(), "symlink");
    }

    #[test]
    fn test_file_function_content() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));
        let config_dir = PathBuf::from("/home/user/config");

        setup_file_function(&lua, declarations.clone(), config_dir).unwrap();

        lua.load(
            r#"
            file {
                path = "/tmp/test.txt",
                content = "Hello, world!",
            }
        "#,
        )
        .exec()
        .unwrap();

        let decls = declarations.borrow();
        assert_eq!(decls.files.len(), 1);

        let file = &decls.files[0];
        assert_eq!(file.content.as_deref(), Some("Hello, world!"));
    }

    #[test]
    fn test_file_function_validation_error() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));
        let config_dir = PathBuf::from("/home/user/config");

        setup_file_function(&lua, declarations.clone(), config_dir).unwrap();

        // Missing required field
        let result = lua
            .load(
                r#"
            file {
                path = "/tmp/test.txt",
            }
        "#,
            )
            .exec();

        assert!(result.is_err());
    }

    #[test]
    fn test_env_function_simple() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));

        setup_env_function(&lua, declarations.clone()).unwrap();

        lua.load(
            r#"
            env {
                EDITOR = "nvim",
            }
        "#,
        )
        .exec()
        .unwrap();

        let decls = declarations.borrow();
        assert_eq!(decls.envs.len(), 1);

        let env = &decls.envs[0];
        assert_eq!(env.name, "EDITOR");
        assert_eq!(env.values.len(), 1);
        assert_eq!(env.values[0].value, "nvim");
        assert!(!env.is_path_like());
    }

    #[test]
    fn test_env_function_path_prepend() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));

        setup_env_function(&lua, declarations.clone()).unwrap();

        lua.load(
            r#"
            env {
                PATH = { "/usr/local/bin", "/opt/bin" },
            }
        "#,
        )
        .exec()
        .unwrap();

        let decls = declarations.borrow();
        assert_eq!(decls.envs.len(), 1);

        let env = &decls.envs[0];
        assert_eq!(env.name, "PATH");
        assert_eq!(env.values.len(), 2);
        assert!(env.is_path_like());
        assert!(matches!(env.values[0].strategy, EnvMergeStrategy::Prepend));
    }

    #[test]
    fn test_env_function_explicit_append() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));

        setup_env_function(&lua, declarations.clone()).unwrap();

        lua.load(
            r#"
            env {
                MANPATH = { append = "/usr/share/man" },
            }
        "#,
        )
        .exec()
        .unwrap();

        let decls = declarations.borrow();
        assert_eq!(decls.envs.len(), 1);

        let env = &decls.envs[0];
        assert_eq!(env.name, "MANPATH");
        assert!(matches!(env.values[0].strategy, EnvMergeStrategy::Append));
    }

    #[test]
    fn test_env_function_tilde_expansion() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));

        setup_env_function(&lua, declarations.clone()).unwrap();

        lua.load(
            r#"
            env {
                PATH = { "~/.local/bin" },
            }
        "#,
        )
        .exec()
        .unwrap();

        let decls = declarations.borrow();
        let env = &decls.envs[0];

        // Should have expanded ~ to home directory
        assert!(!env.values[0].value.starts_with("~/"));
        assert!(env.values[0].value.contains(".local/bin"));
    }

    #[test]
    fn test_env_function_multiple() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));

        setup_env_function(&lua, declarations.clone()).unwrap();

        lua.load(
            r#"
            env {
                EDITOR = "nvim",
                PAGER = "less",
            }
        "#,
        )
        .exec()
        .unwrap();

        let decls = declarations.borrow();
        assert_eq!(decls.envs.len(), 2);
    }

    #[test]
    fn test_derivation_function_basic() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));

        setup_derivation_function(&lua, declarations.clone()).unwrap();

        lua.load(
            r#"
            local rg = derivation {
                name = "ripgrep",
                version = "15.1.0",
                inputs = {
                    url = "https://example.com/rg.tar.gz",
                    sha256 = "abc123",
                },
                build = function(ctx)
                    -- build steps
                end,
            }
        "#,
        )
        .exec()
        .unwrap();

        let decls = declarations.borrow();
        assert_eq!(decls.derivations.len(), 1);

        let drv = &decls.derivations[0];
        assert_eq!(drv.name, "ripgrep");
        assert_eq!(drv.version, Some("15.1.0".to_string()));
        assert!(drv.inputs.contains_key("url"));
        assert!(drv.inputs.contains_key("sha256"));
    }

    #[test]
    fn test_derivation_function_nested_inputs() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));

        setup_derivation_function(&lua, declarations.clone()).unwrap();

        lua.load(
            r#"
            derivation {
                name = "test-pkg",
                inputs = {
                    sources = {
                        main = "src/main.rs",
                        lib = "src/lib.rs",
                    },
                    flags = { "-O2", "-Wall" },
                    enabled = true,
                    count = 42,
                },
                build = function(ctx) end,
            }
        "#,
        )
        .exec()
        .unwrap();

        let decls = declarations.borrow();
        let drv = &decls.derivations[0];

        // Check nested table
        match drv.inputs.get("sources") {
            Some(DerivationInput::Table(t)) => {
                assert!(t.contains_key("main"));
                assert!(t.contains_key("lib"));
            }
            _ => panic!("Expected table for 'sources'"),
        }

        // Check array
        match drv.inputs.get("flags") {
            Some(DerivationInput::Array(a)) => {
                assert_eq!(a.len(), 2);
            }
            _ => panic!("Expected array for 'flags'"),
        }

        // Check bool
        match drv.inputs.get("enabled") {
            Some(DerivationInput::Bool(b)) => assert!(*b),
            _ => panic!("Expected bool for 'enabled'"),
        }

        // Check number
        match drv.inputs.get("count") {
            Some(DerivationInput::Number(n)) => assert_eq!(*n, 42.0),
            _ => panic!("Expected number for 'count'"),
        }
    }

    #[test]
    fn test_derivation_returns_table() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));

        setup_derivation_function(&lua, declarations.clone()).unwrap();

        // The derivation should return a table that can be used later
        lua.load(
            r#"
            local rg = derivation {
                name = "ripgrep",
                inputs = { url = "test" },
                build = function(ctx) end,
            }
            
            -- Verify the returned table has expected fields
            assert(rg.name == "ripgrep")
            assert(rg._type == "derivation")
        "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn test_pkg_function() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));

        setup_derivation_function(&lua, declarations.clone()).unwrap();
        setup_pkg_function(&lua, declarations.clone()).unwrap();

        lua.load(
            r#"
            local rg = derivation {
                name = "ripgrep",
                inputs = { url = "test" },
                build = function(ctx) end,
            }
            
            pkg(rg)
        "#,
        )
        .exec()
        .unwrap();

        let decls = declarations.borrow();
        assert_eq!(decls.derivations.len(), 1);
        assert_eq!(decls.pkgs.len(), 1);
        assert_eq!(decls.pkgs[0].derivation_name, "ripgrep");
        assert!(decls.pkgs[0].add_to_path);
    }

    #[test]
    fn test_pkg_function_requires_derivation() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));

        setup_pkg_function(&lua, declarations.clone()).unwrap();

        // Should fail when passing a non-derivation table
        let result = lua
            .load(
                r#"
            pkg({ name = "not-a-derivation" })
        "#,
            )
            .exec();

        assert!(result.is_err());
    }

    #[test]
    fn test_input_function_github() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));
        let config_dir = PathBuf::from("/tmp");

        setup_input_function(&lua, declarations.clone(), config_dir).unwrap();

        lua.load(
            r#"
            local pkgs = input { source = "sys-lua/pkgs" }
            -- Check that input returns a table
            assert(type(pkgs) == "table")
            -- Check that it's marked as unresolved
            assert(pkgs._type == "unresolved_input")
            assert(pkgs._source == "sys-lua/pkgs")
        "#,
        )
        .exec()
        .unwrap();

        let decls = declarations.borrow();
        assert_eq!(decls.inputs.len(), 1);
        assert_eq!(decls.inputs[0].source, "sys-lua/pkgs");
    }

    #[test]
    fn test_input_function_github_with_ref() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));
        let config_dir = PathBuf::from("/tmp");

        setup_input_function(&lua, declarations.clone(), config_dir).unwrap();

        // New syntax: ref is part of the source string (owner/repo/ref)
        lua.load(
            r#"
            local pkgs = input { source = "sys-lua/pkgs/v2.0.0" }
            assert(type(pkgs) == "table")
            assert(pkgs._source == "sys-lua/pkgs/v2.0.0")
        "#,
        )
        .exec()
        .unwrap();

        let decls = declarations.borrow();
        assert_eq!(decls.inputs.len(), 1);
        assert_eq!(decls.inputs[0].source, "sys-lua/pkgs/v2.0.0");
    }

    #[test]
    fn test_input_function_path() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));

        // Create a temporary directory with a test module
        let temp_dir = tempfile::TempDir::new().unwrap();
        let module_path = temp_dir.path().join("test_module.lua");
        std::fs::write(&module_path, "return { name = 'test' }").unwrap();

        setup_input_function(&lua, declarations.clone(), temp_dir.path().to_path_buf()).unwrap();

        // Use relative path from config dir
        lua.load(
            r#"
            local local_pkgs = input { source = "path:." }
            assert(type(local_pkgs) == "table")
            assert(local_pkgs._type == "input")
        "#,
        )
        .exec()
        .unwrap();

        let decls = declarations.borrow();
        assert_eq!(decls.inputs.len(), 1);
        assert_eq!(decls.inputs[0].source, "path:.");
        assert!(decls.inputs[0].resolved_path.is_some());
    }

    #[test]
    fn test_input_function_path_module_loading() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));

        // Create a temporary directory with a test module
        let temp_dir = tempfile::TempDir::new().unwrap();
        let module_path = temp_dir.path().join("mymodule.lua");
        std::fs::write(&module_path, "return { value = 42 }").unwrap();

        setup_input_function(&lua, declarations.clone(), temp_dir.path().to_path_buf()).unwrap();

        // Load a module from the input
        lua.load(
            r#"
            local pkgs = input { source = "path:." }
            local mymod = pkgs.mymodule
            assert(type(mymod) == "table")
            assert(mymod.value == 42)
        "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn test_input_function_path_nested_module() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));

        // Create a temporary directory with nested modules
        let temp_dir = tempfile::TempDir::new().unwrap();
        let subdir = temp_dir.path().join("tools");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(subdir.join("ripgrep.lua"), "return { name = 'ripgrep' }").unwrap();

        setup_input_function(&lua, declarations.clone(), temp_dir.path().to_path_buf()).unwrap();

        // Load a nested module
        lua.load(
            r#"
            local pkgs = input { source = "path:." }
            local rg = pkgs.tools.ripgrep
            assert(type(rg) == "table")
            assert(rg.name == "ripgrep")
        "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn test_input_function_path_init_lua() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));

        // Create a temporary directory with init.lua
        let temp_dir = tempfile::TempDir::new().unwrap();
        let subdir = temp_dir.path().join("mypackage");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(subdir.join("init.lua"), "return { initialized = true }").unwrap();

        setup_input_function(&lua, declarations.clone(), temp_dir.path().to_path_buf()).unwrap();

        // Load module that uses init.lua
        lua.load(
            r#"
            local pkgs = input { source = "path:." }
            local mypkg = pkgs.mypackage
            assert(type(mypkg) == "table")
            assert(mypkg.initialized == true)
        "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn test_input_function_multiple() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));

        let temp_dir = tempfile::TempDir::new().unwrap();
        setup_input_function(&lua, declarations.clone(), temp_dir.path().to_path_buf()).unwrap();

        lua.load(
            r#"
            local inputs = {
                pkgs = input { source = "sys-lua/pkgs" },
                extras = input { source = "sys-lua/extras/v1.0.0" },
            }
        "#,
        )
        .exec()
        .unwrap();

        let decls = declarations.borrow();
        assert_eq!(decls.inputs.len(), 2);
    }

    #[test]
    fn test_input_function_unresolved_access_error() {
        let lua = Lua::new();
        let declarations = Rc::new(RefCell::new(Declarations::new()));
        let config_dir = PathBuf::from("/tmp");

        setup_input_function(&lua, declarations.clone(), config_dir).unwrap();

        // Accessing an unresolved GitHub input should error
        let result = lua
            .load(
                r#"
            local pkgs = input { source = "sys-lua/pkgs" }
            local _ = pkgs.ripgrep  -- This should error
        "#,
            )
            .exec();

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("unresolved input"));
    }
}
