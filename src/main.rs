// ======================
// src\main.rs
// ======================

use std::sync::{Arc, Mutex};
use kairos::*;
use kairos::column;

mod setup;
#[macro_use]
mod components;
mod main_ui;
mod bridge;

#[derive(Copy,Clone,PartialEq, Debug)]
pub enum Page {
    Setup,
    Main,
}

const IRIS_BRIDGE_PY: &str = include_str!("python/iris_bridge.py");

struct Iris {
    theme: ThemeSet,
    variant: ThemeVariant,
    active_page: Signal<Page>,
    setup_steps: Arc<Mutex<Vec<setup::SetupStep>>>,
    main_screen: main_ui::MainScreen,
}

fn main() {
    use setup::paths;
    std::fs::create_dir_all(paths::input_dir()).ok();
    std::fs::create_dir_all(paths::output_dir()).ok();
    std::fs::create_dir_all(paths::corridor_models_dir()).ok();
    std::fs::create_dir_all(paths::alpha_models_dir()).ok();
    std::fs::create_dir_all(paths::python_dir()).ok();
    std::fs::create_dir_all(paths::venv_dir()).ok();

    std::fs::write(paths::bridge_script(), IRIS_BRIDGE_PY)
        .expect("Failed to write iris_bridge.py");

    let app = Iris::default();
    setup::run_setup(app.setup_steps.clone());
    run(app);
}

impl Default for Iris {
    fn default() -> Self {
        Iris {
            theme: ThemeSet::default(),
            variant: Dark,
            active_page: Signal::new(Page::Setup),
            setup_steps: Arc::new(Mutex::new(setup::default_steps())),
            main_screen: main_ui::MainScreen::new(),
        }
    }
}

impl App for Iris {
    fn title(&self) -> &str { "Iris" }
    fn theme(&self) -> &ThemeSet { &self.theme }
    fn variant(&self) -> ThemeVariant { self.variant }
    fn icon(&self) -> Option<&'static [u8]> {
        Some(icon_png!("src/assets/logo.svg"))
    }
    fn min_size(&self) -> Option<(f32, f32)> {
        Some((1000.0, 600.0))
    }
    fn tick_rate(&self) -> u64 { 42 }
}

impl Component for Iris {
    fn build(&self, palette: &Palette) -> Widget {
        match self.active_page.get() {
            Page::Setup => {
                let screen = setup::ui::SetupScreen {
                    steps: self.setup_steps.clone(),
                    active_page: self.active_page.clone(),
                };
                screen.build(palette)
            },
            Page::Main => self.main_screen.build(palette),
        }
    }
}