// Testlast der Probe: nachgebildet auf den Szenendurchgang des Spiels (Bodenmaterial mit zwei Maßstäben, Rauschen,
// halbtransparente Schichten in ein Float-Ziel, danach Ausgabe ins Fenster). Gleiche Last auf Mac und Xbox, damit
// das Verhältnis der Zeiten auf die gemessenen Spielwerte übertragen werden kann.
struct Params { time: f32, layer: f32, srgb_out: f32, pad: f32 };
@group(0) @binding(0) var tex: texture_2d_array<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> u: Params;

struct V { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> V {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    var o: V;
    o.pos = vec4(p * 2.0 - 1.0, 0.5, 1.0);
    o.uv = vec2(p.x, 1.0 - p.y);
    return o;
}
fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2(0.1031, 0.11369));
    return fract((q.x + q.y) * (q.x * q.y * 97.31 + 19.19));
}
fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let s = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), s.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), s.x), s.y);
}
@fragment fn layer_fs(in: V) -> @location(0) vec4<f32> {
    // „Meter“ wie im Spiel: 256 × 144 m im Bild, je Schicht versetzt
    let p = in.uv * vec2(256.0, 144.0) + vec2(u.layer * 13.7, u.time * 0.5);
    let layer = i32(u.layer) % 4;
    let r = vec2(p.x * 0.8 - p.y * 0.6, p.x * 0.6 + p.y * 0.8);
    let w = smoothstep(0.3, 0.7, vnoise(p / 9.0));
    let a = textureSample(tex, samp, p / 3.0, layer);
    let b = textureSample(tex, samp, r / 7.1 + vec2(0.37, 0.71), layer);
    let n = textureSample(tex, samp, p / 2.3, (layer + 1) % 4);
    let grime = vnoise(p * 0.42) * 0.6 + vnoise(r * 1.3) * 0.4;
    var c = mix(a.rgb, b.rgb, w) * (0.8 + 0.4 * n.r) * (0.9 + 0.2 * grime);
    let light = 0.6 + 0.4 * max(0.0, dot(normalize(vec3(n.rg * 2.0 - 1.0, 1.0)), normalize(vec3(0.3, -0.5, 0.8))));
    return vec4(clamp(c * light, vec3(0.0), vec3(1.0)), 0.55);
}
@group(0) @binding(3) var scene: texture_2d<f32>;
fn srgb(c: vec3<f32>) -> vec3<f32> {
    return select(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, c > vec3(0.0031308));
}
@fragment fn post_fs(in: V) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(scene, samp, in.uv, 0.0).rgb;
    return vec4(select(c, srgb(c), u.srgb_out > 0.5), 1.0);
}
