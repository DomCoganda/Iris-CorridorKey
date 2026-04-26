use std::io::Read;

use crate::setup::paths;
use crate::setup::python::Progress;

pub fn download_corridor(report: impl Fn(Progress)) -> Result<(), String> {
    if corridor_installed() {
        report(Progress {
            message: "CorridorKey model already present".into(),
            fraction: Some(1.0),
        });
        return Ok(());
    }

    let dest = paths::corridor_models_dir();
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;

    download_file(
        "https://huggingface.co/nikopueringer/CorridorKey_v1.0/resolve/main/CorridorKey_v1.0.pth",
        dest.join("CorridorKey_v1.0.pth"),
        "CorridorKey model",
        &report,
    )
}

pub fn corridor_installed() -> bool {
    let dir = paths::corridor_models_dir();

    match std::fs::metadata(dir.join("CorridorKey_v1.0.pth")) {
        Ok(m) => m.len() > 100 * 1024 * 1024,
        Err(_) => false,
    }
}

pub fn download_birefnet(report: impl Fn(Progress)) -> Result<(), String> {
    if birefnet_installed() {
        report(Progress {
            message: "BiRefNet already present".into(),
            fraction: Some(1.0),
        });
        return Ok(());
    }

    let dest_dir = paths::alpha_models_dir().join("BiRefNet");
    std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;

    let base = "https://huggingface.co/ZhengPeng7/BiRefNet/resolve/main";

    for filename in &["birefnet.py", "BiRefNet_config.py", "config.json"] {
        download_file(
            &format!("{}/{}", base, filename),
            dest_dir.join(filename),
            filename,
            &report,
        )?;
    }

    download_file(
        &format!("{}/model.safetensors", base),
        dest_dir.join("model.safetensors"),
        "BiRefNet weights",
        &report,
    )
}

pub fn birefnet_installed() -> bool {
    match std::fs::metadata(paths::alpha_models_dir().join("BiRefNet").join("model.safetensors")) {
        Ok(m) => m.len() > 100 * 1024 * 1024,
        Err(_) => false,
    }
}

pub fn download_corridor_src(report: impl Fn(Progress)) -> Result<(), String> {
    let dest = paths::corridor_src_dir();
    let module_dir = dest.join("CorridorKeyModule");

    if module_dir.join("inference_engine.py").exists()
        && module_dir.join("core").join("model_transformer.py").exists()
    {
        report(Progress {
            message: "CorridorKey scripts already present".into(),
            fraction: Some(1.0),
        });
        return Ok(());
    }

    if dest.exists() {
        report(Progress {
            message: "Removing incomplete CorridorKey scripts...".into(),
            fraction: Some(0.0),
        });

        std::fs::remove_dir_all(&dest)
            .map_err(|e| format!("Failed to remove old CorridorKey scripts: {}", e))?;
    }

    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;

    report(Progress {
        message: "Downloading CorridorKey scripts...".into(),
        fraction: Some(0.0),
    });

    let client = client()?;
    let url = "https://github.com/nikopueringer/CorridorKey/archive/refs/heads/main.zip";

    let response = client
        .get(url)
        .send()
        .map_err(|e| format!("Download failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Server returned {} for CorridorKey scripts",
            response.status()
        ));
    }

    let bytes = response.bytes().map_err(|e| e.to_string())?;

    report(Progress {
        message: "Extracting CorridorKey scripts...".into(),
        fraction: Some(0.8),
    });

    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().to_string();

        let prefix = "CorridorKey-main/CorridorKeyModule/";
        if !name.starts_with(prefix) {
            continue;
        }

        let relative = &name[prefix.len()..];
        if relative.is_empty() {
            continue;
        }

        let out_path = module_dir.join(relative);

        if name.ends_with('/') {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }

            let mut out = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
        }
    }

    report(Progress {
        message: "CorridorKey scripts ready".into(),
        fraction: Some(1.0),
    });

    Ok(())
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())
}

fn download_file(
    url: &str,
    dest: std::path::PathBuf,
    label: &str,
    report: impl Fn(Progress),
) -> Result<(), String> {
    report(Progress {
        message: format!("Downloading {}...", label),
        fraction: Some(0.0),
    });

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let client = client()?;
    let response = client
        .get(url)
        .send()
        .map_err(|e| format!("Request failed for {}: {}", label, e))?;

    if !response.status().is_success() {
        return Err(format!("Server returned {} for {}", response.status(), label));
    }

    let total = response.content_length().unwrap_or(0);
    let mut downloaded = 0u64;
    let mut buf = Vec::new();
    let mut reader = response;
    let mut chunk = [0u8; 65536];

    loop {
        let n = reader
            .read(&mut chunk)
            .map_err(|e| format!("Read error for {}: {}", label, e))?;

        if n == 0 {
            break;
        }

        buf.extend_from_slice(&chunk[..n]);
        downloaded += n as u64;

        if total > 0 {
            report(Progress {
                message: format!(
                    "Downloading {}... {:.1} MB / {:.1} MB",
                    label,
                    downloaded as f32 / 1_048_576.0,
                    total as f32 / 1_048_576.0
                ),
                fraction: Some(downloaded as f32 / total as f32),
            });
        }
    }

    std::fs::write(&dest, &buf).map_err(|e| format!("Write failed for {}: {}", label, e))?;

    report(Progress {
        message: format!("{} ready", label),
        fraction: Some(1.0),
    });

    Ok(())
}