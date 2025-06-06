pub mod app;
pub mod button_texture;
pub mod buttons;
pub mod common;
pub mod event_handler;
pub mod layout_manager;
pub mod render_pipeline;
pub mod scrollbar;
pub mod spectogram;
pub mod text_processor;
pub mod text_renderer;
pub mod text_window;
pub mod window;

pub use app::{run, run_with_audio_data};
