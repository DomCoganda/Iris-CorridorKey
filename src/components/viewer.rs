// ======================
// src\components\viewer.rs
// ======================

use kairos::*;
use kairos::column;
use kairos::row;

#[derive(Clone, PartialEq, Copy)]
pub enum ViewMode {
    In,
    Fg,
    Matte,
    Comp,
    Proc,
}

impl ViewMode {
    pub fn label(&self) -> &'static str {
        match self {
            ViewMode::In    => "IN",
            ViewMode::Fg    => "FG",
            ViewMode::Matte => "MATTE",
            ViewMode::Comp  => "COMP",
            ViewMode::Proc  => "PROC",
        }
    }

    fn subdir(&self) -> &'static str {
        match self {
            ViewMode::In    => "Input",
            ViewMode::Fg    => "Output/FG",
            ViewMode::Matte => "Output/Matte",
            ViewMode::Comp  => "Output/Comp",
            ViewMode::Proc  => "Output/Processed",
        }
    }
}

#[component]
pub struct ViewerPanel {
    pub mode: Signal<ViewMode>,
    pub current_frame: Signal<u32>,
    pub total_frames: Signal<u32>,
    pub active_work_dir: Signal<String>,
    pub alpha_model: Signal<Option<String>>,
    pub hint_path: Signal<String>,
    pub hint_status: Signal<String>,
    pub playing: Signal<bool>,
}

impl ViewerPanel {
    pub fn new(
        active_work_dir: Signal<String>,
        alpha_model: Signal<Option<String>>,
    ) -> Self {
        let playing       = Signal::new(false);
        let current_frame = Signal::new(0u32);
        let total_frames  = Signal::new(0u32);

        // Playback thread — advances current_frame at ~24fps while playing is true.
        // Stops at the last frame and resets playing to false.
        {
            let playing_t = playing.clone();
            let frame_t   = current_frame.clone();
            let total_t   = total_frames.clone();

            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(42)); // ~24fps

                    if !playing_t.get() {
                        continue;
                    }

                    let total = total_t.get();
                    if total == 0 {
                        continue;
                    }

                    let f    = frame_t.get();
                    let last = total.saturating_sub(1);

                    if f >= last {
                        // Reached the end — stop playback
                        playing_t.set(false);
                        frame_t.set(0);
                    } else {
                        frame_t.set(f + 1);
                    }
                }
            });
        }

        ViewerPanel {
            mode: Signal::new(ViewMode::Comp),
            current_frame,
            total_frames,
            active_work_dir,
            alpha_model,
            hint_path: Signal::new(String::new()),
            hint_status: Signal::new(String::new()),
            playing,
        }
    }
}

impl Component for ViewerPanel {
    fn build(&self, _palette: &Palette) -> Widget {
        let current_mode  = self.mode.get();
        let current_frame = self.current_frame.get();
        let total_frames  = self.total_frames.get();
        let work_dir      = self.active_work_dir.get_clone();
        let hint_status   = self.hint_status.get_clone();
        let hint_path_val = self.hint_path.get_clone();
        let is_generating = hint_status == "GENERATING...";
        let is_playing    = self.playing.get();

        const SKIP_SVG: &str = include_str!("../assets/skip.svg");

        let thumb_path = format!("{}/thumb.png", work_dir);

        let input_path = if total_frames > 0 {
            format!("{}/Input/frame_{:06}.png", work_dir, current_frame + 1)
        } else {
            thumb_path.clone()
        };

        let output_path = if total_frames > 0 {
            // Matte mode: show the BiRefNet hint while no inference output exists yet,
            // fall back to Output/Matte once inference has run
            if current_mode == ViewMode::Matte && !hint_path_val.is_empty() {
                let matte_frame = format!(
                    "{}/Output/Matte/frame_{:06}.png",
                    work_dir, current_frame + 1
                );
                if std::path::Path::new(&matte_frame).exists() {
                    matte_frame
                } else {
                    hint_path_val.clone()
                }
            } else {
                format!(
                    "{}/{}/frame_{:06}.png",
                    work_dir,
                    current_mode.subdir(),
                    current_frame + 1
                )
            }
        } else {
            thumb_path.clone()
        };

        let hint_btn_label = if is_generating {
            "GENERATING HINT..."
        } else if hint_status.starts_with("✓") {
            "HINT SET — CLICK TO UPDATE"
        } else {
            "USE AS ALPHA HINT →"
        };

        let work_dir_for_hint  = work_dir.clone();
        let alpha_model_sig    = self.alpha_model.clone();
        let current_frame_sig  = self.current_frame.clone();
        let hint_path_sig      = self.hint_path.clone();
        let hint_status_sig    = self.hint_status.clone();

        let hint_button = button![
            label: hint_btn_label,
            width: Fill,
            height: Custom(32.0),
            on_press: {
                let work_dir        = work_dir_for_hint.clone();
                let hint_path_sig   = hint_path_sig.clone();
                let hint_status_sig = hint_status_sig.clone();
                let frame           = current_frame_sig.get();
                let model = alpha_model_sig.get_clone()
                    .map(|name| crate::setup::paths::alpha_model_path(&name))
                    .unwrap_or_default();
                hint_status_sig.set("GENERATING...".to_string());
                crate::bridge::run_hint(
                    work_dir,
                    frame,
                    model,
                    move |event| match event {
                        crate::bridge::BridgeEvent::HintReady(path) => {
                            hint_path_sig.set(path);
                            hint_status_sig.set("✓ HINT SET".to_string());
                        }
                        crate::bridge::BridgeEvent::Error(e) => {
                            eprintln!("[iris] hint error: {}", e);
                            hint_status_sig.set("ERROR".to_string());
                        }
                        _ => {}
                    },
                );
            },
            style: widget_style![
                background: if is_generating {
                    hex("#0A0A2A", 0.9)
                } else {
                    hex("#FF5F1F", 0.85)
                },
                text_color: Palette(Text),
                border: border![thickness: 0.0],
            ],
        ];

        let mode_buttons: Vec<Widget> = [
            ViewMode::In,
            ViewMode::Fg,
            ViewMode::Matte,
            ViewMode::Comp,
            ViewMode::Proc,
        ]
            .iter()
            .map(|m| {
                let is_active   = *m == current_mode;
                let mode_signal = self.mode.clone();
                let m           = *m;
                button![
                label: m.label(),
                width: Custom(60.0),
                height: Custom(28.0),
                on_press: { mode_signal.set(m); },
                style: widget_style![
                    background: if is_active {
                        hex("#1E3A5F", 1.0)
                    } else {
                        hex("#000020", 0.7)
                    },
                    text_color: if is_active {
                        ColorSource::Palette(TextColor)
                    } else {
                        ColorSource::Palette(Secondary)
                    },
                    border: border![thickness: 0.0],
                ],
            ]
            })
            .collect();

        let left_pane = stack![
            width: Fill,
            height: Fill,
            children![
                image![
                    source: File(input_path),
                    width: Fill,
                    height: Fill,
                    fit: ImageFit::Contain,
                ],
                column![
                    width: Fill,
                    height: Fill,
                    scrollable: false,
                    gap: Custom(0.0),
                    padding: Padding::none(),
                    children![
                        spacer![size: Fill, orientation: Orientation::Vertical,],
                        hint_button,
                    ],
                ],
            ],
        ];

        let right_pane = stack![
            width: Fill,
            height: Fill,
            children![
                image![
                    source: File(output_path),
                    width: Fill,
                    height: Fill,
                    fit: ImageFit::Contain,
                ],
                column![
                    width: Fill,
                    height: Fill,
                    scrollable: false,
                    gap: Custom(0.0),
                    padding: Padding::none(),
                    children![
                        spacer![size: Fill, orientation: Orientation::Vertical,],
                        row![
                            width: Shrink,
                            height: Custom(32.0),
                            padding: Padding::horizontal(Symmetrical(Custom(4.0))),
                            gap: Custom(2.0),
                            background: hex("#000020", 0.85),
                            alignment: VerticalAlignment::Center,
                            children: mode_buttons,
                        ],
                    ],
                ],
            ],
        ];

        // Signals for playback control buttons
        let playing_btn = self.playing.clone();
        let frame_back10 = self.current_frame.clone();
        let frame_back1  = self.current_frame.clone();
        let frame_fwd1   = self.current_frame.clone();
        let frame_fwd10  = self.current_frame.clone();
        let total_fwd1   = self.total_frames.clone();
        let total_fwd10  = self.total_frames.clone();

        column![
            width: Fill(4),
            height: Fill,
            scrollable: false,
            gap: Custom(0.0),
            children![
                row![
                    width: Fill,
                    height: Fill,
                    gap: Custom(2.0),
                    padding: Padding::all(Custom(8.0)),
                    children![
                        column![
                            width: Fill,
                            height: Fill,
                            scrollable: false,
                            padding: Padding::none(),
                            gap: Custom(0.0),
                            background: hex("#0A0A2A", 1.0),
                            children![left_pane,],
                        ],
                        column![
                            width: Fill,
                            height: Fill,
                            scrollable: false,
                            padding: Padding::none(),
                            gap: Custom(0.0),
                            background: hex("#0A0A2A", 1.0),
                            children![right_pane,],
                        ],
                    ],
                ],
                divider![],
                row![
                    width: Fill,
                    height: Custom(40.0),
                    padding: Padding::horizontal(Symmetrical(Custom(8.0))),
                    alignment: VerticalAlignment::Center,
                    gap: Custom(8.0),
                    children![
                        text![
                            content: format!("{}", current_frame),
                            style: Caption,
                            color: Palette(Secondary),
                        ],
                        scrubber![
                            current_frame: self.current_frame.clone(),
                            total_frames: total_frames,
                            width: Fill,
                        ],
                        text![
                            content: format!("{}", total_frames),
                            style: Caption,
                            color: Palette(Secondary),
                        ],
                    ],
                ],
                row![
                    width: Fill,
                    height: Custom(36.0),
                    padding: Padding::horizontal(Symmetrical(Custom(8.0))),
                    alignment: VerticalAlignment::Center,
                    gap: Custom(8.0),
                    background: hex("#04041E", 1.0),
                    children![
                        spacer![size: Fill, orientation: Orientation::Horizontal,],

                        // Skip back 10
                        clickable![
                            bindings: [
                                MouseButton(Left) => {
                                    let f = frame_back10.get();
                                    frame_back10.set(f.saturating_sub(10));
                                },
                            ],
                            icon![
                                source: IconSource::Raw(SKIP_SVG.to_string()),
                                size: Md,
                                color: Palette(Secondary),
                                rotation: 180.0,
                            ]
                        ],

                        // Back 1
                        clickable![
                            bindings: [
                                MouseButton(Left) => {
                                    let f = frame_back1.get();
                                    frame_back1.set(f.saturating_sub(1));
                                },
                            ],
                            icon![
                                source: IconSource::Raw(icons::TRIANGLE.to_string()),
                                size: Md,
                                color: Palette(Secondary),
                                rotation: 270.0,
                            ]
                        ],

                        // Play / Pause
                        {
                            let play_src = if is_playing {
                                IconSource::Raw(icons::SQUARE.to_string())
                            } else {
                                IconSource::Raw(icons::PLAY.to_string())
                            };
                            clickable![
                                bindings: [
                                    MouseButton(Left) => {
                                        playing_btn.set(!playing_btn.get());
                                    },
                                ],
                                icon![
                                    source: play_src,
                                    size: Md,
                                    color: Palette(Accent),
                                ]
                            ]
                        },

                        // Forward 1
                        clickable![
                            bindings: [
                                MouseButton(Left) => {
                                    let f   = frame_fwd1.get();
                                    let max = total_fwd1.get().saturating_sub(1);
                                    frame_fwd1.set((f + 1).min(max));
                                },
                            ],
                            icon![
                                source: IconSource::Raw(icons::TRIANGLE.to_string()),
                                size: Md,
                                color: Palette(Secondary),
                                rotation: 90.0,
                            ]
                        ],

                        // Skip forward 10
                        clickable![
                            bindings: [
                                MouseButton(Left) => {
                                    let f   = frame_fwd10.get();
                                    let max = total_fwd10.get().saturating_sub(1);
                                    frame_fwd10.set((f + 10).min(max));
                                },
                            ],
                            icon![
                                source: IconSource::Raw(SKIP_SVG.to_string()),
                                size: Md,
                                color: Palette(Secondary),
                            ]
                        ],

                        spacer![size: Fill, orientation: Orientation::Horizontal,],
                    ],
                ],
            ],
        ]
    }
}