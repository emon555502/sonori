// Vertex shader for a rounded rectangle

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(
    vertex: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(vertex.position, 0.0, 1.0);
    // Convert from clip space [-1,1] to UV space [0,1]
    out.uv = vertex.position * 0.5 + 0.5;
    return out;
}

// Fragment shader for a rounded rectangle
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let center = vec2<f32>(0.5, 0.5);
    let size = vec2<f32>(1.0, 1.0); // Increased size to almost fill the screen
    let corner_radius = 0.16; // Reduced corner radius for less roundness
    
    // Calculate distance from the edge of the rectangle
    let half_size = size * 0.5;
    let dist = abs(in.uv - center) - half_size + corner_radius;
    let dist_to_edge = length(max(dist, vec2<f32>(0.0, 0.0))) - corner_radius;
    
    // Use a very sharp edge with minimal anti-aliasing
    // This creates a much crisper corner
    let edge_width = 0.005; // Very narrow transition for pixel-perfect edges
    let alpha = 1.0 - clamp(dist_to_edge / edge_width + 0.5, 0.0, 1.0);
    
    // Return black with the calculated alpha
    return vec4<f32>(0.0, 0.0, 0.0, alpha * 0.33);
} 