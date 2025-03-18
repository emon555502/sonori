use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use wgpu::{self, util::DeviceExt};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, MouseButton, MouseScrollDelta},
    event_loop::ActiveEventLoop,
    window::Window,
};

use super::buttons::ButtonManager;
use super::common::AudioVisualizationData;
use super::event_handler::EventHandler;
use super::layout_manager::LayoutManager;
use super::render_pipeline::RenderPipelines;
use super::scrollbar::{Scrollbar, SCROLLBAR_WIDTH};
use super::spectogram::Spectrogram;
use super::text_processor::{TextLayoutInfo, TextProcessor};
use super::text_window::TextWindow;
use parking_lot::RwLock;

pub const SPECTROGRAM_WIDTH: u32 = 240; // Width of the spectrogram
pub const SPECTROGRAM_HEIGHT: u32 = 80; // Height of the spectrogram
pub const TEXT_AREA_HEIGHT: u32 = 90; // Additional height for text above spectrogram
pub const MARGIN: i32 = 32; // Margin from the bottom of the screen
pub const GAP: u32 = 4; // Gap between text area and spectrogram
pub const RIGHT_MARGIN: f32 = 4.0; // Right margin for text area
pub const LEFT_MARGIN: f32 = 4.0; // Left margin for text area

pub struct WindowState {
    pub window: Arc<dyn Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub spectrogram: Option<Spectrogram>,
    pub audio_data: Option<Arc<RwLock<AudioVisualizationData>>>,
    pub render_pipelines: RenderPipelines,
    pub text_window: TextWindow,
    pub button_manager: ButtonManager,
    pub text_processor: TextProcessor,
    pub layout_manager: LayoutManager,
    pub scrollbar: Scrollbar,
    pub scroll_offset: f32,
    pub max_scroll_offset: f32,
    pub auto_scroll: bool,
    pub last_transcript_len: usize,
    pub event_handler: EventHandler,
    pub running: Option<Arc<AtomicBool>>,
    pub recording: Option<Arc<AtomicBool>>,
}

impl WindowState {
    pub fn new(
        window: Box<dyn Window>,
        running: Option<Arc<AtomicBool>>,
        recording: Option<Arc<AtomicBool>>,
    ) -> Self {
        let window: Arc<dyn Window> = Arc::from(window);

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .unwrap();

        let fixed_width = SPECTROGRAM_WIDTH;
        let fixed_height = SPECTROGRAM_HEIGHT + TEXT_AREA_HEIGHT + GAP;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .filter(|f| f.is_srgb())
            .next()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: fixed_width,
            height: fixed_height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        // Create render pipelines
        let render_pipelines = RenderPipelines::new(&device, &config);

        // Initialize TextWindow
        let text_window = TextWindow::new(
            &device,
            &queue,
            &config,
            PhysicalSize::new(config.width, config.height),
        );

        // Create the button manager
        let mut button_manager = ButtonManager::new(
            &device,
            &queue,
            PhysicalSize::new(config.width, config.height),
            config.format,
        );

        // Load button icons
        let copy_icon = include_bytes!("../../assets/copy.png");
        let reset_icon = include_bytes!("../../assets/reset.png");
        let pause_icon = include_bytes!("../../assets/pause.png");
        let play_icon = include_bytes!("../../assets/play.png");

        button_manager.load_textures(
            &device,
            &queue,
            Some(copy_icon),
            Some(reset_icon),
            Some(pause_icon),
            Some(play_icon),
            config.format,
        );

        // Set recording state in button manager
        button_manager.set_recording(recording.clone());

        // Create the scrollbar
        let scrollbar = Scrollbar::new(&device, &config);

        // Create text processor with default values
        let text_processor = TextProcessor::new(8.0, 20.0, 4.0);

        // Create layout manager
        let layout_manager = LayoutManager::new(
            config.width,
            config.height,
            SPECTROGRAM_WIDTH,
            SPECTROGRAM_HEIGHT,
            TEXT_AREA_HEIGHT,
            RIGHT_MARGIN,
            LEFT_MARGIN,
            GAP,
        );

        // Create event handler
        let mut event_handler = EventHandler::new();
        event_handler.recording = recording.clone();

        Self {
            window,
            surface,
            device,
            queue,
            config,
            spectrogram: None,
            audio_data: None,
            render_pipelines,
            text_window,
            button_manager,
            text_processor,
            layout_manager,

            // Scrollbar and scroll state
            scrollbar,
            scroll_offset: 0.0,
            max_scroll_offset: 0.0,

            // Auto-scroll control
            auto_scroll: true,
            last_transcript_len: 0,

            // Event handler
            event_handler,

            // Transcriber state references
            running,
            recording,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);

            // Update layout manager dimensions
            self.layout_manager.update_dimensions(width, height);

            if let Some(spectrogram) = &mut self.spectrogram {
                spectrogram.resize(PhysicalSize::new(width, height));
            }

            self.text_window.resize(PhysicalSize::new(width, height));
            self.button_manager.resize(PhysicalSize::new(width, height));
        }
    }

    pub fn set_audio_data(&mut self, audio_data: Arc<RwLock<AudioVisualizationData>>) {
        self.audio_data = Some(audio_data);

        // Initialize spectrogram if not already created
        if self.spectrogram.is_none() {
            // Create the spectrogram with the dedicated spectrogram size, not the full window size
            let size = PhysicalSize::new(SPECTROGRAM_WIDTH, SPECTROGRAM_HEIGHT);
            let spectrogram = Spectrogram::new(
                Arc::new(self.device.clone()),
                Arc::new(self.queue.clone()),
                size,
                self.config.format,
            );
            self.spectrogram = Some(spectrogram);
        }
    }

    pub fn draw(&mut self, _width: u32) {
        let output = self.surface.get_current_texture().unwrap();
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // First clear the screen to transparent
        self.render_pipelines.draw_background(&mut encoder, &view);

        // Draw the rounded rectangle background for the spectrogram only
        self.render_pipelines.draw_spectrogram_background(
            &mut encoder,
            &view,
            TEXT_AREA_HEIGHT,
            GAP,
            SPECTROGRAM_WIDTH,
            SPECTROGRAM_HEIGHT,
        );

        // Get audio data once
        let mut display_text: String = String::new();
        let mut is_speaking: bool = false;
        let empty_samples: Vec<f32> = Vec::new();

        // Check recording state
        let is_recording = self
            .recording
            .as_ref()
            .map(|rec| rec.load(Ordering::Relaxed))
            .unwrap_or(false);

        // Determine if scrollbar is needed and the actual width to use for text area
        let mut need_scrollbar: bool = false;
        let mut text_area_width: u32;
        let text_area_height = self.layout_manager.get_text_area_height();

        // Always ensure the spectrogram is initialized
        if self.spectrogram.is_none() {
            let size = PhysicalSize::new(SPECTROGRAM_WIDTH, SPECTROGRAM_HEIGHT);
            let spectrogram = Spectrogram::new(
                Arc::new(self.device.clone()),
                Arc::new(self.queue.clone()),
                size,
                self.config.format,
            );
            self.spectrogram = Some(spectrogram);
        }

        // Render the spectrogram with either the available audio data or empty data
        if let Some(spectrogram) = &mut self.spectrogram {
            let samples = if let Some(audio_data) = &self.audio_data {
                let audio_data_lock = audio_data.read();
                let samples_clone = if is_recording {
                    audio_data_lock.samples.clone() // Only use real samples when recording
                } else {
                    empty_samples.clone() // Use empty samples when not recording
                };
                is_speaking = is_recording && audio_data_lock.is_speaking; // Only show speaking state when recording
                let transcript = audio_data_lock.transcript.clone();
                display_text = self.text_processor.clean_whitespace(&transcript);
                drop(audio_data_lock);
                samples_clone
            } else {
                if is_recording {
                    display_text = "Sonori is ready".to_string();
                }
                is_speaking = false;
                self.max_scroll_offset = 0.0;
                self.last_transcript_len = 0;
                need_scrollbar = false;
                text_area_width = self.layout_manager.calculate_text_area_width(false);
                self.scrollbar.max_scroll_offset = 0.0;
                self.scrollbar.scroll_offset = 0.0;
                empty_samples.clone()
            };

            // Always update and render the spectrogram
            spectrogram.update(&samples);

            // Create a render pass with a viewport that positions the spectrogram below the text area
            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Spectrogram Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load, // Load existing content
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                // Set the viewport using the layout manager
                let (x, y, width, height) = self.layout_manager.get_spectrogram_position();
                render_pass.set_viewport(x, y, width, height, 0.0, 1.0);

                // Use the custom render pass
                spectrogram.render_with_custom_pass(&mut render_pass);
            }
        }

        // Check if transcript has changed - only when recording
        let transcript_changed = is_recording && display_text.len() != self.last_transcript_len;
        if is_recording {
            self.last_transcript_len = display_text.len();
        }

        // Calculate text layout using the text processor
        let layout_info = self.text_processor.calculate_layout(
            &display_text,
            self.config.width as f32,
            text_area_height as f32,
        );

        need_scrollbar = layout_info.need_scrollbar;

        // Set text area width based on whether scrollbar is needed
        text_area_width = self
            .layout_manager
            .calculate_text_area_width(need_scrollbar);

        self.max_scroll_offset = layout_info.max_scroll_offset;
        self.scroll_offset = self.scroll_offset.min(self.max_scroll_offset);
        self.scrollbar.max_scroll_offset = self.max_scroll_offset;
        self.scrollbar.scroll_offset = self.scroll_offset;
        self.scrollbar.auto_scroll = self.event_handler.auto_scroll;

        if self.auto_scroll && transcript_changed {
            self.scroll_offset = self.max_scroll_offset;
            self.scrollbar.scroll_offset = self.max_scroll_offset;
        }

        // Get text position from the layout manager
        let (text_x, text_y) = self.layout_manager.get_text_position(self.scroll_offset);

        let text_scale = 1.0;

        // Choose text color based on speaking state
        let text_color = if is_speaking {
            [0.0, 0.8, 0.4, 1.0] // Teal-green for listening
        } else {
            [1.0, 0.8, 0.1, 1.0] // Bright gold for ready/text
        };

        // Render text window (background and text)
        self.text_window.render(
            &mut encoder,
            &view,
            &display_text,
            text_area_width,
            text_area_height,
            GAP,
            text_x,
            text_y,
            text_scale,
            text_color,
        );

        // Draw scrollbar only if needed
        if need_scrollbar {
            // Use the scrollbar component to render
            self.scrollbar.render(
                &view,
                &mut encoder,
                self.config.width,
                text_area_height,
                GAP,
            );
        }

        // Render the buttons after the text - only when hovering over transcript
        // First make sure the pause/play button texture is up-to-date
        if self.event_handler.hovering_transcript {
            // Update button texture based on recording state
            self.button_manager.update_pause_button_texture();
        }

        (&mut self.button_manager).render(
            &view,
            &mut encoder,
            self.event_handler.hovering_transcript,
            &self.queue,
        );

        // Submit all rendering commands
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        // Request redraw to keep animation loop going
        self.window.request_redraw();
    }

    pub fn handle_scroll(&mut self, delta: MouseScrollDelta) {
        self.event_handler
            .handle_scroll(&mut self.scroll_offset, self.max_scroll_offset, delta);
        self.auto_scroll = self.event_handler.auto_scroll;
        self.scrollbar.auto_scroll = self.auto_scroll;
        self.scrollbar.scroll_offset = self.scroll_offset;
        self.window.request_redraw();
    }

    pub fn handle_cursor_moved(&mut self, position: PhysicalPosition<f64>) {
        // Calculate text area dimensions
        let text_area_width = self
            .layout_manager
            .calculate_text_area_width(self.max_scroll_offset > 0.0);
        let text_area_height = self.layout_manager.get_text_area_height();

        // Get window size
        let window_size = self.window.outer_size();

        // Update event handler and button states
        self.event_handler.handle_cursor_moved(
            position,
            text_area_width,
            text_area_height,
            window_size.width,
            window_size.height,
            &mut self.button_manager,
        );

        self.window.request_redraw();
    }

    pub fn handle_mouse_input(
        &mut self,
        button: MouseButton,
        state: ElementState,
        position: PhysicalPosition<f64>,
        event_loop: Option<&dyn ActiveEventLoop>,
    ) {
        let redraw_needed = self.event_handler.handle_mouse_input(
            button,
            state,
            position,
            &mut self.button_manager,
            &self.audio_data,
            &mut self.last_transcript_len,
            &mut self.scroll_offset,
            &mut self.max_scroll_offset,
            &self.running,
            event_loop,
        );

        if redraw_needed {
            self.window.request_redraw();
        }
    }

    pub fn copy_transcript(&self) {
        EventHandler::copy_transcript(&self.audio_data);
    }

    pub fn reset_transcript(&mut self) {
        EventHandler::reset_transcript(
            &self.audio_data,
            &mut self.last_transcript_len,
            &mut self.scroll_offset,
            &mut self.max_scroll_offset,
        );
    }

    pub fn toggle_recording(&mut self) {
        if let Some(recording) = &self.recording {
            // Toggle recording state
            let was_recording = recording.load(Ordering::Relaxed);
            let new_state = !was_recording;
            recording.store(new_state, Ordering::Relaxed);
            println!("Recording toggled to: {}", new_state);

            // Update button texture after toggling recording state
            self.button_manager.update_pause_button_texture();
        } else {
            println!("Error: recording state is None");
        }
    }

    pub fn quit(&mut self) {
        if let Some(running) = &self.running {
            running.store(false, Ordering::Relaxed);
        }
    }
}
