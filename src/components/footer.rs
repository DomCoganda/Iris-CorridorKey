// ======================
// src\components\footer.rs
// ======================

use kairos::*;

#[component]
pub struct Footer {
    pub status: Signal<String>,
    pub progress: Signal<f32>,
    pub is_running: Signal<bool>,
    pub parallel: Signal<u32>,
    pub green_max: Signal<u32>,
    pub yellow_max: Signal<u32>,
    pub active_work_dir: Signal<String>,
    pub alpha_method: Signal<Option<String>>,
    pub despill: Signal<f32>,
    pub despeckle: Signal<f32>,
    pub refiner: Signal<f32>,
}

impl Footer {
    pub fn new(
        active_work_dir: Signal<String>,
        alpha_method: Signal<Option<String>>,
        despill: Signal<f32>,
        despeckle: Signal<f32>,
        refiner: Signal<f32>,
        status: Signal<String>,
        progress: Signal<f32>,
    ) -> Self {
        let green_max  = Signal::new(1u32);
        let yellow_max = Signal::new(2u32);

        let gm = green_max.clone();
        let ym = yellow_max.clone();

        crate::bridge::run_gpu_info(move |event| {
            if let crate::bridge::BridgeEvent::GpuInfo { green_max: g, yellow_max: y, .. } = event {
                gm.set(g);
                ym.set(y);
            }
        });

        Footer {
            status,
            progress,
            is_running: Signal::new(false),
            parallel: Signal::new(1),
            green_max,
            yellow_max,
            active_work_dir,
            alpha_method,
            despill,
            despeckle,
            refiner,
        }
    }
}

impl Component for Footer {
    fn build(&self, _palette: &Palette) -> Widget {
        let status     = self.status.get_clone();
        let progress   = self.progress.get();
        let is_running = self.is_running.get();
        let parallel   = self.parallel.get();
        let green_max  = self.green_max.get();
        let yellow_max = self.yellow_max.get();

        let btn_label = if is_running { "CANCEL" } else { "RUN INFERENCE" };
        let btn_icon_source = if is_running {
            IconSource::Raw(kairos::icons::SQUARE.to_string())
        } else {
            IconSource::Raw(kairos::icons::PLAY.to_string())
        };
        let status_color = if is_running {
            hex("#60A5FA", 1.0)
        } else {
            hex("#4ADE80", 1.0)
        };

        let number_color = if parallel <= green_max {
            ColorSource::Palette(PaletteColor::Success)
        } else if parallel <= yellow_max {
            ColorSource::Palette(PaletteColor::Warning)
        } else {
            ColorSource::Palette(PaletteColor::Error)
        };

        let parallel_up   = self.parallel.clone();
        let parallel_down = self.parallel.clone();
        let max_parallel  = yellow_max + 1;

        let infer_work_dir  = self.active_work_dir.clone();
        let infer_method    = self.alpha_method.clone();
        let infer_despill   = self.despill.clone();
        let infer_despeckle = self.despeckle.clone();
        let infer_refiner   = self.refiner.clone();
        let infer_parallel  = self.parallel.clone();
        let infer_status    = self.status.clone();
        let infer_progress  = self.progress.clone();
        let infer_running   = self.is_running.clone();

        row![
            width: Fill,
            height: Custom(36.0),
            padding: horizontal(Symmetrical(Custom(14.0))),
            gap: Custom(14.0),
            background: hex("#04041E", 1.0),
            alignment: Center,
            children![
                row![
                    height: Shrink,
                    gap: Custom(6.0),
                    alignment: Center,
                    children![
                        row![
                            width: Custom(6.0),
                            height: Custom(6.0),
                            background: status_color,
                        ],
                        text![
                            content: status,
                            style: Caption,
                            color: Palette(Secondary),
                        ],
                    ],
                ],
                row![
                    width: Custom(220.0),
                    height: Shrink,
                    gap: Custom(8.0),
                    alignment: Center,
                    children![
                        progress_bar![
                            value: progress,
                            min: 0.0,
                            max: 1.0,
                            width: Fill,
                            color: Palette(Accent),
                        ],
                        text![
                            content: if progress >= 1.0 {
                                "Complete".to_string()
                            } else {
                                format!("{:.0}%", progress * 100.0)
                            },
                            style: Caption,
                            color: Palette(Accent),
                        ],
                    ],
                ],
                spacer![ size: Fill, orientation: Horizontal, ],
                text![
                    content: "Powered by CorridorKey",
                    style: Caption,
                    color: Palette(Secondary),
                ],
                spacer![ size: Fill, orientation: Horizontal, ],
                row![
                    height: Custom(25.0),
                    width: Custom(175.0),
                    gap: Custom(4.0),
                    padding: horizontal(Symmetrical(Custom(8.0))),
                    background: hex("#05051E", 1.0),
                    alignment: Center,
                    children![
                        text![
                            content: "PARALLEL",
                            style: Caption,
                            color: Palette(Secondary),
                        ],
                        spacer![ size: Custom(5.0), orientation: Horizontal, ],
                        clickable![
                            bindings: [
                                MouseButton(Left) => {
                                    let v = parallel_down.get();
                                    if v > 1 { parallel_down.set(v - 1); }
                                },
                            ],
                            icon![
                                source: IconSource::Raw(icons::TRIANGLE.to_string()),
                                size: Md,
                                color: Palette(Secondary),
                                rotation: 180.0,
                            ]
                        ],
                        spacer![ size: Custom(5.0), orientation: Horizontal, ],
                        text![
                            content: format!("{}", parallel),
                            style: Body,
                            color: number_color,
                        ],
                        spacer![ size: Custom(5.0), orientation: Horizontal, ],
                        clickable![
                            bindings: [
                                MouseButton(Left) => {
                                    let v = parallel_up.get();
                                    if v < max_parallel { parallel_up.set(v + 1); }
                                },
                            ],
                            icon![
                                source: IconSource::Raw(icons::TRIANGLE.to_string()),
                                size: Md,
                                color: Palette(Secondary),
                                rotation: 0.0,
                            ]
                        ],
                    ],
                ],
                button![
                    label: Both(btn_label, icon![
                        source: btn_icon_source,
                        size: Sm,
                        color: Palette(Text),
                    ]),
                    width: Custom(160.0),
                    height: Custom(25.0),
                    on_press: {
                        let work_dir        = infer_work_dir.get_clone();
                        let already_running = infer_running.get();

                        if already_running {
                            infer_running.set(false);
                            infer_status.set("Cancelled".to_string());
                            infer_progress.set(0.0);
                        } else if work_dir.is_empty() {
                            eprintln!("[iris] run_infer: no active clip selected");
                        } else {
                            let model = infer_method.get_clone()
                                .map(|n| crate::setup::paths::alpha_model_path(&n))
                                .unwrap_or_default();
                            let params = crate::bridge::InferParams {
                                despill:   infer_despill.get(),
                                refiner:   infer_refiner.get(),
                                despeckle: infer_despeckle.get() as u32,
                                workers:   infer_parallel.get(),
                            };
                            let fs = infer_status.clone();
                            let fp = infer_progress.clone();
                            let ir = infer_running.clone();

                            ir.set(true);
                            fs.set("Running inference...".to_string());
                            fp.set(0.0);

                            crate::bridge::run_infer(
                                String::new(),
                                work_dir,
                                model,
                                params,
                                move |event| match event {
                                    crate::bridge::BridgeEvent::Status(msg) => {
                                        fs.set(msg);
                                    }
                                    crate::bridge::BridgeEvent::Progress { current, total, message } => {
                                        fs.set(message);
                                        fp.set(current as f32 / total as f32);
                                    }
                                    crate::bridge::BridgeEvent::Done => {
                                        ir.set(false);
                                        fs.set("Done — switch to COMP or FG to preview".to_string());
                                        fp.set(1.0);
                                    }
                                    crate::bridge::BridgeEvent::Error(msg) => {
                                        ir.set(false);
                                        eprintln!("[iris] inference error: {}", msg);
                                        fs.set(format!("Error: {}", msg));
                                        fp.set(0.0);
                                    }
                                    _ => {}
                                },
                            );
                        }
                    },
                    style: widget_style![
                        background: Palette(Accent),
                        text_color: Palette(Text),
                        border: border![thickness: 0.0,],
                    ],
                ],
            ],
        ]
    }
} 