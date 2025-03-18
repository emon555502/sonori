// Vertex shader output structure to pass texture coordinates to fragment shader
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

// Uniform for rotation of close button
struct RotationUniform {
    rotation: f32,
}
@group(0) @binding(0) var<uniform> rotation_data: RotationUniform;

// Binding for texture and sampler - used by copy and reset buttons only
// These use the same group/binding positions as the rotation uniform
// but they're used in different pipelines, so it's OK
@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;

// Vertex shader for copy button
@vertex
fn vs_copy(@location(0) position: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(position, 0.0, 1.0);
    // Map from [-1, 1] to [0, 1] for texture coordinates
    out.tex_coords = vec2<f32>(position.x * 0.5 + 0.5, -position.y * 0.5 + 0.5);
    return out;
}

// Vertex shader for reset button
@vertex
fn vs_reset(@location(0) position: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(position, 0.0, 1.0);
    // Map from [-1, 1] to [0, 1] for texture coordinates
    out.tex_coords = vec2<f32>(position.x * 0.5 + 0.5, -position.y * 0.5 + 0.5);
    return out;
}

// Vertex shader for close button - with rotation support
@vertex
fn vs_close(@location(0) position: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    
    // Apply rotation to vertex position
    let angle = rotation_data.rotation;
    let cos_angle = cos(angle);
    let sin_angle = sin(angle);
    
    // Rotate the position around center (0,0)
    let rotated_x = position.x * cos_angle - position.y * sin_angle;
    let rotated_y = position.x * sin_angle + position.y * cos_angle;
    
    out.position = vec4<f32>(rotated_x, rotated_y, 0.0, 1.0);
    
    // For texture coordinates, we need to ensure they're still in [0,1] range
    // Map from [-1, 1] to [0, 1] for texture coordinates
    // We don't rotate texture coordinates to keep X appearance consistent
    out.tex_coords = vec2<f32>(position.x * 0.5 + 0.5, -position.y * 0.5 + 0.5);
    
    return out;
}

// Fragment shader for copy button - uses texture
@fragment
fn fs_copy(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the texture
    var color = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    
    return color;
}

// Fragment shader for reset button - uses texture with red tint
@fragment
fn fs_reset(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the texture
    var color = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    
    return color;
}

// Fragment shader for close button - draws an X (NO texture binding needed)
@fragment
fn fs_close(in: VertexOutput) -> @location(0) vec4<f32> {
    // Draw an X for close button
    var color = vec4<f32>(0.0, 0.0, 0.0, 0.0); // Start transparent
    
    // Coordinates from 0-1
    let uv = in.tex_coords;
    
    // Define the thickness of the X lines
    let thickness = 0.12;
    
    // Check if we're on either diagonal
    let on_diagonal1 = abs(uv.x - uv.y) < thickness;
    let on_diagonal2 = abs(uv.x - (1.0 - uv.y)) < thickness;
    
    // If we're on either diagonal, color is white
    if (on_diagonal1 || on_diagonal2) {
        // Pure white color for the X
        color = vec4<f32>(1.0, 1.0, 1.0, 0.9);
    }
    
    return color;
}
