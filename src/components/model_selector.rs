use kairos::*;
use kairos::column;
use kairos::row;
use kairos::dropdown;
use crate::setup::paths;

#[component]
pub struct ModelSelector {
    selected: Signal<Option<String>>,
    open: Signal<bool>,
    screen_color: String,
    options: Signal<Vec<String>>,
}

impl ModelSelector {
    pub fn new() -> Self {
        let options: Signal<Vec<String>> = Signal::new(Vec::new());
        let selected: Signal<Option<String>> = Signal::new(None);

        // Background thread — rescans every 2s so newly downloaded models appear
        // without restarting the app
        {
            let options  = options.clone();
            let selected = selected.clone();
            std::thread::spawn(move || {
                loop {
                    let found: Vec<String> = std::fs::read_dir(paths::corridor_models_dir())
                        .map(|entries| {
                            entries
                                .filter_map(|e| e.ok())
                                .filter(|e| e.path().is_file())
                                .filter_map(|e| {
                                    e.path()
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                        .map(|s| s.to_string())
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    if found != options.get_clone() {
                        // Auto-select first model if nothing is selected yet
                        if selected.get_clone().is_none() && !found.is_empty() {
                            selected.set(Some(found[0].clone()));
                        }
                        options.set(found);
                    }

                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            });
        }

        ModelSelector {
            selected,
            open: Signal::new(false),
            screen_color: "Green Screen".to_string(),
            options,
        }
    }
}

impl Component for ModelSelector {
    fn build(&self, _palette: &Palette) -> Widget {
        let options = self.options.get_clone();

        let model_size = if options.is_empty() {
            "No models found".to_string()
        } else {
            format!("{} models available", options.len())
        };

        column![
            width: Fill,
            height: Shrink,
            gap: Custom(0.0),
            padding: none(),
            children![
                row![
                    width: Fill,
                    height: Custom(32.0),
                    padding: horizontal(Symmetrical(Custom(10.0))),
                    children![
                        text![
                            content: "MODEL",
                            style: Caption,
                            color: Palette(Secondary),
                        ],
                    ],
                ],
                divider![],
                column![
                    width: Fill,
                    height: Shrink,
                    children![
                        dropdown![
                            selected: self.selected,
                            placeholder: "Select model...",
                            open: self.open,
                            width: Fill,
                            options: options,
                        ],
                    ],
                ],
                divider![],
                row![
                    width: Fill,
                    height: Custom(32.0),
                    padding: horizontal(Symmetrical(Custom(10.0))),
                    gap: Custom(8.0),
                    children![
                        text![
                            content: model_size,
                            style: Caption,
                            color: Palette(Secondary),
                        ],
                        spacer![
                            size: Fill,
                            orientation: Horizontal,
                        ],
                        text![
                            content: self.screen_color.as_str(),
                            style: Caption,
                            color: Palette(Secondary),
                        ],
                    ],
                ],
                divider![],
            ],
        ]
    }
}