use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::setup::paths;

pub struct Progress {
    pub message: String,
    pub fraction: Option<f32>,
}

pub type ProgressTx = Arc<Mutex<dyn Fn(Progress) + Send + Sync>>;

fn python_binary() -> PathBuf {
    #[cfg(target_os = "windows")]
    return paths::python_dir().join("python").join("python.exe");
    #[cfg(not(target_os = "windows"))]
    return paths::python_dir().join("python").join("bin").join("python3");
}

pub fn is_installed() -> bool {
    python_binary().exists()
}

fn platform_tag() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "x86_64-pc-windows-msvc-shared";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "x86_64-unknown-linux-gnu";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "aarch64-apple-darwin";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "x86_64-apple-darwin";
}

pub fn install(report: impl Fn(Progress)) -> Result<(), String> {
    if is_installed() {
        report(Progress { message: "Python already installed".into(), fraction: Some(1.0) });
        return Ok(());
    }

    let version = "3.11.9";
    let tag = platform_tag();
    let filename = format!("cpython-{}%2B20240713-{}-install_only.tar.gz", version, tag);
    let url = format!(
        "https://github.com/indygreg/python-build-standalone/releases/download/20240713/{}",
        filename
    );

    report(Progress { message: format!("Downloading Python {}...", version), fraction: Some(0.0) });

    // Retry up to 3 times — large file downloads over reqwest blocking can
    // fail mid-stream with "error decoding response body"
    let buf = (0..3).find_map(|attempt| {
        if attempt > 0 {
            report(Progress {
                message: format!("Retrying Python download (attempt {})...", attempt + 1),
                fraction: Some(0.0),
            });
        }
        let result: Result<Vec<u8>, String> = (|| {
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .map_err(|e| e.to_string())?;
            let response = client.get(&url)
                .send()
                .map_err(|e| format!("Download failed: {}", e))?;
            let total = response.content_length().unwrap_or(0);
            let mut downloaded = 0u64;
            let mut buf: Vec<u8> = Vec::new();
            use std::io::Read;
            let mut reader = response;
            let mut chunk = [0u8; 65536];
            loop {
                let n = reader.read(&mut chunk).map_err(|e| e.to_string())?;
                if n == 0 { break; }
                buf.extend_from_slice(&chunk[..n]);
                downloaded += n as u64;
                if total > 0 {
                    report(Progress {
                        message: format!("Downloading Python... {:.1} MB / {:.1} MB",
                                         downloaded as f32 / 1_048_576.0,
                                         total as f32 / 1_048_576.0),
                        fraction: Some(downloaded as f32 / total as f32 * 0.7),
                    });
                }
            }
            Ok(buf)
        })();
        result.ok()
    }).ok_or_else(|| "Python download failed after 3 attempts".to_string())?;

    report(Progress { message: "Extracting Python...".into(), fraction: Some(0.7) });

    let cursor = std::io::Cursor::new(buf);
    let gz = flate2::read::GzDecoder::new(cursor);
    let mut archive = tar::Archive::new(gz);
    archive.unpack(paths::python_dir()).map_err(|e| format!("Extraction failed: {}", e))?;

    report(Progress { message: "Python ready".into(), fraction: Some(1.0) });
    Ok(())
}