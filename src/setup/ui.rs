use std::sync::{Arc, Mutex};
use kairos::*;
use kairos::column;
use kairos::row;
use super::{SetupStep, StepState};
use crate::Page;
use crate::components::step_row::SetupStepRow;

pub struct SetupScreen {
    pub steps: Arc<Mutex<Vec<SetupStep>>>,
    pub active_page: Signal<Page>,
}

impl Component for SetupScreen {
    fn build(&self, palette: &Palette) -> Widget {
        let active_page = self.active_page.clone();
        let steps = self.steps.lock().unwrap();

        let total = steps.len() as f32;
        let completed = steps.iter()
            .filter(|s| matches!(s.state, StepState::Completed))
            .count() as f32;
        let progress = completed / total;

        let current_step = steps.iter()
            .find(|s| matches!(s.state, StepState::Active))
            .map(|s| s.title.clone())
            .unwrap_or_else(|| if completed == total {
                "Ready to launch".to_string()
            } else {
                "Starting...".to_string()
            });

        let setup_done = completed == total;
        let step_rows: Vec<Widget> = steps.iter().map(|step| {
            SetupStepRow {
                title: step.title.clone(),
                status: step.status.clone(),
                state: step.state.clone(),
                progress: step.progress,
            }.build(palette)
        }).collect();

         row![
             width: Fill,
             height: Fill,
             background: Palette(Background),
             gap: Custom(0.0),
             children![
                 spacer![
                     size: Fill,
                     orientation: Orientation::Horizontal,
                 ],
                 column![
                     width: Fill,
                     height: Fill,
                     scrollable: false,
                     children![
                         // Header
                         row![
                             width: Fill,
                             height: Custom(56.0),
                             padding: Padding::horizontal(Symmetrical(Custom(20.0))),
                             children![
                                 icon![
                                     source: Path(Embedded("../assets/logo.svg")),
                                     size: Custom(56.0),
                                 ],
                                 spacer![
                                     size: Custom(8.0),
                                     orientation: Orientation::Horizontal,
                                 ],
                                 text![
                                     content: "IRIS",
                                     style: TextStyle::H3,
                                 ],
                                 spacer![
                                     size: Fill,
                                     orientation: Orientation::Horizontal,
                                 ],
                                 column![
                                     width: Shrink,
                                     height: Shrink,
                                     alignment: HorizontalAlignment::Right,
                                     gap: Custom(2.0),
                                     children![
                                         text![
                                             content: "FIRST RUN SETUP".to_string(),
                                             style: TextStyle::Caption,
                                             color: ColorSource::Palette(Secondary),
                                         ],
                                         text![
                                             content: current_step.clone(),
                                             style: TextStyle::Body,
                                         ],
                                     ],
                                 ],
                             ],
                         ],
                         divider![],
                         // Progress bar row
                         row![
                             width: Fill,
                             height: Custom(40.0),
                             padding: Padding::horizontal(Symmetrical(Custom(20.0))),
                             children![
                                 text![
                                     content: if completed == total {
                                         "All done - ready to launch".to_string()
                                     } else {
                                         format!("Setting up your environment")
                                     },
                                     style: TextStyle::Caption,
                                     color: hex(palette.accent.hex.as_str(), 1.0),
                                 ],
                                 spacer![
                                     size: Fill,
                                     orientation: Orientation::Horizontal,
                                 ],
                                 text![
                                     content: format!("{:.0}%", progress * 100.0),
                                     style: TextStyle::Caption,
                                     color: hex(palette.accent.hex.as_str(), 1.0),
                                 ],
                             ],
                         ],
                         divider![],
                         // Step list
                         column![
                            width: Fill,
                            height: Fill,
                            padding: Padding::all(Custom(20.0)),
                            gap: Custom(16.0),
                            scrollable: false,
                            children: step_rows,
                        ],
                        divider![],
                        // Footer
                        row![
                            width: Fill,
                            height: Custom(64.0),
                            padding: Padding::horizontal(Symmetrical(Custom(20.0))),
                            children![
                                text![
                                    content: "GVM and VideoMaMa are downloaded on demand - only when you first select them.",
                                    style: TextStyle::Label,
                                    width: Fill,
                                ],
                                button![
                                    label: text![
                                        content: if setup_done { "Open Iris ->" } else { "Setting up..." },
                                        style: TextStyle::Body,
                                        alignment: HorizontalAlignment::Center,
                                        width: Fill,
                                    ],
                                    height: Custom(40.0),
                                    width: Custom(160.0),
                                    on_press: {
                                        let page = active_page.clone();
                                        if setup_done {
                                            page.set(Page::Main)
                                        }
                                    },
                                    style: widget_style![
                                        background: if setup_done { Accent } else { Secondary },
                                        text_color: Background,
                                        border: border![
                                            radius: BorderRadius::Rounded(8.0),
                                        ],
                                    ],
                                ],
                            ],
                        ],
                        divider![],
                        row![
                            width: Fill,
                            height: Custom(32.0),
                            alignment: Center,
                            children![
                                spacer![
                                    size: Fill,
                                    orientation: Horizontal,
                                ],
                                text![
                                    content: "Powered by CorridorKey - By Corridor Digital",
                                    style: TextStyle::Caption,
                                ],
                                spacer![
                                    size: Fill,
                                    orientation: Horizontal,
                                ],
                            ],
                        ],
                     ],
                 ],
                 spacer![
                     size: Fill,
                     orientation: Orientation::Horizontal,
                 ],
             ],
         ]
    }
}