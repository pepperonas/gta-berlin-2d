// HUD im Bildschirmraum (Pixel, y nach unten). Wird hinter scene.wgsl und lighting.wgsl angehängt.
struct HudOut {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>, @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) shape: f32, @location(3) @interpolate(flat) extent: vec2<f32>,
    @location(4) @interpolate(flat) extra: vec4<f32>,
};
@vertex fn hud_vs(
    @builtin(vertex_index) index: u32,
    @location(0) center: vec2<f32>, @location(1) extent: vec2<f32>, @location(2) angle: f32,
    @location(3) shape: f32, @location(4) color: vec4<f32>, @location(5) extra: vec4<f32>,
) -> HudOut {
    let corners = array<vec2<f32>, 6>(vec2(-1.,-1.),vec2(1.,-1.),vec2(1.,1.),vec2(-1.,-1.),vec2(1.,1.),vec2(-1.,1.));
    var pad = vec2(1.0);
    if shape == 3.0 { pad = vec2(0.0); } // Zeichen pixelgenau
    let local = corners[index] * (extent + pad);
    let c = cos(angle); let s = sin(angle);
    let p = center + vec2(local.x * c - local.y * s, local.x * s + local.y * c);
    var out: HudOut;
    out.position = vec4(p.x / camera.viewport.x * 2.0 - 1.0, 1.0 - p.y / camera.viewport.y * 2.0, 0.0, 1.0);
    out.local = local; out.color = color; out.shape = shape; out.extent = extent; out.extra = extra;
    return out;
}
fn coverage(d: f32) -> f32 {
    return clamp(0.5 - d / max(fwidth(d), 0.0001), 0.0, 1.0);
}
@fragment fn hud_fs(in: HudOut) -> @location(0) vec4<f32> {
    var a = 1.0;
    if in.shape == 3.0 {
        let uv01 = in.local / in.extent * 0.5 + 0.5;
        let cell = in.extra.x;
        let uv = (vec2(cell % 16.0, floor(cell / 16.0)) * 8.0 + clamp(uv01, vec2(0.0), vec2(0.999)) * 8.0) / 128.0;
        a = textureSample(atlas, atlas_sampler, uv).r;
    } else if in.shape == 1.0 {
        let q = in.local / in.extent;
        a = coverage((length(q) - 1.0) * min(in.extent.x, in.extent.y));
    } else if in.shape == 2.0 {
        let r = length(in.local);
        var ang = atan2(in.local.y, in.local.x);
        // Winkel in den Bereich ab a0 bringen
        let a0 = in.extra.x; let a1 = in.extra.y;
        let tau = 6.2831853;
        ang = a0 + ((ang - a0) % tau + tau) % tau;
        if ang > a1 { discard; }
        a = coverage(abs(r - in.extra.z - in.extra.w * 0.5) - in.extra.w * 0.5);
    } else if in.shape == 5.0 {
        // weicher Fleck: Deckkraft fällt zum Rand auf 0
        let q = length(in.local / in.extent);
        let k = clamp(1.0 - q * q, 0.0, 1.0);
        a = k * k;
    } else if in.shape == 4.0 {
        // Dreieck: Spitze bei (+ext.x, 0), Basis bei x = −ext.x
        let u = (in.local.x + in.extent.x) / (2.0 * in.extent.x);
        let w = (1.0 - u) * in.extent.y;
        a = coverage(max(abs(in.local.y) - w, abs(in.local.x) - in.extent.x));
    } else {
        let radius = min(in.extra.x, min(in.extent.x, in.extent.y));
        let q = abs(in.local) - in.extent + vec2(radius);
        a = coverage(length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - radius);
    }
    let alpha = a * in.color.a;
    if alpha < 0.004 { discard; }
    return vec4(linear_color(in.color.rgb), alpha);
}
