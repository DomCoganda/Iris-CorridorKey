use std::io::{BufRead, BufReader, Read, Write};
use std::sync::{Arc, Mutex};
use crate::setup::paths;

#[derive(Debug, Clone)]
pub enum BridgeEvent {
    Status(String),
    Progress { current: u32, total: u32, message: String },
    Done,
    HintReady(String),
    FrameCount(u32),
    GpuInfo { vram_gb: f32, name: String, green_max: u32, yellow_max: u32 },
    ServerReady,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct InferParams {
    pub despill: f32,
    pub refiner: f32,
    pub despeckle: u32,
    pub workers: u32,
}

impl Default for InferParams {
    fn default() -> Self {
        Self { despill: 0.5, refiner: 1.0, despeckle: 400, workers: 2 }
    }
}

fn parse_event(line: &str) -> BridgeEvent {
    println!("[iris bridge parse] raw line: {}", line);
    let v: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            println!("[iris bridge parse] bad json: {}", e);
            return BridgeEvent::Error(format!("Bad JSON: {}", line));
        }
    };
    println!("[iris bridge parse] parsed json: {}", v);

    match v["kind"].as_str() {
        Some("status") => {
            let msg = v["message"].as_str().unwrap_or("").to_string();
            println!("[iris bridge event] Status: {}", msg);
            BridgeEvent::Status(msg)
        }
        Some("progress") => {
            let current = v["current"].as_u64().unwrap_or(0) as u32;
            let total   = v["total"].as_u64().unwrap_or(1) as u32;
            let message = v["message"].as_str().unwrap_or("").to_string();
            println!("[iris bridge event] Progress: {}/{} {}", current, total, message);
            BridgeEvent::Progress { current, total, message }
        }
        Some("done") => {
            println!("[iris bridge event] Done");
            BridgeEvent::Done
        }
        Some("hint_ready") => {
            let path = v["path"].as_str().unwrap_or("").to_string();
            println!("[iris bridge event] HintReady: {}", path);
            BridgeEvent::HintReady(path)
        }
        Some("frame_count") => {
            let count = v["count"].as_u64().unwrap_or(0) as u32;
            println!("[iris bridge event] FrameCount: {}", count);
            BridgeEvent::FrameCount(count)
        }
        Some("gpu_info") => {
            let vram_gb    = v["vram_gb"].as_f64().unwrap_or(0.0) as f32;
            let name       = v["name"].as_str().unwrap_or("").to_string();
            let green_max  = v["green_max"].as_u64().unwrap_or(1) as u32;
            let yellow_max = v["yellow_max"].as_u64().unwrap_or(2) as u32;
            println!("[iris bridge event] GpuInfo: {} {:.1}GB green={} yellow={}", name, vram_gb, green_max, yellow_max);
            BridgeEvent::GpuInfo { vram_gb, name, green_max, yellow_max }
        }
        Some("server_ready") => {
            println!("[iris bridge event] ServerReady");
            BridgeEvent::ServerReady
        }
        Some("error") => {
            let msg = v["message"].as_str().unwrap_or("").to_string();
            println!("[iris bridge event] Error: {}", msg);
            BridgeEvent::Error(msg)
        }
        _ => {
            println!("[iris bridge event] Unknown: {}", line);
            BridgeEvent::Error(format!("Unknown event: {}", line))
        }
    }
}

fn python_exe() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    { paths::venv_dir().join("Scripts").join("python.exe") }
    #[cfg(not(target_os = "windows"))]
    { paths::venv_dir().join("bin").join("python3") }
}

fn spawn_bridge(
    args: Vec<String>,
    on_event: impl Fn(BridgeEvent) + Send + Sync + 'static,
) {
    let on_event = Arc::new(on_event);
    std::thread::spawn(move || {
        let python        = python_exe();
        let bridge_script = paths::bridge_script();

        println!("[iris bridge spawn] python={}", python.to_string_lossy());
        println!("[iris bridge spawn] script={}", bridge_script.to_string_lossy());
        println!("[iris bridge spawn] args={:?}", args);

        let mut command = std::process::Command::new(&python);
        command
            .arg(&bridge_script)
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("HF_HUB_OFFLINE", "1")
            .env("HF_HUB_DISABLE_SYMLINKS_WARNING", "1");

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }

        let mut child = match command.spawn() {
            Ok(c) => { println!("[iris bridge spawn] process spawned"); c }
            Err(e) => {
                println!("[iris bridge spawn] failed: {}", e);
                on_event(BridgeEvent::Error(format!("Failed to spawn bridge: {}", e)));
                return;
            }
        };

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        std::thread::spawn(move || {
            println!("[iris bridge stderr] reader started");
            let mut reader = BufReader::new(stderr);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&buf);
                        let line = line.trim_end_matches(['\n', '\r']);
                        if !line.trim().is_empty() {
                            println!("[iris bridge stderr] {}", line);
                        }
                    }
                    Err(e) => { println!("[iris bridge stderr] read error: {}", e); break; }
                }
            }
            println!("[iris bridge stderr] reader ended");
        });

        println!("[iris bridge stdout] reader started");
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    println!("[iris bridge stdout] {}", line);
                    on_event(parse_event(&line));
                }
                Err(e) => {
                    println!("[iris bridge stdout] read error: {}", e);
                    on_event(BridgeEvent::Error(format!("Bridge stdout read error: {}", e)));
                    break;
                }
            }
        }

        println!("[iris bridge stdout] reader ended");
        match child.wait() {
            Ok(status) => println!("[iris bridge wait] exited with {}", status),
            Err(e) => {
                println!("[iris bridge wait] wait failed: {}", e);
                on_event(BridgeEvent::Error(format!("Bridge wait failed: {}", e)));
            }
        }
    });
}

// ---------------------------------------------------------------------------
// PreviewServer
// ---------------------------------------------------------------------------

pub struct PreviewServer {
    stdin: Arc<Mutex<std::process::ChildStdin>>,
    _child: std::process::Child,
}

impl PreviewServer {
    pub fn start(
        work_dir: String,
        alpha_model: String,
        on_event: impl Fn(BridgeEvent) + Send + Sync + 'static,
    ) -> Self {
        let python        = python_exe();
        let bridge_script = paths::bridge_script();
        let ck_src   = paths::corridor_src_dir().to_string_lossy().to_string();
        let ck_model = paths::corridor_models_dir()
            .join("CorridorKey_v1.0.pth")
            .to_string_lossy()
            .to_string();

        println!("[iris preview server] starting");

        let mut command = std::process::Command::new(&python);
        command
            .arg(&bridge_script)
            .args(&[
                "--action",       "preview_server",
                "--work",         &work_dir,
                "--ck-src",       &ck_src,
                "--ck-model",     &ck_model,
                "--alpha-model",  &alpha_model,
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("HF_HUB_OFFLINE", "1")
            .env("HF_HUB_DISABLE_SYMLINKS_WARNING", "1");

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }

        let mut child = command.spawn()
            .expect("failed to spawn preview server");

        let stdin  = Arc::new(Mutex::new(child.stdin.take().unwrap()));
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        std::thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&buf);
                        let line = line.trim_end_matches(['\n', '\r']);
                        if !line.trim().is_empty() {
                            println!("[iris preview server] {}", line);
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let on_event = Arc::new(on_event);
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    println!("[iris preview server stdout] {}", line);
                    on_event(parse_event(&line));
                }
            }
            println!("[iris preview server] stdout closed");
        });

        PreviewServer { stdin, _child: child }
    }

    pub fn request(&self, frame: u32, params: &InferParams) {
        let cmd = format!(
            "{{\"frame\":{},\"despill\":{},\"refiner\":{},\"despeckle\":{}}}\n",
            frame, params.despill, params.refiner, params.despeckle
        );
        if let Ok(mut stdin) = self.stdin.lock() {
            let _ = stdin.write_all(cmd.as_bytes());
            let _ = stdin.flush();
            println!("[iris preview server] sent: {}", cmd.trim());
        }
    }
}

// ---------------------------------------------------------------------------
// Public bridge API
// ---------------------------------------------------------------------------

pub fn run_gpu_info(on_event: impl Fn(BridgeEvent) + Send + Sync + 'static) {
    println!("[iris bridge api] run_gpu_info");
    spawn_bridge(
        vec!["--action".into(), "gpu_info".into(), "--alpha-model".into(), "".into()],
        on_event,
    );
}

pub fn run_extract(
    clip_path: String,
    work_dir: String,
    on_event: impl Fn(BridgeEvent) + Send + Sync + 'static,
) {
    println!("[iris bridge api] run_extract");
    spawn_bridge(vec![
        "--action".into(), "extract".into(),
        "--input".into(), clip_path,
        "--work".into(), work_dir,
        "--alpha-model".into(), "".into(),
    ], on_event);
}

pub fn run_alpha(
    clip_path: String,
    work_dir: String,
    alpha_model: String,
    params: InferParams,
    on_event: impl Fn(BridgeEvent) + Send + Sync + 'static,
) {
    println!("[iris bridge api] run_alpha");
    let ck_src   = paths::corridor_src_dir().to_string_lossy().to_string();
    let ck_model = paths::corridor_models_dir().join("CorridorKey_v1.0.pth").to_string_lossy().to_string();
    spawn_bridge(vec![
        "--action".into(), "alpha".into(),
        "--input".into(), clip_path,
        "--work".into(), work_dir,
        "--ck-src".into(), ck_src,
        "--ck-model".into(), ck_model,
        "--alpha-model".into(), alpha_model,
        "--despill".into(), params.despill.to_string(),
        "--refiner".into(), params.refiner.to_string(),
        "--despeckle".into(), params.despeckle.to_string(),
        "--workers".into(), params.workers.to_string(),
    ], on_event);
}

pub fn run_hint(
    work_dir: String,
    frame: u32,
    alpha_model: String,
    on_event: impl Fn(BridgeEvent) + Send + Sync + 'static,
) {
    println!("[iris bridge api] run_hint");
    spawn_bridge(vec![
        "--action".into(), "hint".into(),
        "--work".into(), work_dir,
        "--alpha-model".into(), alpha_model,
        "--frame".into(), frame.to_string(),
    ], on_event);
}

pub fn run_infer(
    clip_path: String,
    work_dir: String,
    alpha_model: String,
    params: InferParams,
    on_event: impl Fn(BridgeEvent) + Send + Sync + 'static,
) {
    println!("[iris bridge api] run_infer");
    let ck_src   = paths::corridor_src_dir().to_string_lossy().to_string();
    let ck_model = paths::corridor_models_dir().join("CorridorKey_v1.0.pth").to_string_lossy().to_string();
    spawn_bridge(vec![
        "--action".into(), "infer".into(),
        "--input".into(), clip_path,
        "--work".into(), work_dir,
        "--ck-src".into(), ck_src,
        "--ck-model".into(), ck_model,
        "--alpha-model".into(), alpha_model,
        "--despill".into(), params.despill.to_string(),
        "--refiner".into(), params.refiner.to_string(),
        "--despeckle".into(), params.despeckle.to_string(),
        "--workers".into(), params.workers.to_string(),
    ], on_event);
}

pub fn run_preview(
    work_dir: String,
    frame: u32,
    alpha_model: String,
    params: InferParams,
    on_event: impl Fn(BridgeEvent) + Send + Sync + 'static,
) {
    println!("[iris bridge api] run_preview");
    let ck_src   = paths::corridor_src_dir().to_string_lossy().to_string();
    let ck_model = paths::corridor_models_dir().join("CorridorKey_v1.0.pth").to_string_lossy().to_string();
    spawn_bridge(vec![
        "--action".into(), "preview".into(),
        "--work".into(), work_dir,
        "--ck-src".into(), ck_src,
        "--ck-model".into(), ck_model,
        "--alpha-model".into(), alpha_model,
        "--frame".into(), frame.to_string(),
        "--despill".into(), params.despill.to_string(),
        "--refiner".into(), params.refiner.to_string(),
        "--despeckle".into(), params.despeckle.to_string(),
        "--workers".into(), "1".into(),
    ], on_event);
}

pub fn run_export(
    work_dir: String,
    fps: f32,
    on_event: impl Fn(BridgeEvent) + Send + Sync + 'static,
) {
    println!("[iris bridge api] run_export");
    println!("[iris bridge api] work_dir={}", work_dir);
    println!("[iris bridge api] fps={}", fps);

    spawn_bridge(
        vec![
            "--action".into(),      "export".into(),
            "--work".into(),        work_dir,
            "--fps".into(),         fps.to_string(),
            "--alpha-model".into(), "".into(),
        ],
        on_event,
    );
}