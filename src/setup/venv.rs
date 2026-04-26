use std::process::Command;
use std::io::{BufRead, BufReader};
use crate::setup::paths;
use crate::setup::python::Progress;

pub fn is_ready() -> bool {
    #[cfg(target_os = "windows")]
    let pip = paths::venv_dir().join("Scripts").join("pip.exe");
    #[cfg(not(target_os = "windows"))]
    let pip = paths::venv_dir().join("bin").join("pip");
    pip.exists()
}

fn python_bin() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    return paths::python_dir().join("python").join("python.exe");
    #[cfg(not(target_os = "windows"))]
    return paths::python_dir().join("python").join("bin").join("python3");
}

fn venv_python() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    return paths::venv_dir().join("Scripts").join("python.exe");
    #[cfg(not(target_os = "windows"))]
    return paths::venv_dir().join("bin").join("python3");
}

fn detect_gpu_vendor() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        let result = Command::new("powershell")
            .args([
                "-NoProfile", "-Command",
                "(Get-CimInstance Win32_VideoController | \
                  Where-Object { $_.Caption -notmatch 'Microsoft|VMware|VirtualBox|Parsec' } | \
                  Select-Object -First 1 -ExpandProperty Caption)",
            ])
            .output();

        if let Ok(out) = result {
            let name = String::from_utf8_lossy(&out.stdout).to_lowercase();
            if name.contains("nvidia") || name.contains("geforce") || name.contains("quadro") {
                return "nvidia";
            }
            if name.contains("amd") || name.contains("radeon") {
                return "amd";
            }
            if name.contains("intel") && (name.contains("arc") || name.contains("iris xe")) {
                return "intel";
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if Command::new("nvidia-smi").output().map(|o| o.status.success()).unwrap_or(false) {
            return "nvidia";
        }
        if let Ok(out) = Command::new("lspci").output() {
            let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
            if text.contains("amd") || text.contains("radeon") { return "amd"; }
            if text.contains("intel") { return "intel"; }
        }
    }

    "unknown"
}

fn cuda_available() -> bool {
    Command::new(venv_python())
        .args(["-c", "import torch; print(torch.cuda.is_available())"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "True")
        .unwrap_or(false)
}

fn directml_available() -> bool {
    Command::new(venv_python())
        .args(["-c", "import torch_directml"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run a pip install command and stream download progress back through `report`.
/// Parses pip's progress lines which look like:
///   Downloading torch-2.4.0-...-win_amd64.whl (2.6 GB)
///   ━━━━━━━━━━ 1.2/2.6 GB 45.2 MB/s eta 0:00:30
fn run_with_progress(
    cmd: &mut Command,
    report: &impl Fn(Progress),
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    // pip writes progress to stdout when not quiet
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Drain stderr silently on a background thread so the pipe doesn't block
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for _ in reader.lines() {}
    });

    let reader = BufReader::new(stdout);
    let mut total_gb: Option<f64> = None;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        let trimmed = line.trim();

        // "Downloading torch-....whl (2.6 GB)" — grab the total size
        if trimmed.starts_with("Downloading ") && trimmed.contains("GB)") {
            if let Some(start) = trimmed.rfind('(') {
                let size_str = &trimmed[start+1..];
                if let Some(end) = size_str.find(" GB)") {
                    if let Ok(gb) = size_str[..end].trim().parse::<f64>() {
                        total_gb = Some(gb);
                    }
                }
            }
        }

        // Progress line contains "/" and either "MB/s" or "GB/s"
        // e.g. "   ━━━━ 1.2/2.6 GB 45.2 MB/s eta 0:00:30"
        if (trimmed.contains("MB/s") || trimmed.contains("GB/s")) && trimmed.contains('/') {
            // Extract downloaded/total and speed
            let mut downloaded: Option<f64> = None;
            let mut speed_str = String::new();
            let mut fraction = 0.0f32;

            for part in trimmed.split_whitespace() {
                // "1.2/2.6"
                if part.contains('/') && !part.contains(':') {
                    let sides: Vec<&str> = part.split('/').collect();
                    if sides.len() == 2 {
                        if let (Ok(a), Ok(b)) = (
                            sides[0].parse::<f64>(),
                            sides[1].parse::<f64>(),
                        ) {
                            downloaded = Some(a);
                            if b > 0.0 {
                                fraction = (a / b) as f32;
                            }
                            if total_gb.is_none() {
                                total_gb = Some(b);
                            }
                        }
                    }
                }

                // "45.2" followed by "MB/s" or "GB/s"
                if part == "MB/s" || part == "GB/s" {
                    // speed was the previous token — captured below
                }
            }

            // Re-scan for speed token (the number before MB/s or GB/s)
            let tokens: Vec<&str> = trimmed.split_whitespace().collect();
            for i in 0..tokens.len() {
                if (tokens[i] == "MB/s" || tokens[i] == "GB/s") && i > 0 {
                    speed_str = format!("{} {}", tokens[i - 1], tokens[i]);
                    break;
                }
            }

            let message = match (downloaded, total_gb.as_ref()) {
                (Some(dl), Some(total)) => {
                    if speed_str.is_empty() {
                        format!("Downloading CUDA torch... {:.1}/{:.1} GB", dl, total)
                    } else {
                        format!(
                            "Downloading CUDA torch... {:.1}/{:.1} GB  @ {}",
                            dl, total, speed_str
                        )
                    }
                }
                _ => {
                    if speed_str.is_empty() {
                        "Downloading CUDA torch...".to_string()
                    } else {
                        format!("Downloading CUDA torch... @ {}", speed_str)
                    }
                }
            };

            report(Progress { message, fraction: Some(fraction) });
        }
    }

    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("pip install failed".to_string())
    }
}

pub fn ensure_gpu_torch(report: impl Fn(Progress)) -> Result<(), String> {
    let gpu = detect_gpu_vendor();
    report(Progress {
        message: format!("Checking GPU support (detected: {})...", gpu),
        fraction: Some(0.0),
    });

    match gpu {
        "nvidia" => {
            if cuda_available() {
                report(Progress {
                    message: "CUDA torch already installed.".into(),
                    fraction: Some(1.0),
                });
                return Ok(());
            }
            report(Progress {
                message: "Downloading CUDA torch for NVIDIA GPU...".into(),
                fraction: Some(0.0),
            });
            run_with_progress(
                Command::new(venv_python()).args([
                    "-m", "pip", "install",
                    "torch", "torchvision",
                    "--index-url", "https://download.pytorch.org/whl/cu124",
                    "--force-reinstall",
                    "--progress-bar", "on",
                ]),
                &report,
            )?;
            report(Progress {
                message: "CUDA torch installed.".into(),
                fraction: Some(1.0),
            });
        }

        "amd" | "intel" => {
            if directml_available() {
                report(Progress {
                    message: "DirectML already installed.".into(),
                    fraction: Some(1.0),
                });
                return Ok(());
            }
            report(Progress {
                message: format!("Installing torch-directml for {} GPU...", gpu.to_uppercase()),
                fraction: Some(0.2),
            });
            run_command(Command::new(venv_python()).args([
                "-m", "pip", "install", "torch-directml", "--quiet",
            ]))?;
            report(Progress {
                message: "DirectML installed.".into(),
                fraction: Some(1.0),
            });
        }

        _ => {
            report(Progress {
                message: "No dedicated GPU detected — using CPU.".into(),
                fraction: Some(1.0),
            });
        }
    }

    Ok(())
}

fn install_torch_for_gpu(vendor: &str, report: &impl Fn(Progress)) -> Result<(), String> {
    match vendor {
        "nvidia" => {
            report(Progress {
                message: "Installing PyTorch with CUDA (NVIDIA)...".into(),
                fraction: Some(0.5),
            });
            run_with_progress(
                Command::new(venv_python()).args([
                    "-m", "pip", "install",
                    "torch", "torchvision",
                    "--index-url", "https://download.pytorch.org/whl/cu124",
                    "--progress-bar", "on",
                ]),
                report,
            )?;
        }
        "amd" | "intel" => {
            report(Progress {
                message: format!("Installing PyTorch + DirectML ({})...", vendor.to_uppercase()),
                fraction: Some(0.5),
            });
            run_command(Command::new(venv_python()).args([
                "-m", "uv", "pip", "install",
                "torch", "torchvision", "--quiet",
            ]))?;
            run_command(Command::new(venv_python()).args([
                "-m", "pip", "install", "torch-directml", "--quiet",
            ]))?;
        }
        _ => {
            report(Progress {
                message: "Installing PyTorch (CPU)...".into(),
                fraction: Some(0.5),
            });
            run_command(Command::new(venv_python()).args([
                "-m", "uv", "pip", "install",
                "torch", "torchvision", "--quiet",
            ]))?;
        }
    }
    Ok(())
}

pub fn create(report: impl Fn(Progress)) -> Result<(), String> {
    if is_ready() {
        report(Progress { message: "Environment already ready".into(), fraction: Some(1.0) });
        return Ok(());
    }

    report(Progress { message: "Creating virtual environment...".into(), fraction: Some(0.0) });
    run_command(Command::new(python_bin())
        .args(["-m", "venv"])
        .arg(paths::venv_dir()))?;

    report(Progress { message: "Installing uv...".into(), fraction: Some(0.2) });
    run_command(Command::new(venv_python())
        .args(["-m", "pip", "install", "uv", "--quiet"]))?;

    let gpu = detect_gpu_vendor();
    report(Progress {
        message: format!("Detected GPU: {} — installing PyTorch...", gpu),
        fraction: Some(0.3),
    });
    install_torch_for_gpu(gpu, &report)?;

    report(Progress { message: "Installing CorridorKey packages...".into(), fraction: Some(0.7) });
    run_command(Command::new(venv_python()).args([
        "-m", "uv", "pip", "install", "safetensors",
        "opencv-python", "numpy", "Pillow", "kornia", "timm", "einops", "setuptools",
        "--quiet",
    ]))?;

    report(Progress { message: "Installing FFmpeg bindings...".into(), fraction: Some(0.85) });
    run_command(Command::new(venv_python())
        .args(["-m", "uv", "pip", "install", "imageio-ffmpeg", "transformers", "--quiet"]))?;

    report(Progress { message: "Environment ready".into(), fraction: Some(1.0) });
    Ok(())
}

pub fn create_env(report: impl Fn(Progress)) -> Result<(), String> {
    if is_ready() {
        report(Progress { message: "Already exists".into(), fraction: Some(1.0) });
        return Ok(());
    }
    report(Progress { message: "Creating virtual environment...".into(), fraction: Some(0.0) });
    run_command(Command::new(python_bin()).args(["-m", "venv"]).arg(paths::venv_dir()))?;
    report(Progress { message: "Done".into(), fraction: Some(1.0) });
    Ok(())
}

pub fn install_packages(report: impl Fn(Progress)) -> Result<(), String> {
    report(Progress { message: "Installing uv...".into(), fraction: Some(0.0) });
    run_command(Command::new(venv_python()).args(["-m", "pip", "install", "uv", "--quiet"]))?;

    let gpu = detect_gpu_vendor();
    report(Progress {
        message: format!("Detected GPU: {} — installing PyTorch...", gpu),
        fraction: Some(0.2),
    });
    install_torch_for_gpu(gpu, &report)?;

    report(Progress { message: "Installing packages...".into(), fraction: Some(0.7) });
    run_command(Command::new(venv_python()).args([
        "-m", "uv", "pip", "install",
        "opencv-python", "numpy", "Pillow", "kornia",
        "imageio-ffmpeg", "transformers", "--quiet", "einops", "timm", "safetensors", "setuptools",
    ]))?;
    report(Progress { message: "Done".into(), fraction: Some(1.0) });
    Ok(())
}

pub fn install_uv(report: impl Fn(Progress)) -> Result<(), String> {
    report(Progress { message: "Installing uv...".into(), fraction: Some(0.0) });
    run_command(Command::new(venv_python()).args(["-m", "pip", "install", "uv", "--quiet"]))?;
    report(Progress { message: "Done".into(), fraction: Some(1.0) });
    Ok(())
}

pub fn install_ffmpeg(report: impl Fn(Progress)) -> Result<(), String> {
    report(Progress { message: "Installing FFmpeg bindings...".into(), fraction: Some(0.0) });
    run_command(Command::new(venv_python()).args([
        "-m", "uv", "pip", "install", "safetensors",
        "torch", "torchvision", "opencv-python", "numpy", "Pillow", "kornia",
        "imageio-ffmpeg", "transformers", "--quiet", "einops", "timm", "setuptools",
    ]))?;
    report(Progress { message: "Done".into(), fraction: Some(1.0) });
    Ok(())
}

fn run_command(cmd: &mut Command) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let output = cmd.output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}