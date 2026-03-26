use std::collections::BTreeMap;

use bashli_core::VarError;
use serde_json::Value;

use crate::path::resolve_path;

/// Variable store with global variables and a lexical scope stack.
///
/// Variables are looked up scope-first (most recent scope wins), then globals.
/// System variables (prefixed with `_`) are stored as globals.
#[derive(Debug, Clone)]
pub struct VarStore {
    globals: BTreeMap<String, Value>,
    scopes: Vec<BTreeMap<String, Value>>,
}

impl VarStore {
    /// Create a new, empty variable store with no scopes.
    pub fn new() -> Self {
        Self {
            globals: BTreeMap::new(),
            scopes: Vec::new(),
        }
    }

    /// Resolve a variable reference such as `VAR`, `VAR.field[0].name`, or `ENV.PATH`.
    ///
    /// The reference should NOT include the leading `$`.
    pub fn resolve(&self, reference: &str) -> Result<Value, VarError> {
        let reference = reference.strip_prefix('$').unwrap_or(reference);
        // Split into the root variable name and an optional path tail.
        let (root_name, tail) = split_reference(reference);

        let root_value = self.lookup(root_name)?;

        if tail.is_empty() {
            Ok(root_value)
        } else {
            resolve_path(&root_value, tail)
        }
    }

    /// Set a global variable. Strips leading `$` if present.
    pub fn set(&mut self, name: &str, value: Value) {
        let name = name.strip_prefix('$').unwrap_or(name);
        self.globals.insert(name.to_string(), value);
    }

    /// Push a new empty scope onto the scope stack.
    pub fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    /// Pop the most recent scope. Does nothing if no scopes are active.
    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Set a variable in the current (topmost) scope.
    ///
    /// If no scope is active, falls back to setting a global.
    pub fn set_scoped(&mut self, name: &str, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), value);
        } else {
            self.set(name, value);
        }
    }

    /// Return all known variable names (globals + all scopes), deduplicated and sorted.
    pub fn keys(&self) -> Vec<&str> {
        let mut seen = BTreeMap::new();
        for key in self.globals.keys() {
            seen.insert(key.as_str(), ());
        }
        for scope in &self.scopes {
            for key in scope.keys() {
                seen.insert(key.as_str(), ());
            }
        }
        seen.into_keys().collect()
    }

    /// Export a subset of variables by name. Missing keys are silently skipped.
    pub fn export_summary(&self, keys: &[String]) -> BTreeMap<String, Value> {
        let mut out = BTreeMap::new();
        for key in keys {
            if let Ok(val) = self.resolve(key) {
                out.insert(key.clone(), val);
            }
        }
        out
    }

    /// Initialise well-known system variables (`$_CWD`, `$_HOME`, `$_OS`, `$_ARCH`, `$_TIMESTAMP`).
    pub fn init_system_vars(&mut self) {
        if let Ok(cwd) = std::env::current_dir() {
            self.set("_CWD", Value::String(cwd.to_string_lossy().into_owned()));
        }
        if let Some(home) = dirs_home() {
            self.set("_HOME", Value::String(home));
        }
        self.set("_OS", Value::String(std::env::consts::OS.to_string()));
        self.set("_ARCH", Value::String(std::env::consts::ARCH.to_string()));

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.set("_TIMESTAMP", Value::Number(ts.into()));
    }

    /// Export all non-system variables.
    pub fn export_all(&self) -> BTreeMap<String, Value> {
        let mut out = BTreeMap::new();
        for (k, v) in &self.globals {
            if !k.starts_with('_') {
                out.insert(format!("${k}"), v.clone());
            }
        }
        out
    }

    /// Convenience: interpolate a template string using this store.
    pub fn interpolate(&self, template: &str, escape: bool) -> Result<String, VarError> {
        crate::interpolate::interpolate(template, self, escape)
    }

    // ---- internal helpers ----

    /// Look up a bare variable name, checking scopes (top-down) then globals.
    fn lookup(&self, name: &str) -> Result<Value, VarError> {
        // Search scopes from top (most recent) to bottom.
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Ok(val.clone());
            }
        }
        if let Some(val) = self.globals.get(name) {
            return Ok(val.clone());
        }
        Err(VarError::Undefined(name.to_string()))
    }
}

impl Default for VarStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Split `"VAR.field[0]"` into `("VAR", "field[0]")`.
/// The root name is everything up to the first `.` or `[`.
fn split_reference(reference: &str) -> (&str, &str) {
    let end = reference
        .find(|c: char| c == '.' || c == '[')
        .unwrap_or(reference.len());
    let root = &reference[..end];
    let tail = &reference[end..];
    // Strip a leading dot if present so `resolve_path` sees `field[0]` not `.field[0]`.
    let tail = tail.strip_prefix('.').unwrap_or(tail);
    (root, tail)
}

/// Portable way to get the home directory without pulling in the `dirs` crate.
fn dirs_home() -> Option<String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn set_and_resolve_global() {
        let mut store = VarStore::new();
        store.set("FOO", json!("bar"));
        assert_eq!(store.resolve("FOO").unwrap(), json!("bar"));
    }

    #[test]
    fn undefined_variable() {
        let store = VarStore::new();
        let err = store.resolve("MISSING").unwrap_err();
        assert!(matches!(err, VarError::Undefined(_)));
    }

    #[test]
    fn resolve_with_path() {
        let mut store = VarStore::new();
        store.set("DATA", json!({"items": [{"name": "alice"}]}));
        assert_eq!(
            store.resolve("DATA.items[0].name").unwrap(),
            json!("alice")
        );
    }

    #[test]
    fn scope_shadows_global() {
        let mut store = VarStore::new();
        store.set("X", json!(1));
        store.push_scope();
        store.set_scoped("X", json!(2));
        assert_eq!(store.resolve("X").unwrap(), json!(2));
        store.pop_scope();
        assert_eq!(store.resolve("X").unwrap(), json!(1));
    }

    #[test]
    fn nested_scopes() {
        let mut store = VarStore::new();
        store.set("V", json!("global"));
        store.push_scope();
        store.set_scoped("V", json!("scope1"));
        store.push_scope();
        store.set_scoped("V", json!("scope2"));
        assert_eq!(store.resolve("V").unwrap(), json!("scope2"));
        store.pop_scope();
        assert_eq!(store.resolve("V").unwrap(), json!("scope1"));
        store.pop_scope();
        assert_eq!(store.resolve("V").unwrap(), json!("global"));
    }

    #[test]
    fn set_scoped_without_scope_sets_global() {
        let mut store = VarStore::new();
        store.set_scoped("K", json!(99));
        assert_eq!(store.resolve("K").unwrap(), json!(99));
    }

    #[test]
    fn keys_deduplicates_and_sorts() {
        let mut store = VarStore::new();
        store.set("B", json!(1));
        store.set("A", json!(2));
        store.push_scope();
        store.set_scoped("A", json!(3));
        store.set_scoped("C", json!(4));
        assert_eq!(store.keys(), vec!["A", "B", "C"]);
    }

    #[test]
    fn export_summary_picks_subset() {
        let mut store = VarStore::new();
        store.set("X", json!(1));
        store.set("Y", json!(2));
        store.set("Z", json!(3));
        let summary = store.export_summary(&["X".into(), "Z".into(), "NOPE".into()]);
        assert_eq!(summary.len(), 2);
        assert_eq!(summary["X"], json!(1));
        assert_eq!(summary["Z"], json!(3));
    }

    #[test]
    fn init_system_vars_populates() {
        let mut store = VarStore::new();
        store.init_system_vars();
        assert!(store.resolve("_OS").is_ok());
        assert!(store.resolve("_ARCH").is_ok());
        assert!(store.resolve("_CWD").is_ok());
        assert!(store.resolve("_TIMESTAMP").is_ok());
    }

    #[test]
    fn split_reference_simple() {
        assert_eq!(split_reference("VAR"), ("VAR", ""));
    }

    #[test]
    fn split_reference_with_dot() {
        assert_eq!(split_reference("VAR.field"), ("VAR", "field"));
    }

    #[test]
    fn split_reference_with_bracket() {
        assert_eq!(split_reference("ARR[0]"), ("ARR", "[0]"));
    }

    #[test]
    fn split_reference_complex() {
        assert_eq!(split_reference("OBJ.a[1].b"), ("OBJ", "a[1].b"));
    }
}
