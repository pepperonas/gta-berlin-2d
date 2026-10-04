// Licht und Schatten (Port von lighting.js als GPU-Pässe). Wird hinter scene.wgsl angehängt.

// --- Schattenmaske: Hauswände werden im Vertex-Shader entlang der Sonne extrudiert -----------------------
@vertex fn shadow_vs(@location(0) point: vec2<f32>, @location(1) height: f32, @location(2) extrude: f32) -> @builtin(position) vec4<f32> {
    let reach = min(900.0, height * camera.shadow.z);
    let world = point + camera.shadow.xy * reach * extrude;
    return project(vec3(world, 0.0), world, 0.5);
}
@fragment fn shadow_fs() -> @location(0) vec4<f32> {
    return vec4(1.0);
}

// Baumkronen: Kronenbild als Schatten, entlang der Sonne gestreckt und versetzt (treeShadowGeom)
struct TreeShadowOut { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn tree_shadow_vs(
    @builtin(vertex_index) index: u32,
    @location(0) point: vec3<f32>, @location(1) size: vec2<f32>, @location(2) angle: f32,
    @location(3) color: vec3<f32>, @location(4) cell: f32, @location(5) depth: f32,
) -> TreeShadowOut {
    var out: TreeShadowOut;
    if !(cell == 0.0 || cell == 6.0) {
        out.position = vec4(2.0, 2.0, 2.0, 1.0); // nur Bäume werfen Schatten (Decals nicht)
        return out;
    }
    let corners = array<vec2<f32>, 6>(vec2(-0.5,-0.5),vec2(0.5,-0.5),vec2(0.5,0.5),vec2(-0.5,-0.5),vec2(0.5,0.5),vec2(-0.5,0.5));
    let q = corners[index];
    let r = size.x * 0.5;
    let l = min(camera.shadow.z, 6.0) * 0.45;
    var u = camera.shadow.xy;
    if length(u) < 1e-4 { u = vec2(0.0, 1.0); }
    u = normalize(u);
    let p = vec2(-u.y, u.x);
    let center = point.xy + u * min(900.0, r * 1.8 * l);
    let minor = r * 0.95;
    let major = minor * min(3.0, sqrt(1.0 + l * l));
    let world = center + u * q.x * 2.3 * major + p * q.y * 2.3 * minor;
    out.position = project(vec3(world, 0.0), world, 0.5);
    let uv = vec2(0.5 / 64.0) + (q + 0.5) * (63.0 / 64.0);
    out.uv = (vec2(cell % 4.0, floor(cell / 4.0)) + uv) / vec2(4.0, 2.0);
    return out;
}
@fragment fn tree_shadow_fs(in: TreeShadowOut) -> @location(0) vec4<f32> {
    let a = textureSample(atlas, atlas_sampler, in.uv).a;
    if a < 0.04 { discard; }
    return vec4(a * 0.9);
}

// --- Lichtkarte: Lichtquellen additiv auf das Umgebungslicht --------------------------------------------
struct LightOut {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>, @location(1) color: vec3<f32>, @location(2) @interpolate(flat) cone: f32,
};
@vertex fn light_vs(
    @builtin(vertex_index) index: u32,
    @location(0) center: vec2<f32>, @location(1) radius: f32, @location(2) angle: f32,
    @location(3) color: vec3<f32>, @location(4) intensity: f32, @location(5) cone: f32,
) -> LightOut {
    let corners = array<vec2<f32>, 6>(vec2(-1.,-1.),vec2(1.,-1.),vec2(1.,1.),vec2(-1.,-1.),vec2(1.,1.),vec2(-1.,1.));
    let q = corners[index];
    var local = q;
    var offset = q * radius;
    if cone > 0.5 {
        // Scheinwerferkegel: Ursprung am Licht, zeigt in Fahrtrichtung, 0,62 × Reichweite breit
        local = vec2((q.x + 1.0) * 0.5, q.y * 0.31);
        let c = cos(angle); let s = sin(angle);
        let l = local * radius;
        offset = vec2(l.x * c - l.y * s, l.x * s + l.y * c);
    }
    var out: LightOut;
    let world = center + offset;
    out.position = project(vec3(world, 0.0), world, 0.5);
    out.local = local;
    out.color = linear_color(color) * intensity;
    out.cone = cone;
    return out;
}
@fragment fn light_fs(in: LightOut) -> @location(0) vec4<f32> {
    var f = 0.0;
    if in.cone > 0.5 {
        let spread = 0.31 * (0.125 + 0.875 * in.local.x);
        if abs(in.local.y) > spread { discard; }
        let d = length(in.local);
        f = select(mix(0.4, 0.0, (d - 0.5) / 0.5), mix(0.95, 0.4, d / 0.5), d < 0.5);
    } else {
        let d = length(in.local);
        f = select(mix(0.55, 0.0, (d - 0.35) / 0.65), mix(1.0, 0.55, d / 0.35), d < 0.35);
    }
    f = max(f, 0.0);
    return vec4(in.color * f, 1.0);
}

// --- Auftragen: Vollbild-Dreieck in Tiefe 0,5 (Boden liegt dahinter, Dächer und Kronen davor) --------------
struct FullOut { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn full_vs(@builtin(vertex_index) i: u32) -> FullOut {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    var out: FullOut;
    out.position = vec4(p * 2.0 - 1.0, 0.5, 1.0);
    out.uv = vec2(p.x, 1.0 - p.y);
    return out;
}
@fragment fn shadow_composite_fs(in: FullOut) -> @location(0) vec4<f32> {
    let m = textureSample(atlas, atlas_sampler, in.uv).r;
    // lighting.js: SHADOW_ALPHA 0,34 × Sonnenstärke, Farbe leicht bläulich (#162b3e)
    return vec4(linear_color(vec3(0.086, 0.169, 0.243)), m * 0.34 * camera.shadow.w);
}
@fragment fn light_composite_fs(in: FullOut) -> @location(0) vec4<f32> {
    return vec4(min(textureSample(atlas, atlas_sampler, in.uv).rgb, vec3(1.0)), 1.0);
}
@fragment fn ambient_composite_fs(in: FullOut) -> @location(0) vec4<f32> {
    return vec4(linear_color(camera.ambient.rgb), 1.0);
}

// Farbabstimmung und Vignette (lighting.js drawGrade, grime.js drawVignette) als ein Vollbild-Durchgang, der das
// Bild multipliziert: warmer Verlauf von oben links (Wärme params.w, in der Dämmerung am stärksten), kühler von unten
// (nachts stärker), dunkler Rand. Soft-Light bei 2–6 % Deckkraft ist praktisch eine Tönung: Faktor 1 + a·(2c − 1),
// auf das hellste Glied normiert (ein Ziel ohne HDR kann nicht aufhellen). Die Vignettenfarbe ist fast schwarz,
// darum genügt Abdunkeln.
@fragment fn grade_fs(in: FullOut) -> @location(0) vec4<f32> {
    let aspect = camera.viewport.x / max(camera.viewport.y, 1.0);
    let q = vec2(in.uv.x * aspect, in.uv.y);
    let dir = vec2(0.7 * aspect, 1.0);
    let t = clamp(dot(q, dir) / dot(dir, dir), 0.0, 1.0);
    let warm_a = camera.params.w * (1.0 - t);
    let cool_a = (0.015 + camera.ambient.w * 0.035) * in.uv.y;
    let warm = vec3(1.0, 0.851, 0.616);
    let cool = vec3(0.318, 0.475, 0.6);
    var f = (vec3(1.0) + warm_a * (2.0 * warm - 1.0)) * (vec3(1.0) + cool_a * (2.0 * cool - 1.0));
    f = f / max(1.0, max(f.r, max(f.g, f.b)));
    // Vignette: ab 45 % der kürzeren Seite bis in die Ecken auf 17 %
    let c = (in.uv - 0.5) * vec2(aspect, 1.0);
    let r0 = 0.45 * min(aspect, 1.0);
    let r1 = length(vec2(aspect, 1.0)) * 0.5;
    let v = 0.17 * clamp((length(c) - r0) / (r1 - r0), 0.0, 1.0);
    return vec4(pow(f, vec3(2.2)) * (1.0 - v), 1.0);
}

// Bloom (lighting.js drawBloom): helle Stellen der Lichtkarte überstrahlen. Quelle wie brightness(0,55) contrast(5),
// weichgezeichnet (13 Abtastungen auf zwei Ringen), Mischung „screen“. Stärke max(0, Dunkelheit − 0,3) · 0,32.
fn bloom_src(uv: vec2<f32>) -> vec3<f32> {
    let c = textureSample(atlas, atlas_sampler, uv).rgb * 0.55;
    return clamp((c - 0.5) * 5.0 + 0.5, vec3(0.0), vec3(1.0));
}
@fragment fn bloom_fs(in: FullOut) -> @location(0) vec4<f32> {
    let strength = max(0.0, camera.ambient.w - 0.3) * 0.32;
    let px = vec2(6.0) / camera.viewport;
    var sum = bloom_src(in.uv) * 0.2;
    for (var i = 0; i < 6; i++) {
        let a = f32(i) * 1.0471976;
        let d = vec2(cos(a), sin(a));
        sum += bloom_src(in.uv + d * px) * 0.08;
        sum += bloom_src(in.uv + d * px * 2.2) * 0.0533;
    }
    return vec4(sum * strength, 1.0);
}
