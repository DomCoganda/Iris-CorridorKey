// ======================
// src\main_ui.rs
// ======================

use kairos::*;
use kairos::column;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::components::header::Header;
use crate::components::model_selector::ModelSelector;
use crate::components::queue_panel::QueuePanel;
use crate::components::viewer::ViewerPanel;
use crate::components::controls::ControlsPanel;
use crate::components::footer::Footer;

pub struct MainScreen {
    pub header: Header,
    pub model_selector: ModelSelector,
    pub queue_panel: QueuePanel,
    pub viewer: ViewerPanel,
    pub controls: ControlsPanel,
    pub footer: Footer,
}

impl MainScreen {
    pub fn new() -> Self {
        let active_work_dir: Signal<String> = Signal::new(String::new());
        let extracting_frames: Signal<bool> = Signal::new(false);

        // Pre-create the shared status/progress signals so both footer and
        // controls write to the same signals — no circular dependency needed.
        let shared_status:   Signal<String> = Signal::new("Ready".to_string());
        let shared_progress: Signal<f32>    = Signal::new(0.0);

        let controls = ControlsPanel::new(
            active_work_dir.clone(),
            shared_status.clone(),
            shared_progress.clone(),
        );

        // app_footer instead of footer to avoid conflict with the footer! macro
        let app_footer = Footer::new(
            active_work_dir.clone(),
            controls.method.clone(),
            controls.despill.clone(),
            controls.despeckle.clone(),
            controls.refiner.clone(),
            shared_status.clone(),
            shared_progress.clone(),
        );

        let viewer = ViewerPanel::new(
            active_work_dir.clone(),
            controls.method.clone(),
        );

        let queue_panel = QueuePanel::new(
            controls.method.clone(),
            shared_status.clone(),
            shared_progress.clone(),
            active_work_dir.clone(),
            viewer.total_frames.clone(),
            extracting_frames,
            viewer.hint_path.clone(),
            viewer.hint_status.clone(),
            viewer.mode.clone(),
        );

        // ── Live preview thread ───────────────────────────────────────────────
        {
            let live_preview  = controls.live_preview.clone();
            let despill       = controls.despill.clone();
            let despeckle     = controls.despeckle.clone();
            let refiner       = controls.refiner.clone();
            let work_dir      = active_work_dir.clone();
            let hint_path     = viewer.hint_path.clone();
            let method        = controls.method.clone();
            let current_frame = viewer.current_frame.clone();
            let footer_status = shared_status.clone();

            std::thread::spawn(move || {
                let server: Arc<Mutex<Option<crate::bridge::PreviewServer>>> =
                    Arc::new(Mutex::new(None));

                let processing = Arc::new(AtomicBool::new(false));

                let mut last_despill   = despill.get();
                let mut last_despeckle = despeckle.get();
                let mut last_refiner   = refiner.get();
                let mut last_frame     = current_frame.get();

                let pending: Arc<Mutex<Option<(u32, crate::bridge::InferParams)>>> =
                    Arc::new(Mutex::new(None));

                loop {
                    std::thread::sleep(std::time::Duration::from_millis(50));

                    let preview_on = live_preview.get();
                    let work = work_dir.get_clone();
                    let hint = hint_path.get_clone();

                    if !preview_on {
                        let mut srv = server.lock().unwrap();
                        if srv.is_some() {
                            *srv = None;
                            footer_status.set("Live preview off".to_string());
                        }
                        continue;
                    }

                    if work.is_empty() || hint.is_empty() {
                        continue;
                    }

                    {
                        let mut srv = server.lock().unwrap();
                        if srv.is_none() {
                            footer_status.set("Loading preview server...".to_string());

                            let model = method.get_clone()
                                .map(|n| crate::setup::paths::alpha_model_path(&n))
                                .unwrap_or_default();

                            let proc_flag = processing.clone();
                            let pend      = pending.clone();
                            let srv_ref   = server.clone();
                            let fs        = footer_status.clone();

                            let new_server = crate::bridge::PreviewServer::start(
                                work.clone(),
                                model,
                                move |event| {
                                    match event {
                                        crate::bridge::BridgeEvent::ServerReady => {
                                            fs.set("Preview server ready".to_string());
                                        }
                                        crate::bridge::BridgeEvent::Status(msg) => {
                                            fs.set(msg);
                                        }
                                        crate::bridge::BridgeEvent::Done => {
                                            proc_flag.store(false, Ordering::SeqCst);
                                            fs.set("Preview ready".to_string());
                                            if let Some((frame, params)) =
                                                pend.lock().unwrap().take()
                                            {
                                                if let Ok(srv) = srv_ref.lock() {
                                                    if let Some(s) = srv.as_ref() {
                                                        proc_flag.store(true, Ordering::SeqCst);
                                                        s.request(frame, &params);
                                                    }
                                                }
                                            }
                                        }
                                        crate::bridge::BridgeEvent::Error(e) => {
                                            proc_flag.store(false, Ordering::SeqCst);
                                            fs.set(format!("Preview error: {}", e));
                                        }
                                        _ => {}
                                    }
                                },
                            );

                            *srv = Some(new_server);
                            last_despill   = despill.get();
                            last_despeckle = despeckle.get();
                            last_refiner   = refiner.get();
                            last_frame     = current_frame.get();
                        }
                    }

                    let d  = despill.get();
                    let de = despeckle.get();
                    let r  = refiner.get();
                    let f  = current_frame.get();

                    let changed = d != last_despill || de != last_despeckle
                        || r != last_refiner || f != last_frame;

                    if changed {
                        last_despill   = d;
                        last_despeckle = de;
                        last_refiner   = r;
                        last_frame     = f;

                        let params = crate::bridge::InferParams {
                            despill:   d,
                            refiner:   r,
                            despeckle: de as u32,
                            workers:   1,
                        };

                        if processing.load(Ordering::SeqCst) {
                            *pending.lock().unwrap() = Some((f, params));
                        } else {
                            let srv = server.lock().unwrap();
                            if let Some(s) = srv.as_ref() {
                                processing.store(true, Ordering::SeqCst);
                                footer_status.set(format!("Preview frame {}...", f + 1));
                                s.request(f, &params);
                            }
                        }
                    }
                }
            });
        }
        // ─────────────────────────────────────────────────────────────────────

        MainScreen {
            header: Header::new(),
            model_selector: ModelSelector::new(),
            queue_panel,
            viewer,
            controls,
            footer: app_footer,
        }
    }
}

impl Component for MainScreen {
    fn build(&self, palette: &Palette) -> Widget {
        column![
            width: Fill,
            height: Fill,
            background: Palette(Background),
            scrollable: false,
            children![
                self.header.build(palette),
                divider![],
                row![
                    width: Fill,
                    height: Fill,
                    padding: Padding::none(),
                    children![
                        column![
                            width: Fill,
                            height: Fill,
                            scrollable: false,
                            padding: Padding::none(),
                            children![
                                self.model_selector.build(palette),
                                self.queue_panel.build(palette),
                            ],
                        ],
                        self.viewer.build(palette),
                        self.controls.build(palette),
                    ],
                ],
                self.footer.build(palette),
            ],
        ]
    }
}