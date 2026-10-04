struct Camera {
    position: vec2<f32>, scale: f32, padding: f32,
    viewport: vec2<f32>, padding2: vec2<f32>,
    sun: vec4<f32>, params: vec4<f32>,
    // shadow: Richtung (x, y), Länge je Höhe, Stärke · ambient: Umgebungslicht (sRGB) und Dunkelheit
    shadow: vec4<f32>, ambient: vec4<f32>,
};
@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>, @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>, @location(3) @interpolate(flat) material: f32,
};
fn project(point: vec3<f32>, center: vec2<f32>, depth: f32) -> vec4<f32> {
    let delta = point.xy - camera.position;
    var h = 0.0;
    if point.z > 0.0 { h = max(18.0, point.z * 0.5); }
    let relative_center = center - camera.position;
    let offset = vec2(relative_center.x * h * 0.0005, -h * 0.5 + relative_center.y * h * 0.00025);
    let screen = (delta + offset) * camera.scale;
    return vec4(screen.x * 2.0 / camera.viewport.x, -screen.y * 2.0 / camera.viewport.y, depth, 1.0);
}
@vertex fn vs(
    @location(0) point: vec3<f32>, @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>, @location(3) uv: vec2<f32>,
    @location(4) center: vec2<f32>, @location(5) material: f32, @location(6) depth: f32,
) -> Out {
    var out: Out;
    out.position = project(point, center, depth);
    out.color = color; out.normal = normal; out.uv = uv; out.material = material;
    return out;
}
fn noise(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2(0.1031, 0.11369));
    return fract((q.x + q.y) * (q.x * q.y * 97.31 + 19.19));
}
fn linear_color(c: vec3<f32>) -> vec3<f32> {
    return select(c / 12.92, pow((c + vec3(0.055)) / 1.055, vec3(2.4)), c > vec3(0.04045));
}
@fragment fn fs(in: Out) -> @location(0) vec4<f32> {
    let px_per_m = camera.params.x;
    let p = in.uv / px_per_m;
    let n = noise(floor(p * 12.0));
    var factor = 1.0;
    let detail = 1.0 - smoothstep(0.04, 0.3, max(fwidth(p.x), fwidth(p.y)));
    if in.material == 1.0 { factor = 0.96 + (n - 0.5) * 0.12 * detail; }
    if in.material == 2.0 || in.material == 3.0 || in.material == 6.0 || in.material == 7.0 {
        var cell = vec2(0.28, 0.20);
        if in.material == 3.0 { cell = vec2(0.70, 0.60); }
        if in.material == 6.0 || in.material == 7.0 { cell = vec2(0.26, 0.70); }
        let row = floor(p.y / cell.y);
        let st = fract((p + vec2(select(0.0, cell.x * 0.5, i32(row) % 2 == 0), 0.0)) / cell);
        let edge = min(min(st.x, 1.0-st.x), min(st.y, 1.0-st.y));
        factor = mix(1.0, select(0.76, 1.0, edge > 0.04) + (n-0.5)*0.08, detail);
    }
    if in.material == 4.0 { factor = 0.95 + noise(floor(p * 2.0)) * 0.12; }
    if in.material == 5.0 { factor = 0.96 + 0.04 * sin(p.x * 1.7 + p.y * 2.1); }
    if in.material == 8.0 { factor = mix(1.0, 0.90 + 0.1*sin(p.y*30.0), detail); }
    if in.material == 9.0 || in.material == 10.0 { factor = 0.97 + (n-0.5)*0.10*detail; }
    var color = in.color * factor;
    if in.material == 11.0 {
        let window = fract(p / vec2(2.5, 3.0));
        let glass = window.x > 0.35 && window.x < 0.72 && window.y > 0.28 && window.y < 0.80;
        color = mix(color, vec3(0.26, 0.32, 0.35), select(0.0, 0.85 * detail, glass));
    }
    let light = 0.60 + 0.40 * max(0.0, dot(normalize(in.normal), normalize(camera.sun.xyz)));
    return vec4(linear_color(clamp(color * light, vec3(0.0), vec3(1.0))), 1.0);
}
struct SpriteOut { @builtin(position) position: vec4<f32>, @location(0) color: vec3<f32>, @location(1) uv: vec2<f32> };
@vertex fn sprite_vs(
    @builtin(vertex_index) index: u32,
    @location(0) point: vec3<f32>, @location(1) size: vec2<f32>, @location(2) angle: f32,
    @location(3) color: vec3<f32>, @location(4) cell: f32, @location(5) depth: f32,
) -> SpriteOut {
    let corners = array<vec2<f32>, 6>(vec2(-0.5,-0.5),vec2(0.5,-0.5),vec2(0.5,0.5),vec2(-0.5,-0.5),vec2(0.5,0.5),vec2(-0.5,0.5));
    let q = corners[index]; let local = q * size;
    let offset = vec2(local.x*cos(angle)-local.y*sin(angle),local.x*sin(angle)+local.y*cos(angle));
    var out: SpriteOut;
    out.position = project(vec3(point.xy+offset, point.z), point.xy, depth);
    out.color = color;
    // Half-texel inset prevents sampling neighboring atlas cells.
    let uv = vec2(0.5/64.0) + (q+0.5) * (63.0/64.0);
    out.uv = (vec2(cell % 4.0, floor(cell/4.0)) + uv) / vec2(4.0,2.0);
    return out;
}
@fragment fn sprite_fs(in: SpriteOut) -> @location(0) vec4<f32> {
    let texel = textureSample(atlas, atlas_sampler, in.uv);
    if texel.a < 0.04 { discard; }
    return vec4(linear_color(texel.rgb * in.color), texel.a);
}
// Bewegte Objekte (Autos, Personen, Marker): instanzierte Rechtecke/Kreise mit weicher Kante (SDF).
// shape 0 = abgerundetes Rechteck, 1 = Ellipse, 2 = Ring.
struct BodyOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>, @location(1) local: vec2<f32>,
    @location(2) extent: vec2<f32>, @location(3) @interpolate(flat) shape: f32,
};
@vertex fn body_vs(
    @builtin(vertex_index) index: u32,
    @location(0) center: vec2<f32>, @location(1) extent: vec2<f32>, @location(2) angle: f32,
    @location(3) shape: f32, @location(4) depth: f32, @location(5) color: vec4<f32>,
) -> BodyOut {
    let corners = array<vec2<f32>, 6>(vec2(-1.,-1.),vec2(1.,-1.),vec2(1.,1.),vec2(-1.,-1.),vec2(1.,1.),vec2(-1.,1.));
    // ein Pixel Rand für die Kantenglättung
    let pad = vec2(1.5 / max(camera.scale, 0.01));
    let local = corners[index] * (extent + pad);
    let c = cos(angle); let s = sin(angle);
    let world = center + vec2(local.x * c - local.y * s, local.x * s + local.y * c);
    var out: BodyOut;
    out.position = project(vec3(world, 0.0), world, depth);
    out.color = color; out.local = local; out.extent = extent; out.shape = shape;
    return out;
}
@fragment fn body_fs(in: BodyOut) -> @location(0) vec4<f32> {
    var d: f32;
    if in.shape == 1.0 {
        let q = in.local / in.extent;
        d = (length(q) - 1.0) * min(in.extent.x, in.extent.y);
    } else if in.shape == 2.0 {
        let r = length(in.local);
        d = abs(r - in.extent.x * 0.85) - in.extent.x * 0.15;
    } else {
        let radius = min(in.extent.x, in.extent.y) * 0.35;
        let q = abs(in.local) - in.extent + vec2(radius);
        d = length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - radius;
    }
    let aa = max(fwidth(d), 0.0001);
    let alpha = clamp(0.5 - d / aa, 0.0, 1.0) * in.color.a;
    if alpha < 0.01 { discard; }
    return vec4(linear_color(in.color.rgb), alpha);
}
