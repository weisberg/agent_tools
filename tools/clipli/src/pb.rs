// Pasteboard FFI — reads/writes macOS NSPasteboard via objc2.
// See CLIPLI_SPEC.md §5.1 for full specification.
//
// API notes for objc2 0.5 / objc2-foundation 0.2 / objc2-app-kit 0.2:
//
// • NSPasteboard::generalPasteboard() → Retained<NSPasteboard>
// • pb.types() → Option<Retained<NSArray<NSPasteboardType>>>
//   where NSPasteboardType is a type alias for NSString.
// • NSArray::objectAtIndex(i) → &T  (NSPasteboard types are &NSString)
//   Use Retained::retain(ref) to obtain an owned Retained<NSString>.
// • NSString::to_string() via the Display impl gives a Rust String.
// • pb.dataForType(&NSString) → Option<Retained<NSData>>
// • NSData::bytes() → NonNull<c_void>  — call .as_ptr() as *const u8
// • NSData::length() → NSUInteger (usize on 64-bit)
// • NSData::with_bytes(&[u8]) → Retained<NSData>  (safe wrapper)
// • pb.clearContents() → NSInteger  (return value ignored)
// • pb.setData_forType(&NSData, &NSString) → bool
// • NSWorkspace::sharedWorkspace() → Retained<NSWorkspace>
// • workspace.frontmostApplication() → Option<Retained<NSRunningApplication>>
//   Requires "NSRunningApplication" feature in objc2-app-kit.
// • app.bundleIdentifier() → Option<Retained<NSString>>

use crate::model::{PbSnapshot, PbType, PbTypeEntry};
use chrono::Utc;
use objc2::rc::Retained;
use objc2_app_kit::{NSPasteboard, NSRunningApplication, NSWorkspace};
use objc2_foundation::{NSArray, NSData, NSString};
use thiserror::Error;

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum PbError {
    #[error("pasteboard is empty")]
    Empty,
    #[error("requested type '{0}' not available on pasteboard")]
    TypeNotFound(String),
    #[error("failed to write to pasteboard: {0}")]
    WriteFailed(String),
    #[allow(dead_code)]
    #[error("objc runtime error: {0}")]
    ObjcError(String),
}

impl PbError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Empty => "PB_EMPTY",
            Self::TypeNotFound(_) => "PB_TYPE_NOT_FOUND",
            Self::WriteFailed(_) => "PB_WRITE_FAILED",
            Self::ObjcError(_) => "PB_OBJC_ERROR",
        }
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Copy NSData bytes into a Rust Vec<u8>.
///
/// NSData::bytes() returns NonNull<c_void> in objc2-foundation 0.2.
/// We use .as_ptr() to get a *const c_void, cast to *const u8, then build
/// a slice.  A zero-length NSData is valid; we return an empty vec.
fn nsdata_to_vec(d: &NSData) -> Vec<u8> {
    let len = d.length() as usize;
    if len == 0 {
        return vec![];
    }
    // bytes() → NonNull<c_void>
    let ptr = d.bytes().as_ptr() as *const u8;
    unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec()
}

// ── Read ──────────────────────────────────────────────────────────────────────

/// Read every type currently on the general pasteboard.
pub fn read_all() -> Result<PbSnapshot, PbError> {
    let captured_at = Utc::now();
    let source_app = source_app();

    let pb = unsafe { NSPasteboard::generalPasteboard() };

    // types() → Option<Retained<NSArray<NSPasteboardType>>>
    // NSPasteboardType is a type alias for NSString.
    let types_array: Retained<NSArray<NSString>> = match unsafe { pb.types() } {
        Some(a) => a,
        None => {
            return Ok(PbSnapshot {
                types: vec![],
                captured_at,
                source_app,
            });
        }
    };

    let mut entries: Vec<PbTypeEntry> = Vec::new();

    let count = types_array.count();
    for i in 0..count {
        // objectAtIndex returns &NSString; retain to get an owned Retained<NSString>.
        let uti_retained: Retained<NSString> = unsafe { types_array.objectAtIndex(i) };
        let uti: String = uti_retained.to_string();

        let pb_type = PbType::from_uti(&uti);

        // dataForType takes &NSPasteboardType which is &NSString.
        let data = match unsafe { pb.dataForType(&*uti_retained) } {
            Some(d) => nsdata_to_vec(&d),
            None => vec![],
        };

        let size_bytes = data.len();
        entries.push(PbTypeEntry {
            pb_type,
            uti,
            size_bytes,
            data,
        });
    }

    if entries.is_empty() {
        return Err(PbError::Empty);
    }

    Ok(PbSnapshot {
        types: entries,
        captured_at,
        source_app,
    })
}

/// Read a single type from the pasteboard, returning raw bytes.
pub fn read_type(pb_type: PbType) -> Result<Vec<u8>, PbError> {
    let snapshot = read_all()?;
    let data = snapshot
        .types
        .into_iter()
        .find(|e| e.pb_type == pb_type)
        .map(|e| e.data)
        .ok_or_else(|| PbError::TypeNotFound(pb_type.uti().to_string()))?;
    tracing::debug!(uti = %pb_type.uti(), bytes = data.len(), "pb: read type");
    Ok(data)
}

/// Read a specific UTI string from the pasteboard, returning raw bytes.
#[allow(dead_code)]
pub fn read_uti(uti: &str) -> Result<Vec<u8>, PbError> {
    let pb = unsafe { NSPasteboard::generalPasteboard() };
    let uti_key = NSString::from_str(uti);
    match unsafe { pb.dataForType(&*uti_key) } {
        Some(d) => Ok(nsdata_to_vec(&d)),
        None => Err(PbError::TypeNotFound(uti.to_string())),
    }
}

// ── Write ─────────────────────────────────────────────────────────────────────

/// Write one or more (type, data) pairs to the pasteboard atomically.
///
/// Calls `clearContents` first (single change-count increment), then
/// `setData:forType:` for each entry.
pub fn write(entries: &[(PbType, &[u8])]) -> Result<(), PbError> {
    if entries.is_empty() {
        return Ok(());
    }

    let pb = unsafe { NSPasteboard::generalPasteboard() };

    // clearContents returns NSInteger; ignore it — it represents the new
    // change count, not a success/failure indicator.
    unsafe { pb.clearContents() };

    for (pb_type, data) in entries {
        let uti_key = NSString::from_str(pb_type.uti());
        // NSData::with_bytes is the safe constructor available in
        // objc2-foundation 0.2 that takes a &[u8] slice.
        let ns_data = NSData::with_bytes(data);
        let ok = unsafe { pb.setData_forType(Some(&ns_data), &*uti_key) };
        if !ok {
            return Err(PbError::WriteFailed(format!(
                "setData:forType: returned NO for {}",
                pb_type.uti()
            )));
        }
    }

    Ok(())
}

/// Write HTML string + optional plain-text fallback to the pasteboard.
pub fn write_html(html: &str, plain: Option<&str>) -> Result<(), PbError> {
    tracing::debug!(html_bytes = html.len(), "pb: writing HTML to pasteboard");
    let html_bytes = html.as_bytes();
    match plain {
        Some(p) => write(&[
            (PbType::Html, html_bytes),
            (PbType::PlainText, p.as_bytes()),
        ]),
        None => write(&[(PbType::Html, html_bytes)]),
    }
}

// ── Source app (best-effort) ──────────────────────────────────────────────────

/// Attempt to identify the frontmost app at the time of reading.
///
/// Returns the bundle identifier, e.g. `"com.microsoft.Excel"`.
/// This is a heuristic — it identifies the *current* frontmost application,
/// not necessarily the one that last wrote to the pasteboard.
///
/// Requires the `NSRunningApplication` feature in `objc2-app-kit` (see
/// Cargo.toml).
pub fn source_app() -> Option<String> {
    let workspace = unsafe { NSWorkspace::sharedWorkspace() };
    // frontmostApplication() → Option<Retained<NSRunningApplication>>
    let app: Retained<NSRunningApplication> = unsafe { workspace.frontmostApplication() }?;
    // bundleIdentifier() → Option<Retained<NSString>>
    let bundle_id: Retained<NSString> = unsafe { app.bundleIdentifier() }?;
    Some(bundle_id.to_string())
}

// ── Tests (require macOS GUI session, marked #[ignore]) ───────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Basic round-trip: write plain text, read it back.
    #[test]
    #[ignore = "requires macOS GUI session with pasteboard access"]
    fn test_roundtrip_plain_text() {
        let text = "clipli test: hello world";
        write(&[(PbType::PlainText, text.as_bytes())]).expect("write failed");
        let data = read_type(PbType::PlainText).expect("read failed");
        assert_eq!(String::from_utf8(data).unwrap(), text);
    }

    /// HTML round-trip.
    #[test]
    #[ignore = "requires macOS GUI session with pasteboard access"]
    fn test_roundtrip_html() {
        let html = "<b>clipli</b> <em>test</em>";
        write(&[(PbType::Html, html.as_bytes())]).expect("write html failed");
        let data = read_type(PbType::Html).expect("read html failed");
        assert_eq!(String::from_utf8(data).unwrap(), html);
    }

    /// Multi-type write: HTML + plain text.
    #[test]
    #[ignore = "requires macOS GUI session with pasteboard access"]
    fn test_write_html_with_plain() {
        write_html("<p>Hello</p>", Some("Hello")).expect("write_html failed");
        let html_data = read_type(PbType::Html).expect("html not found");
        let plain_data = read_type(PbType::PlainText).expect("plain not found");
        assert!(String::from_utf8(html_data).unwrap().contains("Hello"));
        assert_eq!(String::from_utf8(plain_data).unwrap(), "Hello");
    }

    /// Inspect-style: read_all returns a non-empty snapshot after a write.
    #[test]
    #[ignore = "requires macOS GUI session with pasteboard access"]
    fn test_read_all() {
        write(&[(PbType::PlainText, b"test content")]).expect("write failed");
        let snap = read_all().expect("read_all failed");
        assert!(!snap.types.is_empty());
        assert!(snap.types.iter().any(|e| e.pb_type == PbType::PlainText));
    }

    /// Read a type that was not written → TypeNotFound.
    #[test]
    #[ignore = "requires macOS GUI session with pasteboard access"]
    fn test_type_not_found() {
        // Write only plain text, then ask for PDF.
        write(&[(PbType::PlainText, b"no pdf here")]).expect("write failed");
        let result = read_type(PbType::Pdf);
        assert!(matches!(result, Err(PbError::TypeNotFound(_))));
    }
}
