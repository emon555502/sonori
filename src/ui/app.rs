use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, PhysicalSize},
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, DeviceEvents, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    monitor::VideoModeHandle,
    platform::wayland::{
        ActiveEventLoopExtWayland, MonitorHandleExtWayland, WindowAttributesExtWayland,
    },
    window::{CursorIcon, WindowAttributes, WindowId},
};

use smithay_client_toolkit::shell::wlr_layer::{Anchor, Layer};

use super::common::AudioVisualizationData;
use super::window::WindowState;

// Constants from window.rs
use super::window::{GAP, MARGIN, SPECTROGRAM_HEIGHT, SPECTROGRAM_WIDTH, TEXT_AREA_HEIGHT};

pub fn run() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = WindowApp {
        windows: HashMap::new(),
        audio_data: None,
        running: None,
        recording: None,
    };
    event_loop.run_app(&mut app).unwrap();
}

pub fn run_with_audio_data(
    audio_data: Arc<RwLock<AudioVisualizationData>>,
    running: Arc<Mutex<bool>>,
    recording: Arc<Mutex<bool>>,
) {
    let event_loop = EventLoop::new().unwrap();
    let mut app = WindowApp {
        windows: HashMap::new(),
        audio_data: Some(audio_data),
        running: Some(running),
        recording: Some(recording),
    };

    event_loop.run_app(&mut app).unwrap();
}

pub struct WindowApp {
    pub windows: HashMap<WindowId, WindowState>,
    pub audio_data: Option<Arc<RwLock<AudioVisualizationData>>>,
    pub running: Option<Arc<Mutex<bool>>>,
    pub recording: Option<Arc<Mutex<bool>>>,
}

impl ApplicationHandler for WindowApp {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let window_attributes = WindowAttributes::default()
            .with_decorations(false)
            .with_transparent(true);

        if let Some((_, screen)) = event_loop
            .available_monitors()
            .into_iter()
            .enumerate()
            .next()
        {
            let Some(mode) = screen.current_video_mode() else {
                return;
            };
            let window_attributes = window_attributes.clone();
            let mut window_state = create_window(
                event_loop,
                window_attributes.with_title("Sonori"),
                1.0,
                mode,
            );

            if let Some(audio_data) = &self.audio_data {
                window_state.set_audio_data(audio_data.clone());
            }

            // Set the running and recording state references
            window_state.running = self.running.clone();
            window_state.recording = self.recording.clone();

            let window_id = window_state.window.id();
            self.windows.insert(window_id, window_state);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(window) = self.windows.get_mut(&window_id) {
            match event {
                WindowEvent::CloseRequested => {
                    window.quit();
                    event_loop.exit();
                }
                WindowEvent::SurfaceResized(size) => {
                    window.resize(size.width, size.height);
                }
                WindowEvent::RedrawRequested => {
                    window.draw(window.config.width);
                }
                // Handle mouse wheel events for scrolling
                WindowEvent::MouseWheel { delta, .. } => {
                    window.handle_scroll(delta);
                }
                // Add cursor movement handling for button hover states
                WindowEvent::PointerMoved { position, .. } => {
                    window.handle_cursor_moved(position);
                }
                // Add mouse input handling for button clicks
                WindowEvent::PointerButton {
                    button,
                    state,
                    position,
                    ..
                } => {
                    window.handle_mouse_input(
                        button.mouse_button(),
                        state,
                        position,
                        Some(event_loop),
                    );
                }
                // Handle keyboard input for app control
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key: PhysicalKey::Code(key_code),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    match key_code {
                        // Exit on Escape key
                        KeyCode::Escape => {
                            window.quit();
                            event_loop.exit();
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

fn create_window(
    ev: &dyn ActiveEventLoop,
    w: WindowAttributes,
    scale_factor: f64,
    monitor_mode: VideoModeHandle,
) -> WindowState {
    // Use spectrogram size plus text area height and gap
    let fixed_size = PhysicalSize::new(
        SPECTROGRAM_WIDTH,
        SPECTROGRAM_HEIGHT + TEXT_AREA_HEIGHT + GAP,
    );
    let logical_size = fixed_size.to_logical::<i32>(scale_factor);

    // Set the fixed size in the window attributes
    let w = w.with_surface_size(logical_size);

    let w = if ev.is_wayland() {
        // For Wayland, we need to specify the output (monitor)
        w.with_anchor(Anchor::BOTTOM)
            .with_layer(Layer::Overlay)
            .with_margin(MARGIN as i32, MARGIN as i32, MARGIN as i32, MARGIN as i32)
            .with_output(monitor_mode.monitor().native_id())
            .with_resizable(false)
    } else {
        w.with_position(LogicalPosition::new(0, 0))
            .with_window_level(winit::window::WindowLevel::AlwaysOnTop)
            // Don't use fullscreen as it would override our fixed size
            // .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
            .with_resizable(false)
    };

    ev.listen_device_events(DeviceEvents::Always);

    WindowState::new(
        ev.create_window(w.with_cursor(CursorIcon::Default))
            .unwrap(),
    )
}
