// ======================
// src\components\queue_panel.rs
// ======================

use kairos::*;
use kairos::column;
use kairos::row;
use crate::components::clip_card::{ClipCard, ClipStatus};
use crate::components::viewer::ViewMode;

#[component]
pub struct QueuePanel {
    pub clips: SignalBuffer<ClipCard>,
    pub alpha_method: Signal<Option<String>>,
    pub footer_status: Signal<String>,
    pub footer_progress: Signal<f32>,
    pub active_work_dir: Signal<String>,
    pub viewer_total_frames: Signal<u32>,
    pub extracting_frames: Signal<bool>,
    pub viewer_hint_path: Signal<String>,
    pub viewer_hint_status: Signal<String>,
    pub viewer_mode: Signal<ViewMode>,
}

impl QueuePanel {
    pub fn new(
        alpha_method: Signal<Option<String>>,
        footer_status: Signal<String>,
        footer_progress: Signal<f32>,
        active_work_dir: Signal<String>,
        viewer_total_frames: Signal<u32>,
        extracting_frames: Signal<bool>,
        viewer_hint_path: Signal<String>,
        viewer_hint_status: Signal<String>,
        viewer_mode: Signal<ViewMode>,
    ) -> Self {
        let clips = SignalBuffer::new(256);

        // ── Restore clips from previous sessions ─────────────────────────────
        // Scan output/ for subdirectories that contain meta.json.
        let output_root = crate::setup::paths::output_dir();
        if let Ok(entries) = std::fs::read_dir(&output_root) {
            let mut dirs: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            dirs.sort_by_key(|e| e.file_name());

            for entry in dirs {
                let work_path = entry.path();
                if !work_path.is_dir() { continue; }

                let meta_file = work_path.join("meta.json");
                if !meta_file.exists() { continue; }

                let content = match std::fs::read_to_string(&meta_file) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let meta: serde_json::Value = match serde_json::from_str(&content) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let filename = meta["filename"].as_str().unwrap_or("").to_string();
                let clip_path = meta["path"].as_str().unwrap_or("").to_string();
                let work_dir_str = work_path.to_string_lossy().to_string();

                if filename.is_empty() { continue; }

                let card = ClipCard::new(
                    filename,
                    clip_path,
                    work_dir_str.clone(),
                    active_work_dir.clone(),
                    viewer_total_frames.clone(),
                );

                // Detect status from what exists on disk
                let output_has_frames = work_path
                    .join("Output").join("Matte")
                    .read_dir()
                    .map(|mut d| d.next().is_some())
                    .unwrap_or(false);
                let hint_exists = work_path.join("AlphaHint").join("alpha_hint.png").exists();
                let input_frames: u32 = work_path
                    .join("Input")
                    .read_dir()
                    .map(|d| {
                        d.filter_map(|e| e.ok())
                            .filter(|e| e.path().extension().map(|x| x == "png").unwrap_or(false))
                            .count() as u32
                    })
                    .unwrap_or(0);

                card.status.set(if output_has_frames {
                    ClipStatus::Done
                } else if hint_exists {
                    ClipStatus::Ready
                } else if input_frames > 0 {
                    ClipStatus::Raw
                } else {
                    ClipStatus::Raw
                });

                // Restore thumbnail
                let thumb = work_path.join("thumb.png");
                if thumb.exists() {
                    card.thumbnail.set(thumb.to_string_lossy().to_string());
                }

                // Make the first restored clip active, and load its state
                // into the viewer so it's usable immediately
                if active_work_dir.get_clone().is_empty() {
                    active_work_dir.set(work_dir_str.clone());
                    viewer_total_frames.set(input_frames);

                    // Restore hint into viewer if it exists
                    if hint_exists {
                        let hint_path = work_path.join("AlphaHint").join("alpha_hint.png");
                        viewer_hint_path.set(hint_path.to_string_lossy().to_string());
                        viewer_hint_status.set("✓ HINT SET".to_string());
                        viewer_mode.set(ViewMode::Matte);
                    }
                }

                clips.push(card);
            }
        }

        QueuePanel {
            clips,
            alpha_method,
            footer_status,
            footer_progress,
            active_work_dir,
            viewer_total_frames,
            extracting_frames,
            viewer_hint_path,
            viewer_hint_status,
            viewer_mode,
        }
    }
}

impl Component for QueuePanel {
    fn build(&self, palette: &Palette) -> Widget {
        let clips = self.clips.read();
        let count = clips.len();
        let clips_buffer = self.clips.clone();
        let alpha_method = self.alpha_method.clone();
        let active_work_dir = self.active_work_dir.clone();
        let viewer_total_frames = self.viewer_total_frames.clone();
        let footer_status = self.footer_status.clone();
        let footer_progress = self.footer_progress.clone();
        let extracting_frames = self.extracting_frames.clone();
        let viewer_hint_path = self.viewer_hint_path.clone();
        let viewer_hint_status = self.viewer_hint_status.clone();
        let viewer_mode = self.viewer_mode.clone();

        let card_widgets: Vec<Widget> = clips
            .iter()
            .map(|card| card.build(palette))
            .collect();

        column![
            width: Fill,
            height: Fill,
            scrollable: false,
            gap: Custom(0.0),
            padding: Padding::none(),
            children![
                row![
                    width: Fill,
                    height: Custom(32.0),
                    padding: Padding::horizontal(Symmetrical(Custom(10.0))),
                    children![
                        text![
                            content: "QUEUE",
                            style: Caption,
                            color: Palette(Secondary),
                        ],
                        spacer![ size: Fill, orientation: Orientation::Horizontal, ],
                        text![
                            content: format!("{}", count),
                            style: Caption,
                            color: Palette(Accent),
                        ],
                    ],
                ],
                divider![],
                column![
                    width: Fill,
                    height: Fill,
                    gap: Custom(2.0),
                    padding: Padding::all(Custom(6.0)),
                    children: card_widgets,
                ],
                spacer![ size: Fill, orientation: Orientation::Vertical, ],
                divider![],
                button![
                    label: "+ ADD CLIP",
                    width: Fill,
                    height: Custom(36.0),
                    style: widget_style![
                        background: hex("#000015", 1.0),
                        text_color: Palette(Secondary),
                        border: border![thickness: 0.0],
                    ],
                    on_press: {
                        let clips_buffer = clips_buffer.clone();
                        let alpha_method = alpha_method.clone();
                        let active_work_dir = active_work_dir.clone();
                        let viewer_total_frames = viewer_total_frames.clone();
                        let footer_status = footer_status.clone();
                        let footer_progress = footer_progress.clone();
                        let extracting_frames = extracting_frames.clone();
                        let viewer_hint_path = viewer_hint_path.clone();
                        let viewer_hint_status = viewer_hint_status.clone();
                        let viewer_mode = viewer_mode.clone();

                        std::thread::spawn(move || {
                            let files = rfd::FileDialog::new()
                                .add_filter("Video", &["mp4", "mov", "avi", "mkv", "webm"])
                                .pick_files();

                            if let Some(paths) = files {
                                for path in paths {
                                    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                                    let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                                    let work_dir = crate::setup::paths::work_dir_for(&stem).to_string_lossy().to_string();
                                    let _original_path = path.to_string_lossy().to_string();

                                    std::fs::create_dir_all(&work_dir).ok();

                                    // ── Copy video into input/ so it persists ────────────
                                    // We store the copy's path in meta.json so the app
                                    // can always find the video regardless of where the
                                    // user's original file was.
                                    let dest = crate::setup::paths::input_dir().join(&name);
                                    if !dest.exists() {
                                        if let Err(e) = std::fs::copy(&path, &dest) {
                                            eprintln!("[iris] failed to copy clip to input/: {}", e);
                                        }
                                    }
                                    let stored_path = dest.to_string_lossy().to_string();

                                    // ── Write meta.json ──────────────────────────────────
                                    let meta = serde_json::json!({
                                        "filename": name,
                                        "path": stored_path,
                                    });
                                    std::fs::write(
                                        crate::setup::paths::meta_path_for(&stem),
                                        serde_json::to_string_pretty(&meta).unwrap_or_default(),
                                    ).ok();

                                    let card = ClipCard::new(
                                        name,
                                        stored_path.clone(),
                                        work_dir.clone(),
                                        active_work_dir.clone(),
                                        viewer_total_frames.clone(),
                                    );

                                    let status_signal = card.status.clone();
                                    let thumbnail_signal = card.thumbnail.clone();

                                    clips_buffer.push(card);
                                    status_signal.set(ClipStatus::Extract);

                                    if active_work_dir.get_clone().is_empty() {
                                        active_work_dir.set(work_dir.clone());
                                    }

                                    // ── Step 1: thumbnail ────────────────────────────────
                                    let thumb_path = format!("{}/thumb.png", work_dir);
                                    if let Some(ffmpeg) = crate::setup::paths::ffmpeg_exe() {
                                        match std::process::Command::new(&ffmpeg)
                                            .args(["-i", &stored_path, "-frames:v", "1", "-q:v", "2", &thumb_path, "-y"])
                                            .output()
                                        {
                                            Ok(out) if out.status.success() => {
                                                thumbnail_signal.set(thumb_path);
                                            }
                                            Ok(out) => eprintln!("[iris] thumbnail failed:\n{}", String::from_utf8_lossy(&out.stderr)),
                                            Err(e) => eprintln!("[iris] ffmpeg error: {}", e),
                                        }
                                    }

                                    // ── Step 2: extract frames ───────────────────────────
                                    let fs = footer_status.clone();
                                    let fp = footer_progress.clone();
                                    let vtf = viewer_total_frames.clone();
                                    let ef = extracting_frames.clone();
                                    let am = alpha_method.clone();
                                    let wd = work_dir.clone();
                                    let ss = status_signal.clone();
                                    let vhp = viewer_hint_path.clone();
                                    let vhs = viewer_hint_status.clone();
                                    let vm = viewer_mode.clone();

                                    ef.set(true);

                                    crate::bridge::run_extract(
                                        stored_path,
                                        work_dir,
                                        move |event| match event {
                                            crate::bridge::BridgeEvent::Status(msg) => {
                                                fs.set(msg);
                                            }
                                            crate::bridge::BridgeEvent::FrameCount(count) => {
                                                vtf.set(count);
                                            }
                                            crate::bridge::BridgeEvent::Progress { current, total, message } => {
                                                ss.set(ClipStatus::Extract);
                                                fs.set(message);
                                                fp.set(current as f32 / total as f32);
                                            }
                                            crate::bridge::BridgeEvent::Done => {
                                                ef.set(false);
                                                ss.set(ClipStatus::Alpha);
                                                fs.set("Generating alpha hint...".to_string());
                                                fp.set(0.0);

                                                // ── Step 3: auto-generate hint on frame 0 ───
                                                let model = am.get_clone()
                                                    .map(|n| crate::setup::paths::alpha_model_path(&n))
                                                    .unwrap_or_default();

                                                let vhp2 = vhp.clone();
                                                let vhs2 = vhs.clone();
                                                let vm2 = vm.clone();
                                                let fs2 = fs.clone();
                                                let fp2 = fp.clone();
                                                let ss2 = ss.clone();

                                                vhs.set("GENERATING...".to_string());

                                                crate::bridge::run_hint(
                                                    wd.clone(),
                                                    0,
                                                    model,
                                                    move |event| match event {
                                                        crate::bridge::BridgeEvent::HintReady(path) => {
                                                            vhp2.set(path);
                                                            vhs2.set("✓ HINT SET".to_string());
                                                            vm2.set(ViewMode::Matte);
                                                            ss2.set(ClipStatus::Ready);
                                                            fs2.set("Ready — adjust settings and run inference".to_string());
                                                            fp2.set(1.0);
                                                        }
                                                        crate::bridge::BridgeEvent::Status(msg) => {
                                                            fs2.set(msg);
                                                        }
                                                        crate::bridge::BridgeEvent::Error(msg) => {
                                                            eprintln!("[iris] auto-hint error: {}", msg);
                                                            vhs2.set("ERROR".to_string());
                                                            fs2.set("Hint generation failed".to_string());
                                                        }
                                                        _ => {}
                                                    },
                                                );
                                            }
                                            crate::bridge::BridgeEvent::Error(msg) => {
                                                ef.set(false);
                                                eprintln!("[iris] extract error: {}", msg);
                                                ss.set(ClipStatus::Error);
                                                fs.set("Extraction failed".to_string());
                                                fp.set(0.0);
                                            }
                                            _ => {}
                                        },
                                    );
                                }
                            }
                        });
                    },
                ],
            ],
        ]
    }
}