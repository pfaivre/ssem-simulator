//! Atomic file writes: the data lands in a sibling temporary file first,
//! then a rename swaps it into place, so a crash mid-write leaves either
//! the old file or the new one, never a truncated mix.

use std::path::Path;

/// Write `contents` to `path` atomically. The temporary file lives in the
/// same directory as the target, so the rename stays on one filesystem
/// (and is atomic on both POSIX and Windows).
pub(crate) fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let tmp = path.with_file_name(format!("{file_name}.{}.tmp", std::process::id()));
    let result = std::fs::write(&tmp, contents).and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}
