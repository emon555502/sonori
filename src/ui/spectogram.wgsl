// Spectrogram WGSL Shader
// This shader renders a bar-based audio visualization

// Vertex shader inputs
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) instance_position: vec2<f32>,
    @location(2) instance_size: vec2<f32>,
    @location(3) instance_color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Transform the vertex position by the instance position and size
    let world_position = in.position * in.instance_size + in.instance_position;
    out.clip_position = vec4<f32>(world_position, 0.0, 1.0);
    
    // Pass the color to the fragment shader
    out.color = in.instance_color;
    
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
} 