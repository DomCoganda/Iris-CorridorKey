use kairos::*;
use kairos::SpinnerMotion;
use kairos::SpinnerShape;
use crate::setup::StepState;

use kairos::column;
use kairos::spacer;
use kairos::icon;
use kairos::PaletteColor::{Error, Success};

pub struct SetupStepRow {
    pub title: String,
    pub status: String,
    pub state: StepState,
    pub progress: Option<f32>,
}

impl Component for SetupStepRow {
    fn build(&self, _palette: &Palette) -> Widget {
        let state_widget: Widget = match self.state {
            StepState::Active => spinner![
                shape: SpinnerShape::Arc,
                motion: SpinnerMotion::Spin,
                size: Custom(60.0),
                color: ColorSource::Palette(Accent),
            ],
            StepState::Waiting => icon![
                source: Path(Embedded("../assets/circle.svg")),
                size: Size::Lg,
                color: ColorSource::Palette(Secondary),
            ],
            StepState::Completed => icon![
                source: Path(Embedded("../assets/check.svg")),
                size: Size::Lg,
                color: ColorSource::Palette(Success),
            ],
            StepState::Failed => icon![
                source: Path(Embedded("../assets/x.svg")),
                size: Size::Lg,
                color: ColorSource::Palette(Error),
            ],
        };

        let status_color = match self.state {
            StepState::Completed => ColorSource::Palette(Success),
            StepState::Failed => ColorSource::Palette(Error),
            _ => ColorSource::Palette(Secondary),
        };

        let is_active = matches!(self.state, StepState::Active);
        let progress = self.progress;

        column![
            width: Fill,
            height: Shrink,
            gap: Custom(0.0),
            children![
                row![
                    width: Fill,
                    height: Shrink,
                    gap: Custom(12.0),
                    alignment: VerticalAlignment::Center,
                    children![
                        state_widget,
                        column![
                            width: Fill,
                            height: Shrink,
                            gap: Custom(4.0),
                            alignment: HorizontalAlignment::Left,
                            children![
                                text![
                                    content: self.title.clone(),
                                    style: TextStyle::Body,
                                    alignment: HorizontalAlignment::Left,
                                ],
                                if !self.status.is_empty() {
                                    text![
                                        content: self.status.clone(),
                                        style: TextStyle::Caption,
                                        color: status_color,
                                        alignment: HorizontalAlignment::Left,
                                    ]
                                } else {
                                    spacer![
                                        size: Custom(1.0),
                                        orientation: Orientation::Vertical,
                                    ]
                                },
                                if is_active {
                                    if let Some(fraction) = progress {
                                        row![
                                            width: Fill,
                                            height: Shrink,
                                            gap: Custom(8.0),
                                            alignment: VerticalAlignment::Center,
                                            children![
                                                progress_bar![
                                                    value: fraction,
                                                    min: 0.0,
                                                    max: 1.0,
                                                    width: Fill,
                                                    height: 3.0,
                                                    color: Palette(Accent),
                                                ],
                                                text![
                                                    content: format!("{:.0}%", fraction * 100.0),
                                                    style: TextStyle::Caption,
                                                    color: ColorSource::Palette(Accent),
                                                ],
                                            ],
                                        ]
                                    } else {
                                        spacer![
                                            size: Custom(1.0),
                                            orientation: Orientation::Vertical,
                                        ]
                                    }
                                } else {
                                    spacer![
                                        size: Custom(1.0),
                                        orientation: Orientation::Vertical,
                                    ]
                                },
                            ],
                        ],
                    ],
                ],
                divider![],
            ],
        ]
    }
}