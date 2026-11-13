//! Binary transfer shared by the providers that move files: reading a local
//! file to upload, and saving a downloaded body to disk. File bytes never
//! touch stdout, so every command still prints exactly one JSON document.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// A local file read for upload: its bytes plus the name and MIME type the
/// provider records alongside them.
pub(crate) struct Upload {
    pub(crate) filename: String,
    pub(crate) content_type: String,
    pub(crate) bytes: Vec<u8>,
}

impl Upload {
    /// Read `path` whole; the MIME type is guessed from its extension.
    pub(crate) fn read(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("{}: not a UTF-8 file name", path.display()))?
            .to_owned();
        let content_type = mime_guess::from_path(path)
            .first_or_octet_stream()
            .essence_str()
            .to_owned();
        Ok(Self {
            filename,
            content_type,
            bytes,
        })
    }

    pub(crate) fn is_image(&self) -> bool {
        self.content_type.starts_with("image/")
    }
}

/// The destination flag every download verb shares.
#[derive(clap::Args)]
pub struct OutputPath {
    /// Write the file's bytes to this path, replacing any existing file
    #[arg(long, short = 'o')]
    pub output: PathBuf,
}

/// Write a download's body to `output` and return foac's summary of it:
/// the absolute path, the byte count, and the server's Content-Type. A
/// failed status is an error and leaves `output` untouched.
pub(crate) fn save(
    mut response: reqwest::blocking::Response,
    output: &Path,
) -> Result<Value, Box<dyn std::error::Error>> {
    let status = response.status();
    if !status.is_success() {
        let body = crate::rest::parse_response(status, response.text()?);
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(crate::pipe::NotFound(body.to_string()).into());
        }
        return Err(body.to_string().into());
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let path = std::path::absolute(output)?;
    let written = std::fs::File::create(&path)
        .map_err(Box::<dyn std::error::Error>::from)
        .and_then(|mut file| Ok(std::io::copy(&mut response, &mut file)?));
    let size = match written {
        Ok(size) => size,
        Err(error) => {
            // Never leave a truncated file behind for the caller to trust.
            let _ = std::fs::remove_file(&path);
            return Err(format!("{}: {error}", path.display()).into());
        }
    };
    Ok(json!({
        "path": path.display().to_string(),
        "size": size,
        "contentType": content_type,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_reads_bytes_name_and_guessed_type() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("shot.png");
        std::fs::write(&png, b"\x89PNG").unwrap();
        let upload = Upload::read(&png).unwrap();
        assert_eq!(upload.filename, "shot.png");
        assert_eq!(upload.content_type, "image/png");
        assert_eq!(upload.bytes, b"\x89PNG");
        assert!(upload.is_image());

        let unknown = dir.path().join("blob.zzz-unknown");
        std::fs::write(&unknown, b"x").unwrap();
        let upload = Upload::read(&unknown).unwrap();
        assert_eq!(upload.content_type, "application/octet-stream");
        assert!(!upload.is_image());

        let missing = Upload::read(&dir.path().join("missing.txt"));
        assert!(missing.err().unwrap().to_string().contains("missing.txt"));
    }

    #[test]
    fn save_writes_the_body_and_summarizes_it() {
        let (url, _, server) = crate::rest::testing::test_server("200 OK", "file-bytes", "");
        let response = reqwest::blocking::get(url).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("out.bin");
        let summary = save(response, &output).unwrap();
        server.join().unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), b"file-bytes");
        assert_eq!(summary["size"], 10);
        assert_eq!(summary["contentType"], "application/json");
        assert_eq!(summary["path"], output.display().to_string());
    }

    #[test]
    fn save_reports_failures_without_writing() {
        let (url, _, server) =
            crate::rest::testing::test_server("404 Not Found", r#"{"error":"gone"}"#, "");
        let response = reqwest::blocking::get(url).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("out.bin");
        let error = save(response, &output).unwrap_err();
        server.join().unwrap();
        assert!(error.is::<crate::pipe::NotFound>());
        assert!(!output.exists());
    }
}
