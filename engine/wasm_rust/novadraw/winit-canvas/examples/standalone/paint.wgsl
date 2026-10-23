struct Settings {
    // Logical width/height, device scale, focus state.
    viewport: vec4<f32>,
    // Local rectangle x/y/width/height.
    marker: vec4<f32>,
    ink: vec4<f32>,
}
@group(0) @binding(0) var<uniform> settings: Settings;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let points = array<vec2<f32>, 3>(
        vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0)
    );
    return vec4(points[index], 0.0, 1.0);
}

@fragment
fn fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let local = position.xy / settings.viewport.z;
    let grid_spacing = 24.0;
    let grid = local - floor(local / grid_spacing) * grid_spacing;
    var color = mix(vec3(0.965), settings.ink.rgb, 0.045);
    if min(grid.x, grid.y) < 1.0 {
        color = mix(color, settings.ink.rgb, 0.16);
    }
    let edge = min(min(local.x, local.y),
        min(settings.viewport.x - local.x, settings.viewport.y - local.y));
    if edge < 3.0 {
        color = mix(vec3(0.70), settings.ink.rgb, settings.viewport.w);
    }
    let relative = local - settings.marker.xy;
    if all(relative >= vec2(0.0)) && all(relative < settings.marker.zw) {
        color = settings.ink.rgb;
        let distance = min(min(relative.x, relative.y),
            min(settings.marker.z - relative.x, settings.marker.w - relative.y));
        if distance < 2.0 {
            color *= 0.7;
        }
    }
    return vec4(color, 1.0);
}
