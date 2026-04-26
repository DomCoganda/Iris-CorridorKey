// ======================
// src\components\controls.rs
// ======================

use kairos::*;
use kairos::column;
use kairos::row;
use std::io::Read;

const AVAILABLE_ALPHA_MODELS: &[(&str, &str)] = &[
    ("BiRefNet", "https://huggingface.co/ZhengPeng7/BiRefNet/resolve/main/model.pth"),
    ("GMV Auto", "https://huggingface.co/nikopueringer/CorridorKey/resolve/main/gmvauto.pth"),
    ("VideoMaMa", "https://huggingface.co/nikopueringer/CorridorKey/resolve/main/videomama.pth"),
    ("MatAnyone2", "https://huggingface.co/nikopueringer/CorridorKey/resolve/main/matanyone2.pth"),
];

#[component]
pub struct ControlsPanel {
    pub threshold: Signal<f32>,
    pub despill: Signal<f32>,
    pub despeckle: Signal<f32>,
    pub despeckle_enabled: Signal<bool>,
    pub refiner: Signal<f32>,
    pub live_preview: Signal<bool>,
    pub method: Signal<Option<String>>,
    pub method_options: Signal<Vec<String>>,
    pub color_space: Signal<Option<String>>,
    pub output_fg: Signal<bool>,
    pub output_matte: Signal<bool>,
    pub output_comp: Signal<bool>,
    pub output_processed: Signal<bool>,
    pub method_open: Signal<bool>,
    pub color_space_open: Signal<bool>,
    pub active_work_dir: Signal<String>,
    pub footer_status: Signal<String>,
    pub footer_progress: Signal<f32>,
}

impl ControlsPanel {
    pub fn new(
        active_work_dir: Signal<String>,
        footer_status: Signal<String>,
        footer_progress: Signal<f32>,
    ) -> Self {
        let method_options: Vec<String> = std::fs::read_dir(crate::setup::paths::alpha_models_dir())
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        let p = e.path();
                        let name = e.file_name();
                        let name_str = name.to_string_lossy();
                        !name_str.starts_with('.')
                            && name_str != "CACHEDIR.TAG"
                            && !name_str.starts_with("models--")
                            && (p.is_dir() || p.extension().map(|x| x == "pth").unwrap_or(false))
                    })
                    .filter_map(|e| {
                        let p = e.path();
                        if p.is_dir() {
                            p.file_name().and_then(|s| s.to_str()).map(|s| s.to_string())
                        } else {
                            p.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let method = if method_options.is_empty() {
            Signal::new(None)
        } else {
            Signal::new(Some(method_options[0].clone()))
        };

        ControlsPanel {
            threshold: Signal::new(0.5),
            despill: Signal::new(1.0),
            despeckle: Signal::new(400.0),
            despeckle_enabled: Signal::new(true),
            refiner: Signal::new(1.0),
            live_preview: Signal::new(false),
            method,
            method_options: Signal::new(method_options),
            color_space: Signal::new(Some("sRGB".to_string())),
            output_fg: Signal::new(true),
            output_matte: Signal::new(true),
            output_comp: Signal::new(true),
            output_processed: Signal::new(true),
            method_open: Signal::new(false),
            color_space_open: Signal::new(false),
            active_work_dir,
            footer_status,
            footer_progress,
        }
    }
}

impl Component for ControlsPanel {
    fn build(&self, _palette: &Palette) -> Widget {
        let despill_val   = self.despill.get();
        let despeckle_val = self.despeckle.get();
        let refiner_val   = self.refiner.get();
        let installed     = self.method_options.get_clone();

        let selected_name = self.method.get_clone();
        let needs_download = selected_name.as_ref().map(|name| {
            !installed.iter().any(|i| i == name)
                && AVAILABLE_ALPHA_MODELS.iter().any(|(n, _)| n == name)
        }).unwrap_or(false);

        let download_buttons: Vec<Widget> = if needs_download {
            let (name, url) = AVAILABLE_ALPHA_MODELS.iter()
                .find(|(n, _)| Some(n.to_string()) == selected_name)
                .unwrap();
            let label = format!("↓ Download {}", name);
            let url = url.to_string();
            let name = name.to_string();
            let method_options = self.method_options.clone();
            let method = self.method.clone();
            vec![button![
                label: label.as_str(),
                width: Fill,
                height: Custom(30.0),
                on_press: {
                    let _url = url.clone();
                    let name = name.clone();
                    let method_options = method_options.clone();
                    let method = method.clone();
                    std::thread::spawn(move || {
                        let dest = crate::setup::paths::alpha_models_dir().join(&name);
                        std::fs::create_dir_all(&dest).ok();
                        for (filename, file_url) in &[
                            ("config.json", format!("https://huggingface.co/ZhengPeng7/{}/resolve/main/config.json", name)),
                            ("pytorch_model.bin", format!("https://huggingface.co/ZhengPeng7/{}/resolve/main/pytorch_model.bin", name)),
                        ] {
                            if let Ok(mut resp) = reqwest::blocking::get(file_url.as_str()) {
                                let mut buf = Vec::new();
                                resp.read_to_end(&mut buf).ok();
                                std::fs::write(dest.join(filename), buf).ok();
                            }
                        }
                        let mut opts = method_options.get_clone();
                        opts.push(name.clone());
                        method_options.set(opts);
                        method.set(Some(name));
                    });
                },
                style: widget_style![
                    background: hex("#000020", 1.0),
                    text_color: Palette(Secondary),
                    border: border![thickness: 1.0, color: ColorSource::Palette(Secondary)],
                ],
            ]]
        } else {
            vec![]
        };

        let mut alpha_children: Vec<Widget> = vec![
            text![
                content: "ALPHA GENERATION",
                style: Caption, color: Palette(Secondary),
            ],
            row![
                width: Fill, height: Shrink, gap: Custom(8.0),
                alignment: VerticalAlignment::Center,
                children![
                    text![
                        content: "METHOD",
                        style: Caption, color: Palette(Secondary),
                    ],
                    dropdown![
                        selected: self.method.clone(),
                        open: self.method_open.clone(),
                        options: self.method_options.get_clone(),
                        placeholder: "Select...",
                        width: Fill,
                    ],
                ],
            ],
        ];

        alpha_children.extend(download_buttons);

        alpha_children.push(text![
            content: "Some features are still under development",
            style: Caption,
            color: Palette(Secondary),
        ]);

        let export_work_dir = self.active_work_dir.clone();
        let export_status   = self.footer_status.clone();
        let export_progress = self.footer_progress.clone();

        column![
            width: Fill,
            height: Fill,
            scrollable: true,
            gap: Custom(0.0),
            padding: Padding::none(),
            children![
                column![
                    width: Fill,
                    height: Shrink,
                    scrollable: false,
                    padding: Padding::all(Custom(10.0)),
                    gap: Custom(8.0),
                    children: alpha_children,
                ],
                divider![],
                column![
                    width: Fill,
                    height: Shrink,
                    scrollable: false,
                    padding: Padding::all(Custom(10.0)),
                    gap: Custom(8.0),
                    children![
                        text![
                            content: "INFERENCE",
                            style: Caption,
                            color: Palette(Secondary),
                        ],
                        row![
                            width: Fill, height: Shrink,
                            alignment: VerticalAlignment::Center,
                            children![
                                text![ content: "LIVE PREVIEW", style: Caption, color: Palette(Secondary), ],
                                spacer![ size: Fill, orientation: Orientation::Horizontal, ],
                                toggle![ state: self.live_preview.clone(), ],
                            ],
                        ],
                        row![
                            width: Fill, height: Shrink, gap: Custom(8.0),
                            alignment: VerticalAlignment::Center,
                            children![
                                text![
                                    content: "COLOR SPACE",
                                    style: Caption, color: Palette(Secondary),
                                ],
                                dropdown![
                                    selected: self.color_space.clone(),
                                    open: self.color_space_open.clone(),
                                    options: vec!["sRGB".to_string(), "Linear".to_string()],
                                    placeholder: "Select...",
                                    width: Fill,
                                ],
                            ],
                        ],
                        row![
                            width: Fill, height: Shrink,
                            children![
                                text![ content: "DESPILL", style: Caption, color: Palette(Secondary), ],
                                spacer![ size: Fill, orientation: Orientation::Horizontal, ],
                                text![ content: format!("{:.1}", despill_val), style: Caption, color: Palette(Accent), ],
                            ],
                        ],
                        slider![ min: 0.0, max: 2.0, step: 0.1, value: self.despill.clone(), ],
                        row![
                            width: Fill, height: Shrink, gap: Custom(6.0),
                            alignment: VerticalAlignment::Center,
                            children![
                                checkbox![ state: self.despeckle_enabled.clone(), ],
                                text![ content: "DESPECKLE", style: Caption, color: Palette(Secondary), ],
                                spacer![ size: Fill, orientation: Orientation::Horizontal, ],
                                text![ content: format!("{:.0}px", despeckle_val), style: Caption, color: Palette(Accent), ],
                            ],
                        ],
                        slider![ min: 0.0, max: 1000.0, step: 1.0, value: self.despeckle.clone(), ],
                        row![
                            width: Fill, height: Shrink,
                            children![
                                text![ content: "REFINER", style: Caption, color: Palette(Secondary), ],
                                spacer![ size: Fill, orientation: Orientation::Horizontal, ],
                                text![ content: format!("{:.1}", refiner_val), style: Caption, color: Palette(Accent), ],
                            ],
                        ],
                        slider![ min: 0.0, max: 2.0, step: 0.1, value: self.refiner.clone(), ],
                    ],
                ],
                divider![],
                column![
                    width: Fill, height: Shrink, scrollable: false,
                    padding: Padding::all(Custom(10.0)), gap: Custom(6.0),
                    children![
                        text![ content: "OUTPUT", style: Caption, color: Palette(Secondary), ],
                        row![
                            width: Fill, height: Shrink,
                            alignment: VerticalAlignment::Center, gap: Custom(6.0),
                            children![
                                checkbox![ state: self.output_fg.clone(), ],
                                text![ content: "FG", style: Caption, color: Palette(Text), ],
                                spacer![ size: Fill, orientation: Orientation::Horizontal, ],
                                text![ content: "mp4", style: Caption, color: Palette(Secondary), ],
                            ],
                        ],
                        row![
                            width: Fill, height: Shrink,
                            alignment: VerticalAlignment::Center, gap: Custom(6.0),
                            children![
                                checkbox![ state: self.output_matte.clone(), ],
                                text![ content: "Matte", style: Caption, color: Palette(Text), ],
                                spacer![ size: Fill, orientation: Orientation::Horizontal, ],
                                text![ content: "mp4", style: Caption, color: Palette(Secondary), ],
                            ],
                        ],
                        row![
                            width: Fill, height: Shrink,
                            alignment: VerticalAlignment::Center, gap: Custom(6.0),
                            children![
                                checkbox![ state: self.output_comp.clone(), ],
                                text![ content: "Comp", style: Caption, color: Palette(Text), ],
                                spacer![ size: Fill, orientation: Orientation::Horizontal, ],
                                text![ content: "mp4", style: Caption, color: Palette(Secondary), ],
                            ],
                        ],
                        row![
                            width: Fill, height: Shrink,
                            alignment: VerticalAlignment::Center, gap: Custom(6.0),
                            children![
                                checkbox![ state: self.output_processed.clone(), ],
                                text![ content: "Processed", style: Caption, color: Palette(Text), ],
                                spacer![ size: Fill, orientation: Orientation::Horizontal, ],
                                text![ content: "mp4", style: Caption, color: Palette(Secondary), ],
                            ],
                        ],
                        button![
                            label: "EXPORT VIDEOS",
                            width: Fill,
                            height: Custom(32.0),
                            on_press: {
                                let work = export_work_dir.get_clone();
                                let es   = export_status.clone();
                                let ep   = export_progress.clone();
                                if work.is_empty() {
                                    es.set("No clip selected".to_string());
                                } else {
                                    es.set("Exporting...".to_string());
                                    ep.set(0.0);
                                    crate::bridge::run_export(
                                        work,
                                        24.0,
                                        move |event| match event {
                                            crate::bridge::BridgeEvent::Status(msg) => {
                                                es.set(msg);
                                            }
                                            crate::bridge::BridgeEvent::Progress { current, total, message } => {
                                                es.set(message);
                                                ep.set(current as f32 / total as f32);
                                            }
                                            crate::bridge::BridgeEvent::Done => {
                                                es.set("✓ Saved to Export/".to_string());
                                                ep.set(1.0);
                                            }
                                            crate::bridge::BridgeEvent::Error(e) => {
                                                es.set(format!("Export error: {}", e));
                                                ep.set(0.0);
                                            }
                                            _ => {}
                                        },
                                    );
                                }
                            },
                            style: widget_style![
                                background: Palette(Accent),
                                text_color: Palette(Text),
                                border: border![thickness: 0.0],
                            ],
                        ],
                    ],
                ],
                divider![],
                spacer![ size: Fill, orientation: Orientation::Vertical, ],
            ],
        ]
    }
}