use std::sync::Arc;
use wgpu::{self, util::DeviceExt};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, MouseButton},
};

use super::button_texture::ButtonTexture;

// Button dimensions and positions
const COPY_BUTTON_SIZE: u32 = 16;
const RESET_BUTTON_SIZE: u32 = 16;
const CLOSE_BUTTON_SIZE: u32 = 12;
const BUTTON_MARGIN: u32 = 8;
const BUTTON_SPACING: u32 = 8;

// Animation constants
const ANIMATION_DURATION: f32 = 0.1;
const HOVER_SCALE: f32 = 1.1;
const PRESS_SCALE: f32 = 0.9;
const HOVER_ROTATION: f32 = 0.261799; // 15 degrees in radians (π/12)

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonType {
    Copy,
    Reset,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ButtonState {
    Normal,
    Hover,
    Pressed,
}

pub struct Button {
    button_type: ButtonType,
    state: ButtonState,
    position: (u32, u32),
    size: (u32, u32),
    vertices: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
    texture: Option<ButtonTexture>,
    animation_progress: f32,
    previous_state: ButtonState,
    animation_active: bool,
    animation_start_time: std::time::Instant,
    scale: f32,
    rotation: f32,
    rotation_buffer: Option<wgpu::Buffer>,
    rotation_bind_group: Option<wgpu::BindGroup>,
}

pub struct ButtonManager {
    copy_button: Button,
    reset_button: Button,
    close_button: Button,
    text_area_height: u32,
    active_button: Option<ButtonType>,
    default_texture: Option<ButtonTexture>,
}

impl Button {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        button_type: ButtonType,
        position: (u32, u32),
        size: (u32, u32),
        format: wgpu::TextureFormat,
        texture: Option<ButtonTexture>,
    ) -> Self {
        // Create default texture if none provided and it's not a close button
        let texture_for_button = if texture.is_none() && button_type != ButtonType::Close {
            match ButtonTexture::create_default(device, queue, format) {
                Ok(texture) => Some(texture),
                Err(e) => {
                    println!("Failed to create default texture: {}", e);
                    None
                }
            }
        } else {
            texture
        };

        // Create shader for button
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Button Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("button.wgsl").into()),
        });

        // Create rotation uniform buffer and bind group for close button
        let (rotation_buffer, rotation_bind_group) = if button_type == ButtonType::Close {
            // Create rotation uniform buffer
            let rotation_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Close Button Rotation Buffer"),
                contents: bytemuck::cast_slice(&[0.0f32]), // Initial rotation of 0
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            // Create bind group layout
            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                    label: Some("Close Button Bind Group Layout"),
                });

            // Create bind group
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: rotation_buffer.as_entire_binding(),
                }],
                label: Some("Close Button Bind Group"),
            });

            (Some(rotation_buffer), Some(bind_group))
        } else {
            (None, None)
        };

        // Create appropriate pipeline layout based on button type
        let pipeline_layout = if button_type == ButtonType::Close {
            // For close button - use the rotation uniform bind group layout
            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                    label: Some("Close Button Bind Group Layout"),
                });

            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Close Button Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            })
        } else {
            // For buttons that use textures
            // Get the texture bind group layout for the shader
            let bind_group_layout = if let Some(tex) = &texture_for_button {
                &tex.bind_group_layout
            } else {
                // Create a dummy bind group layout if no texture
                &device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                    label: Some("button_texture_bind_group_layout"),
                })
            };

            // Create pipeline layout with texture bindings
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Button Pipeline Layout"),
                bind_group_layouts: &[bind_group_layout],
                push_constant_ranges: &[],
            })
        };

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Button Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: match button_type {
                    ButtonType::Copy => Some("vs_copy"),
                    ButtonType::Reset => Some("vs_reset"),
                    ButtonType::Close => Some("vs_close"),
                },
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 8,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: match button_type {
                    ButtonType::Copy => Some("fs_copy"),
                    ButtonType::Reset => Some("fs_reset"),
                    ButtonType::Close => Some("fs_close"),
                },
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create vertices for button (simple quad)
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Button Vertices"),
            contents: bytemuck::cast_slice(&[
                -1.0f32, -1.0, // top-left
                1.0, -1.0, // top-right
                -1.0, 1.0, // bottom-left
                1.0, 1.0, // bottom-right
            ]),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            button_type,
            state: ButtonState::Normal,
            position,
            size,
            vertices,
            pipeline,
            texture: texture_for_button,
            animation_progress: 0.0,
            previous_state: ButtonState::Normal,
            animation_active: false,
            animation_start_time: std::time::Instant::now(),
            scale: 1.0,
            rotation: 0.0,
            rotation_buffer,
            rotation_bind_group,
        }
    }

    fn contains_point(&self, x: f64, y: f64) -> bool {
        let (button_x, button_y) = self.position;
        let (button_width, button_height) = self.size;

        x >= button_x as f64
            && x <= (button_x + button_width) as f64
            && y >= button_y as f64
            && y <= (button_y + button_height) as f64
    }

    fn set_state(&mut self, state: ButtonState) {
        if self.state != state {
            // Store previous state for animation transition
            self.previous_state = self.state;
            self.state = state;

            // Start animation
            self.animation_active = true;
            self.animation_start_time = std::time::Instant::now();
            self.animation_progress = 0.0;
        }
    }

    // Simplified update_animation method
    fn update_animation(&mut self) {
        if !self.animation_active {
            return;
        }

        // Calculate animation progress
        let elapsed = self.animation_start_time.elapsed().as_secs_f32();
        let duration = ANIMATION_DURATION; // Use the same duration for all states

        self.animation_progress = (elapsed / duration).min(1.0);

        if self.animation_progress >= 1.0 {
            self.animation_active = false;

            // Set final values based on current state
            if self.button_type == ButtonType::Close && self.state == ButtonState::Hover {
                // For close button hover, set rotation
                self.rotation = HOVER_ROTATION;
                self.scale = 1.0; // Keep scale at normal
            } else {
                // For other buttons or states, use scaling
                self.scale = match self.state {
                    ButtonState::Normal => 1.0,
                    ButtonState::Hover => {
                        if self.button_type == ButtonType::Close {
                            1.0
                        } else {
                            HOVER_SCALE
                        }
                    }
                    ButtonState::Pressed => PRESS_SCALE,
                };

                // Reset rotation for non-hover close button
                if self.button_type == ButtonType::Close && self.state != ButtonState::Hover {
                    self.rotation = 0.0;
                }
            }
        } else {
            // Linear interpolation for animations
            if self.button_type == ButtonType::Close {
                // Special case for close button
                if self.state == ButtonState::Hover {
                    // Hovering - rotate
                    let start_rotation = if self.previous_state == ButtonState::Hover {
                        HOVER_ROTATION
                    } else {
                        0.0
                    };
                    self.rotation = start_rotation
                        + self.animation_progress * (HOVER_ROTATION - start_rotation);
                    self.scale = 1.0; // Keep scale normal during hover
                } else if self.state == ButtonState::Pressed {
                    // Pressing - scale down (like other buttons)
                    let start_scale = if self.previous_state == ButtonState::Pressed {
                        PRESS_SCALE
                    } else {
                        1.0
                    };
                    self.scale =
                        start_scale + self.animation_progress * (PRESS_SCALE - start_scale);

                    // Gradually reset rotation if we were hovering before
                    let start_rotation = if self.previous_state == ButtonState::Hover {
                        HOVER_ROTATION
                    } else {
                        0.0
                    };
                    self.rotation = start_rotation * (1.0 - self.animation_progress);
                } else {
                    // Back to normal
                    let start_scale = match self.previous_state {
                        ButtonState::Normal => 1.0,
                        ButtonState::Hover => 1.0, // Was hovering, keep scale normal
                        ButtonState::Pressed => PRESS_SCALE,
                    };
                    self.scale = start_scale + self.animation_progress * (1.0 - start_scale);

                    // Reset rotation
                    let start_rotation = if self.previous_state == ButtonState::Hover {
                        HOVER_ROTATION
                    } else {
                        0.0
                    };
                    self.rotation = start_rotation * (1.0 - self.animation_progress);
                }
            } else {
                // Standard scale animation for other buttons
                let start_scale = match self.previous_state {
                    ButtonState::Normal => 1.0,
                    ButtonState::Hover => HOVER_SCALE,
                    ButtonState::Pressed => PRESS_SCALE,
                };

                let end_scale = match self.state {
                    ButtonState::Normal => 1.0,
                    ButtonState::Hover => HOVER_SCALE,
                    ButtonState::Pressed => PRESS_SCALE,
                };

                // Linear interpolation for scale
                self.scale = start_scale + self.animation_progress * (end_scale - start_scale);
            }
        }
    }

    // Update rotation buffer with current rotation value
    fn update_rotation_buffer(&self, queue: &wgpu::Queue) {
        if let Some(buffer) = &self.rotation_buffer {
            queue.write_buffer(buffer, 0, bytemuck::cast_slice(&[self.rotation]));
        }
    }

    fn render(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
    ) {
        // Update rotation buffer if needed
        if self.button_type == ButtonType::Close {
            self.update_rotation_buffer(queue);
        }

        // Create a new render pass for this button
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Button Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Calculate scaling for animation
        let (center_x, center_y) = (
            self.position.0 as f32 + (self.size.0 as f32 / 2.0),
            self.position.1 as f32 + (self.size.1 as f32 / 2.0),
        );

        // Calculate scaled dimensions
        let scaled_width = self.size.0 as f32 * self.scale;
        let scaled_height = self.size.1 as f32 * self.scale;

        // Calculate top-left position with scaling from center
        let scaled_x = center_x - (scaled_width / 2.0);
        let scaled_y = center_y - (scaled_height / 2.0);

        // Set viewport with animation scaling
        render_pass.set_viewport(scaled_x, scaled_y, scaled_width, scaled_height, 0.0, 1.0);

        render_pass.set_pipeline(&self.pipeline);

        // Set the appropriate bind group
        if self.button_type == ButtonType::Close {
            // Set rotation uniform bind group for close button
            if let Some(bind_group) = &self.rotation_bind_group {
                render_pass.set_bind_group(0, bind_group, &[]);
            }
        } else if let Some(texture) = &self.texture {
            // Set texture bind group for other buttons
            render_pass.set_bind_group(0, &texture.bind_group, &[]);
        }

        render_pass.set_vertex_buffer(0, self.vertices.slice(..));
        render_pass.draw(0..4, 0..1);
    }
}

impl ButtonManager {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        window_size: PhysicalSize<u32>,
        format: wgpu::TextureFormat,
    ) -> Self {
        let text_area_height = super::window::TEXT_AREA_HEIGHT - super::window::GAP;

        // Calculate positions for the copy and reset buttons - centered at bottom
        let total_buttons_width = COPY_BUTTON_SIZE + RESET_BUTTON_SIZE + BUTTON_SPACING;
        let center_x = window_size.width / 2;
        let start_x = center_x - total_buttons_width / 2;

        // Position buttons at the bottom of the text area
        let copy_y_position = text_area_height - COPY_BUTTON_SIZE - BUTTON_MARGIN;
        let reset_y_position = text_area_height - RESET_BUTTON_SIZE - BUTTON_MARGIN;

        // Positions for the buttons
        let copy_position = (start_x, copy_y_position);
        let reset_position = (
            start_x + COPY_BUTTON_SIZE + BUTTON_SPACING,
            reset_y_position,
        );

        // Close button position in top right corner
        let close_position = (
            window_size.width - BUTTON_MARGIN - CLOSE_BUTTON_SIZE,
            BUTTON_MARGIN,
        );

        // Create buttons
        let copy_button = Button::new(
            device,
            queue,
            ButtonType::Copy,
            copy_position,
            (COPY_BUTTON_SIZE, COPY_BUTTON_SIZE),
            format,
            None,
        );

        let reset_button = Button::new(
            device,
            queue,
            ButtonType::Reset,
            reset_position,
            (RESET_BUTTON_SIZE, RESET_BUTTON_SIZE),
            format,
            None,
        );

        let close_button = Button::new(
            device,
            queue,
            ButtonType::Close,
            close_position,
            (CLOSE_BUTTON_SIZE, CLOSE_BUTTON_SIZE),
            format,
            None,
        );

        Self {
            copy_button,
            reset_button,
            close_button,
            text_area_height,
            active_button: None,
            default_texture: None,
        }
    }

    pub fn load_textures(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        copy_image_bytes: Option<&[u8]>,
        reset_image_bytes: Option<&[u8]>,
        format: wgpu::TextureFormat,
    ) {
        // Load copy button texture if provided
        if let Some(image_bytes) = copy_image_bytes {
            if let Ok(texture) = ButtonTexture::from_bytes(
                device,
                queue,
                image_bytes,
                Some("Copy Button Texture"),
                format,
            ) {
                self.copy_button = Button::new(
                    device,
                    queue,
                    ButtonType::Copy,
                    self.copy_button.position,
                    (COPY_BUTTON_SIZE, COPY_BUTTON_SIZE),
                    format,
                    Some(texture),
                );
            }
        }

        // Load reset button texture if provided
        if let Some(image_bytes) = reset_image_bytes {
            if let Ok(texture) = ButtonTexture::from_bytes(
                device,
                queue,
                image_bytes,
                Some("Reset Button Texture"),
                format,
            ) {
                self.reset_button = Button::new(
                    device,
                    queue,
                    ButtonType::Reset,
                    self.reset_button.position,
                    (RESET_BUTTON_SIZE, RESET_BUTTON_SIZE),
                    format,
                    Some(texture),
                );
            }
        }

        // Note: Close button doesn't use a texture - it draws an X directly in the shader
    }

    pub fn resize(&mut self, window_size: PhysicalSize<u32>) {
        // Calculate positions for the copy and reset buttons - centered at bottom
        let total_buttons_width = COPY_BUTTON_SIZE + RESET_BUTTON_SIZE + BUTTON_SPACING;
        let center_x = window_size.width / 2;
        let start_x = center_x - total_buttons_width / 2;

        // Position buttons at the bottom of the text area
        let copy_y_position = self.text_area_height - COPY_BUTTON_SIZE - BUTTON_MARGIN;
        let reset_y_position = self.text_area_height - RESET_BUTTON_SIZE - BUTTON_MARGIN;

        // Update positions
        self.copy_button.position = (start_x, copy_y_position);
        self.reset_button.position = (
            start_x + COPY_BUTTON_SIZE + BUTTON_SPACING,
            reset_y_position,
        );

        // Close button stays in top right
        self.close_button.position = (
            window_size.width - BUTTON_MARGIN - CLOSE_BUTTON_SIZE,
            BUTTON_MARGIN,
        );
    }

    pub fn reset_hover_states(&mut self) {
        self.copy_button.set_state(ButtonState::Normal);
        self.reset_button.set_state(ButtonState::Normal);
        self.close_button.set_state(ButtonState::Normal);
        self.active_button = None;
    }

    pub fn handle_mouse_move(&mut self, position: PhysicalPosition<f64>) {
        // Check if mouse is over any button
        let x = position.x;
        let y = position.y;

        // Determine which button (if any) is being hovered over
        let current_hover = if self.copy_button.contains_point(x, y) {
            Some(ButtonType::Copy)
        } else if self.reset_button.contains_point(x, y) {
            Some(ButtonType::Reset)
        } else if self.close_button.contains_point(x, y) {
            Some(ButtonType::Close)
        } else {
            None
        };

        // Only update states if there's an actual change to avoid wiggling
        // This prevents animation resets when mouse is stationary over a button
        if current_hover != self.active_button {
            // Reset all buttons to normal state first
            self.copy_button.set_state(ButtonState::Normal);
            self.reset_button.set_state(ButtonState::Normal);
            self.close_button.set_state(ButtonState::Normal);

            // Set the newly hovered button to hover state
            match current_hover {
                Some(ButtonType::Copy) => self.copy_button.set_state(ButtonState::Hover),
                Some(ButtonType::Reset) => self.reset_button.set_state(ButtonState::Hover),
                Some(ButtonType::Close) => self.close_button.set_state(ButtonState::Hover),
                None => {}
            }

            // Update active button tracking
            self.active_button = current_hover;
        }
    }

    pub fn handle_pointer_event(
        &mut self,
        button: MouseButton,
        state: ElementState,
        position: PhysicalPosition<f64>,
    ) -> Option<ButtonType> {
        let x = position.x;
        let y = position.y;

        // Only handle left mouse button
        if button != MouseButton::Left {
            return None;
        }

        // Determine which button the mouse is over
        let target_button_type = if self.copy_button.contains_point(x, y) {
            ButtonType::Copy
        } else if self.reset_button.contains_point(x, y) {
            ButtonType::Reset
        } else if self.close_button.contains_point(x, y) {
            ButtonType::Close
        } else {
            return None;
        };

        match state {
            ElementState::Pressed => {
                // Set button state to pressed only if it's different
                let button_to_change = match target_button_type {
                    ButtonType::Copy => &mut self.copy_button,
                    ButtonType::Reset => &mut self.reset_button,
                    ButtonType::Close => &mut self.close_button,
                };

                // Only update if state is changing to avoid restarting animation
                if button_to_change.state != ButtonState::Pressed {
                    button_to_change.set_state(ButtonState::Pressed);
                }

                // Update active button
                self.active_button = Some(target_button_type);
                None
            }
            ElementState::Released => {
                // Check if we're releasing on the same button that was pressed
                if Some(target_button_type) == self.active_button {
                    // Set button state back to hover
                    let button_to_change = match target_button_type {
                        ButtonType::Copy => &mut self.copy_button,
                        ButtonType::Reset => &mut self.reset_button,
                        ButtonType::Close => &mut self.close_button,
                    };

                    // Only update if state is changing
                    if button_to_change.state != ButtonState::Hover {
                        button_to_change.set_state(ButtonState::Hover);
                    }

                    // Return the button type to trigger action
                    Some(target_button_type)
                } else {
                    None
                }
            }
        }
    }

    pub fn update_animations(&mut self) {
        self.copy_button.update_animation();
        self.reset_button.update_animation();
        self.close_button.update_animation();
    }

    pub fn render(
        &mut self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        is_hovering_transcript: bool,
        queue: &wgpu::Queue,
    ) {
        // Only render buttons when hovering over the transcript
        if is_hovering_transcript {
            // Update animations first
            self.update_animations();

            // Render buttons
            self.copy_button.render(view, encoder, queue);
            self.reset_button.render(view, encoder, queue);
            self.close_button.render(view, encoder, queue);
        }
    }
}
