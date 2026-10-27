struct Geometry {
    bounds: vec4<f32>,
    window_size: vec2<f32>,
    uv_size: vec2<f32>,
}
@group(0) @binding(0) var<uniform> geometry: Geometry;
@group(0) @binding(1) var canvas: texture_2d<f32>;
@group(0) @binding(2) var canvas_sampler: sampler;
struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
@vertex fn vertex(@builtin(vertex_index) index: u32) -> Vertex {
    let corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0)
    );
    let corner = corners[index];
    let pixel = geometry.bounds.xy + corner * geometry.bounds.zw;
    var result: Vertex;
    result.position = vec4(pixel.x / geometry.window_size.x * 2.0 - 1.0,
                           1.0 - pixel.y / geometry.window_size.y * 2.0, 0.0, 1.0);
    result.uv = corner * geometry.uv_size;
    return result;
}
@fragment fn fragment(input: Vertex) -> @location(0) vec4<f32> {
    return textureSample(canvas, canvas_sampler, input.uv);
}
