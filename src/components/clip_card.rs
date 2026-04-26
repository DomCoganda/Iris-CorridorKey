// ======================
// src\components\clip_card.rs
// ======================

use kairos::*;
use kairos::PaletteColor::{Error, Success};
use kairos::row;

#[derive(Clone, PartialEq)]
pub enum ClipStatus {
    Extract,
    Raw,
    Alpha,
    Ready,
    Done,
    Error,
}

impl ClipStatus {
    pub fn label(&self) -> &'static str {
        match self {
            ClipStatus::Extract => "EXTRACT",
            ClipStatus::Raw     => "RAW",
            ClipStatus::Alpha   => "ALPHA",
            ClipStatus::Ready   => "READY",
            ClipStatus::Done    => "DONE",
            ClipStatus::Error   => "ERROR",
        }
    }
}

#[component]
pub struct ClipCard {
    pub filename: String,
    pub path: String,
    pub status: Signal<ClipStatus>,
    pub selected: Signal<bool>,
    pub work_dir: String,
    pub thumbnail: Signal<String>,
    pub active_work_dir: Signal<String>,
    pub viewer_total_frames: Signal<u32>,
}

impl ClipCard {
    pub fn new(
        filename: String,
        path: String,
        work_dir: String,
        active_work_dir: Signal<String>,
        viewer_total_frames: Signal<u32>,
    ) -> Self {
        ClipCard {
            filename,
            path,
            work_dir,
            status: Signal::new(ClipStatus::Raw),
            selected: Signal::new(false),
            thumbnail: Signal::new(String::new()),
            active_work_dir,
            viewer_total_frames,
        }
    }
}

impl Component for ClipCard {
    fn build(&self, _palette: &Palette) -> Widget {
        let selected = self.selected.get();
        let status = self.status.get_clone();

        let badge_color = match status {
            ClipStatus::Extract => ColorSource::Palette(Accent),
            ClipStatus::Raw     => ColorSource::Palette(Secondary),
            ClipStatus::Alpha   => hex("#4A9EFF", 1.0),
            ClipStatus::Ready   => hex("#FFD700", 1.0),
            ClipStatus::Done    => ColorSource::Palette(Success),
            ClipStatus::Error   => ColorSource::Palette(Error),
        };

        let bg = if selected {
            ColorSource::Palette(Primary)
        } else {
            hex("#000020", 1.0)
        };

        let selected_signal = self.selected.clone();
        let active_work_dir = self.active_work_dir.clone();
        let work_dir = self.work_dir.clone();
        let thumb_path = self.thumbnail.get_clone();
        let viewer_total_frames = self.viewer_total_frames.clone();

        clickable![
            bindings: [
                MouseButton(Left) => {
                    selected_signal.set(true);
                    active_work_dir.set(work_dir.clone());

                    // Count extracted frames so the viewer scrubber works
                    // immediately when switching clips
                    let input_dir = format!("{}/Input", work_dir);
                    let count = std::fs::read_dir(&input_dir)
                        .map(|d| {
                            d.filter_map(|e| e.ok())
                             .filter(|e| {
                                 e.path().extension()
                                  .map(|x| x == "png")
                                  .unwrap_or(false)
                             })
                             .count() as u32
                        })
                        .unwrap_or(0);
                    viewer_total_frames.set(count);
                },
            ],
            row![
                width: Fill,
                height: Custom(52.0),
                padding: Padding::all(Custom(6.0)),
                gap: Custom(8.0),
                background: bg,
                alignment: VerticalAlignment::Center,
                children![
                    image![
                        source: File(thumb_path),
                        width: Custom(64.0),
                        height: Custom(40.0),
                        fit: ImageFit::Cover,
                    ],
                    text![
                        content: self.filename.clone(),
                        style: Caption,
                        color: ColorSource::Palette(TextColor),
                        width: Fill,
                    ],
                    text![
                        content: self.status.get_clone().label(),
                        style: Caption,
                        color: badge_color,
                    ],
                ],
            ]
        ]
    }
}