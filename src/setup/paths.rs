use std::path::PathBuf;

/// App root.
///
/// Debug:
///   <project>/iris_data/
///
/// Release/shipped:
///   folder beside Iris.exe
///
/// Final visible layout:
///   input/
///   output/
///   models/
///   runtime/   <- app-owned runtime, not user content
pub fn iris_dir() -> PathBuf {
    #[cfg(debug_assertions)]
    {
        std::env::var("CARGO_MANIFEST_DIR")
            .map(|d| PathBuf::from(d).join("iris_data"))
            .unwrap_or_else(|_| exe_dir())
    }

    #[cfg(not(debug_assertions))]
    {
        exe_dir()
    }
}

fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .expect("Could not find executable path")
        .parent()
        .expect("Executable has no parent directory")
        .to_path_buf()
}

// -----------------------------------------------------------------------------
// User-facing folders
// -----------------------------------------------------------------------------

pub fn input_dir() -> PathBuf {
    iris_dir().join("input")
}

pub fn output_dir() -> PathBuf {
    iris_dir().join("output")
}

pub fn models_dir() -> PathBuf {
    iris_dir().join("models")
}

pub fn alpha_models_dir() -> PathBuf {
    models_dir().join("alpha_models")
}

pub fn corridor_models_dir() -> PathBuf {
    models_dir().join("key_models")
}

/// Per-clip output/work folder.
///
/// Example:
///   output/my_clip/Input
///   output/my_clip/AlphaHint
///   output/my_clip/Output/Matte
///   output/my_clip/Output/FG
///   output/my_clip/Output/Comp
///   output/my_clip/Output/Processed
pub fn work_dir_for(clip_stem: &str) -> PathBuf {
    output_dir().join(clip_stem)
}

/// Metadata file inside each clip's work dir.
/// Written on first add so the queue can be restored on next launch.
///
/// Example: output/my_clip/meta.json
pub fn meta_path_for(clip_stem: &str) -> PathBuf {
    work_dir_for(clip_stem).join("meta.json")
}

// -----------------------------------------------------------------------------
// App-owned runtime folders
// -----------------------------------------------------------------------------

pub fn runtime_dir() -> PathBuf {
    iris_dir().join("runtime")
}

/// Kept for compatibility with older code.
/// This is NOT `.iris_internal` anymore.
pub fn internal_dir() -> PathBuf {
    runtime_dir()
}

pub fn bridge_script() -> PathBuf {
    runtime_dir().join("iris_bridge.py")
}

pub fn python_dir() -> PathBuf {
    runtime_dir().join("python")
}

pub fn venv_dir() -> PathBuf {
    runtime_dir().join("venv")
}

pub fn corridor_src_dir() -> PathBuf {
    runtime_dir().join("corridor")
}

pub fn venv_python() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        venv_dir().join("Scripts").join("python.exe")
    }

    #[cfg(not(target_os = "windows"))]
    {
        venv_dir().join("bin").join("python3")
    }
}

pub fn ffmpeg_exe() -> Option<PathBuf> {
    let output = std::process::Command::new(venv_python())
        .args(["-c", "import imageio_ffmpeg; print(imageio_ffmpeg.get_ffmpeg_exe())"])
        .output()
        .ok()?;

    if output.status.success() {
        let path = String::from_utf8(output.stdout).ok()?;
        Some(PathBuf::from(path.trim()))
    } else {
        None
    }
}

pub fn alpha_model_path(name: &str) -> String {
    let dir = alpha_models_dir().join(name);

    if dir.exists() {
        dir.to_string_lossy().to_string()
    } else {
        alpha_models_dir()
            .join(format!("{}.pth", name))
            .to_string_lossy()
            .to_string()
    }
}