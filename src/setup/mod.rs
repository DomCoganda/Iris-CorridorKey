mod python;
mod venv;
mod models;

pub mod paths;
pub mod ui;

use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub enum StepState {
    Waiting,
    Active,
    Completed,
    Failed,
}

#[derive(Clone)]
pub struct SetupStep {
    pub title: String,
    pub state: StepState,
    pub status: String,
    pub progress: Option<f32>,
}

pub fn default_steps() -> Vec<SetupStep> {
    vec![
        SetupStep {
            title: "Python Runtime".into(),
            state: StepState::Waiting,
            status: String::new(),
            progress: None,
        },
        SetupStep {
            title: "Virtual Environment".into(),
            state: StepState::Waiting,
            status: String::new(),
            progress: None,
        },
        SetupStep {
            title: "Backend Packages".into(),
            state: StepState::Waiting,
            status: String::new(),
            progress: None,
        },
        SetupStep {
            title: "GPU Support".into(),
            state: StepState::Waiting,
            status: String::new(),
            progress: None,
        },
        SetupStep {
            title: "FFmpeg".into(),
            state: StepState::Waiting,
            status: String::new(),
            progress: None,
        },
        SetupStep {
            title: "CorridorKey Scripts".into(),
            state: StepState::Waiting,
            status: String::new(),
            progress: None,
        },
        SetupStep {
            title: "CorridorKey Model".into(),
            state: StepState::Waiting,
            status: String::new(),
            progress: None,
        },
        SetupStep {
            title: "BiRefNet Alpha Model".into(),
            state: StepState::Waiting,
            status: String::new(),
            progress: None,
        },
    ]
}

pub fn run_setup(steps: Arc<Mutex<Vec<SetupStep>>>) {
    std::thread::spawn(move || {
        let update = |index: usize,
                      state: StepState,
                      message: String,
                      fraction: Option<f32>| {
            let mut current = steps.lock().unwrap();
            current[index].state = state;
            current[index].status = message;
            current[index].progress = fraction;
        };

        let tasks: &[(usize, fn(&dyn Fn(python::Progress)) -> Result<(), String>)] = &[
            (0, |r| python::install(r)),
            (1, |r| venv::create_env(r)),
            (2, |r| venv::install_packages(r)),
            (3, |r| venv::ensure_gpu_torch(r)),
            (4, |r| venv::install_ffmpeg(r)),
            (5, |r| models::download_corridor_src(r)),
            (6, |r| models::download_corridor(r)),
            (7, |r| models::download_birefnet(r)),
        ];

        for (index, task) in tasks {
            update(*index, StepState::Active, "Starting...".into(), Some(0.0));

            let result = task(&|p| {
                update(*index, StepState::Active, p.message, p.fraction);
            });

            match result {
                Ok(_) => update(*index, StepState::Completed, "Done".into(), Some(1.0)),
                Err(e) => update(*index, StepState::Failed, e, None),
            }
        }
    });
}