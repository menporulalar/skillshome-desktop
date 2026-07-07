//! Server_Fallback path (Module 4 task 4.10) — hands a resume file straight to
//! skillshome-app's existing REST ingestion pipeline instead of running extraction
//! locally. See `auth::backend_client` for the actual HTTP calls; this module owns
//! the file-side glue (mime detection, byte reading) between a picked file path
//! and that client.

use std::path::Path;

/// Mirrors `ALLOWED_MIMES` in skillshome-app's
/// `app/api/profiles/[id]/ingest/route.ts` exactly — the backend rejects anything
/// not in that table, so a mismatch here would surface as a confusing "unsupported
/// file type" error for a file the user picked from a filtered dialog.
pub fn mime_for_extension(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "pdf" => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "doc" => "application/msword",
        "txt" => "text/plain",
        "md" => "text/markdown",
        "json" => "application/json",
        "zip" => "application/zip",
        _ => return None,
    })
}

/// Reads the file at `file_path` and infers its mime type from its extension.
/// Returns `(bytes, filename, mime)` ready to hand to `BackendClient::start_ingest`.
pub fn read_file_for_ingest(file_path: &str) -> Result<(Vec<u8>, String, &'static str), String> {
    let path = Path::new(file_path);
    let mime = mime_for_extension(path)
        .ok_or_else(|| format!("Unsupported file type: {file_path}. Allowed: PDF, DOCX, TXT, MD, JSON, ZIP"))?;
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("upload")
        .to_string();
    let bytes = std::fs::read(path).map_err(|e| format!("Failed to read file: {e}"))?;
    Ok((bytes, filename, mime))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn mime_for_extension_covers_all_allowed_types() {
        assert_eq!(mime_for_extension(Path::new("resume.pdf")), Some("application/pdf"));
        assert_eq!(
            mime_for_extension(Path::new("resume.docx")),
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        );
        assert_eq!(mime_for_extension(Path::new("resume.doc")), Some("application/msword"));
        assert_eq!(mime_for_extension(Path::new("resume.txt")), Some("text/plain"));
        assert_eq!(mime_for_extension(Path::new("resume.md")), Some("text/markdown"));
        assert_eq!(mime_for_extension(Path::new("export.json")), Some("application/json"));
        assert_eq!(mime_for_extension(Path::new("repo.zip")), Some("application/zip"));
    }

    #[test]
    fn mime_for_extension_is_case_insensitive() {
        assert_eq!(mime_for_extension(Path::new("Resume.PDF")), Some("application/pdf"));
    }

    #[test]
    fn mime_for_extension_rejects_unknown_types() {
        assert_eq!(mime_for_extension(Path::new("resume.exe")), None);
        assert_eq!(mime_for_extension(Path::new("no-extension")), None);
    }

    #[test]
    fn read_file_for_ingest_reads_bytes_and_infers_mime() {
        let mut path = std::env::temp_dir();
        path.push(format!("skillshome-desktop-ingest-test-{}.txt", std::process::id()));
        let mut file = std::fs::File::create(&path).expect("create temp file");
        file.write_all(b"hello resume").expect("write temp file");

        let (bytes, filename, mime) = read_file_for_ingest(path.to_str().unwrap()).expect("should read");

        assert_eq!(bytes, b"hello resume");
        assert!(filename.starts_with("skillshome-desktop-ingest-test-"));
        assert_eq!(mime, "text/plain");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn read_file_for_ingest_rejects_unsupported_extension() {
        let err = read_file_for_ingest("resume.exe").expect_err("should fail");
        assert!(err.contains("Unsupported file type"));
    }
}
