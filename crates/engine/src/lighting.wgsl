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
    if !is_crown(cell) {
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
    out.uv = atlas_uv(cell, q);
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
fn full_tri(i: u32, depth: f32) -> FullOut {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    var out: FullOut;
    out.position = vec4(p * 2.0 - 1.0, depth, 1.0);
    out.uv = vec2(p.x, 1.0 - p.y);
    return out;
}
@vertex fn full_vs(@builtin(vertex_index) i: u32) -> FullOut {
    return full_tri(i, 0.5);
}
// Wie full_vs, aber ganz hinten (Tiefe 0,9999): mit Tiefentest „kleiner“ trifft es nur Stellen, an die noch nichts
// gezeichnet wurde (Hintergrundboden nach den Kacheln).
@vertex fn far_vs(@builtin(vertex_index) i: u32) -> FullOut {
    return full_tri(i, 0.9999);
}
@fragment fn shadow_composite_fs(in: FullOut) -> @location(0) vec4<f32> {
    var m = textureSample(atlas, atlas_sampler, in.uv).r;
    if camera.padding2.y >= 1.0 {
        // weiche Schattenkante: 8 Abtastungen auf einem Ring (Mittel: 1,2 px, Hoch: 2 px Halbschatten)
        let px = select(1.2, 2.0, camera.padding2.y >= 2.0) / camera.viewport;
        var sum = m * 0.2;
        for (var i = 0; i < 8; i++) {
            let a = f32(i) * 0.7853982 + 0.39;
            sum += textureSample(atlas, atlas_sampler, in.uv + vec2(cos(a), sin(a)) * px).r * 0.1;
        }
        m = sum;
    }
    // lighting.js: SHADOW_ALPHA 0,34 × Sonnenstärke, Farbe leicht bläulich (#162b3e)
    return vec4(linear_color(vec3(0.086, 0.169, 0.243)), m * 0.34 * camera.shadow.w);
}
@fragment fn light_composite_fs(in: FullOut) -> @location(0) vec4<f32> {
    // ab Qualität Mittel darf Laternenlicht über 1 gehen (HDR): das Tonemapping fängt es ab, der Bloom nimmt es auf
    let cap = select(1.0, 1.8, camera.padding2.y >= 1.0);
    return vec4(min(textureSample(atlas, atlas_sampler, in.uv).rgb, vec3(cap)), 1.0);
}
@fragment fn ambient_composite_fs(in: FullOut) -> @location(0) vec4<f32> {
    return vec4(linear_color(camera.ambient.rgb), 1.0);
}

// Farbabstimmung und Vignette (lighting.js drawGrade, grime.js drawVignette) als Faktor auf das fertige Szenenbild:
// warmer Verlauf von oben links (Wärme params.w, in der Dämmerung am stärksten), kühler von unten (nachts stärker),
// dunkler Rand. Soft-Light bei 2–6 % Deckkraft ist praktisch eine Tönung: Faktor 1 + a·(2c − 1), auf das hellste
// Glied normiert (die Abstimmung soll nur tönen, nicht aufhellen). Die Vignettenfarbe ist fast schwarz, darum genügt
// Abdunkeln.
fn grade_factor(uv: vec2<f32>) -> vec3<f32> {
    let aspect = camera.viewport.x / max(camera.viewport.y, 1.0);
    let q = vec2(uv.x * aspect, uv.y);
    let dir = vec2(0.7 * aspect, 1.0);
    let t = clamp(dot(q, dir) / dot(dir, dir), 0.0, 1.0);
    let warm_a = camera.params.w * (1.0 - t);
    let cool_a = (0.015 + camera.ambient.w * 0.035) * uv.y;
    let warm = vec3(1.0, 0.851, 0.616);
    let cool = vec3(0.318, 0.475, 0.6);
    var f = (vec3(1.0) + warm_a * (2.0 * warm - 1.0)) * (vec3(1.0) + cool_a * (2.0 * cool - 1.0));
    f = f / max(1.0, max(f.r, max(f.g, f.b)));
    // Vignette: ab 45 % der kürzeren Seite bis in die Ecken auf 17 %
    let c = (uv - 0.5) * vec2(aspect, 1.0);
    let r0 = 0.45 * min(aspect, 1.0);
    let r1 = length(vec2(aspect, 1.0)) * 0.5;
    let v = 0.17 * clamp((length(c) - r0) / (r1 - r0), 0.0, 1.0);
    return pow(f, vec3(2.2)) * (1.0 - v);
}

// Vorschau im Fenster bei fester Zeichengröße (`--fenster`): das abseits gezeichnete Bild, verkleinert.
@fragment fn preview_fs(in: FullOut) -> @location(0) vec4<f32> {
    return vec4(textureSample(atlas, atlas_sampler, in.uv).rgb, 1.0);
}

// --- Nachbearbeitung (Grafik-Überarbeitung Phase 7) ------------------------------------------------------------
// Bloom aus dem HDR-Szenenbild: helle Stellen über einer Schwelle (nachts niedriger) werden auf ½ und ¼ verkleinert,
// zurück auf ½ addiert und im Post dazugemischt. Qualität (camera.padding2.y): 0 = Niedrig/Pixel (alter
// Lichtkarten-Bloom im Szenendurchgang, kein Tonemapping-Bloom), 1 = Mittel (nur ½), 2 = Hoch (½ + ¼).
// Zusatzbild in Gruppe 3: im Post das Bloom-Bild ½, im Körper-Durchgang die Lichtkarte (body_fs)
@group(3) @binding(0) var aux_tex: texture_2d<f32>;
@group(3) @binding(1) var aux_samp: sampler;
fn bloom_threshold() -> f32 {
    return mix(1.05, 0.55, clamp(camera.ambient.w * 1.4, 0.0, 1.0));
}
// 13 Abtastungen (Jimenez, „Next Generation Post Processing“): Mitte, vier innere und acht äußere Punkte, gewichtet
// – verkleinert ohne Flimmern. `dims` = Größe des Quellbilds.
fn tap(uv: vec2<f32>, o: vec2<f32>, t: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(atlas, atlas_sampler, uv + o * t, 0.0).rgb;
}
fn down13(uv: vec2<f32>, dims: vec2<f32>) -> vec3<f32> {
    let t = 1.0 / dims;
    let corners = tap(uv, vec2(-2.0, 2.0), t) + tap(uv, vec2(2.0, 2.0), t) + tap(uv, vec2(-2.0, -2.0), t)
        + tap(uv, vec2(2.0, -2.0), t);
    let edges = tap(uv, vec2(0.0, 2.0), t) + tap(uv, vec2(-2.0, 0.0), t) + tap(uv, vec2(2.0, 0.0), t)
        + tap(uv, vec2(0.0, -2.0), t);
    let inner = tap(uv, vec2(-1.0, 1.0), t) + tap(uv, vec2(1.0, 1.0), t) + tap(uv, vec2(-1.0, -1.0), t)
        + tap(uv, vec2(1.0, -1.0), t);
    return tap(uv, vec2(0.0), t) * 0.125 + corners * 0.03125 + edges * 0.0625 + inner * 0.125;
}
@fragment fn bloom_prefilter_fs(in: FullOut) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(atlas));
    let c = down13(in.uv, dims);
    let l = max(c.r, max(c.g, c.b));
    let th = bloom_threshold();
    // weiches Knie: knapp unter der Schwelle schon ein wenig
    let knee = th * 0.5;
    let soft = clamp(l - th + knee, 0.0, 2.0 * knee);
    let w = max(soft * soft / (4.0 * knee + 1e-4), l - th) / max(l, 1e-4);
    return vec4(min(c * w, vec3(8.0)), 1.0);
}
@fragment fn bloom_down_fs(in: FullOut) -> @location(0) vec4<f32> {
    return vec4(down13(in.uv, vec2<f32>(textureDimensions(atlas))), 1.0);
}
// Zelt (3 × 3) aus der kleineren Stufe, additiv auf die größere
@fragment fn bloom_up_fs(in: FullOut) -> @location(0) vec4<f32> {
    let t = 1.0 / vec2<f32>(textureDimensions(atlas));
    var c = vec3(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let w = (2.0 - abs(f32(x))) * (2.0 - abs(f32(y))) / 16.0;
            c += textureSampleLevel(atlas, atlas_sampler, in.uv + vec2(f32(x), f32(y)) * t, 0.0).rgb * w;
        }
    }
    return vec4(c, 1.0);
}
const AGX_POWER: f32 = 1.35;
const AGX_SAT: f32 = 1.2;
// Belichtung vor AgX: so gewählt, dass Mitteltöne etwa so hell bleiben wie vor dem Tonemapping (Abgleich über
// 0,03…0,9 linear)
const AGX_EXPOSURE: f32 = 1.8;
// AgX (Troy Sobotka; Annäherung von Benjamin Wrensch), Look nach „punchy“ (Sättigung zurückgenommen): lineares Rec.709 → lineares Ausgabebild.
fn agx(c: vec3<f32>) -> vec3<f32> {
    let m = mat3x3<f32>(
        vec3(0.842479062253094, 0.0423282422610123, 0.0423756549057051),
        vec3(0.0784335999999992, 0.878468636469772, 0.0784336),
        vec3(0.0792237451477643, 0.0791661274605434, 0.879142973793104));
    let mi = mat3x3<f32>(
        vec3(1.19687900512017, -0.0528968517574562, -0.0529716355144438),
        vec3(-0.0980208811401368, 1.15190312990417, -0.0980434501171241),
        vec3(-0.0990297440797205, -0.0989611768448433, 1.15107367264116));
    let lo = -12.47393;
    let hi = 4.026069;
    var v = m * max(c, vec3(1e-10));
    v = (clamp(log2(v), vec3(lo), vec3(hi)) - lo) / (hi - lo);
    let x2 = v * v;
    let x4 = x2 * x2;
    v = 15.5 * x4 * x2 - 40.14 * x4 * v + 31.96 * x4 - 6.868 * x2 * v + 0.4298 * x2 + 0.1191 * v - 0.00232;
    // Look: Potenz 1,35, Sättigung 1,2
    v = pow(max(v, vec3(0.0)), vec3(AGX_POWER));
    let luma = dot(v, vec3(0.2126, 0.7152, 0.0722));
    v = luma + AGX_SAT * (v - luma);
    v = mi * v;
    return pow(max(v, vec3(0.0)), vec3(2.2));
}
// Szenenbild (linear, HDR) ins Ausgabebild: Bloom dazu, Farbabstimmung und Vignette, Belichtung, AgX. Ein
// Bildpunkt je Bildpunkt (gleiche Größe), daher textureLoad.
@fragment fn post_fs(in: FullOut) -> @location(0) vec4<f32> {
    var c = textureLoad(atlas, vec2<i32>(floor(in.position.xy)), 0).rgb;
    if camera.padding2.y >= 1.0 {
        let strength = 0.1 + 0.8 * clamp(camera.ambient.w, 0.0, 1.0);
        c += textureSample(aux_tex, aux_samp, in.uv).rgb * strength;
    }
    c *= grade_factor(in.uv);
    return vec4(agx(c * AGX_EXPOSURE), 1.0);
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
    return vec4(clamp(sum * strength, vec3(0.0), vec3(1.0)), 1.0);
}

// --- Pixel-Modus (Phase 8) ----------------------------------------------------------------------------------------
// Das kleine Szenenbild (Gruppe 1) ganzzahlig vergrößert und mittig mit schwarzem Rand ins Ausgabebild; je Bildpunkt
// Kontur an Tiefensprüngen (eine näher liegende Kante daneben: Dach, Krone, Auto über dem Boden), Farbabstimmung,
// Bayer-4×4-Streuung und die Farbtabelle der Palette (palette.rs). camera = volle Ausgabegröße.
@group(3) @binding(2) var pixel_depth: texture_depth_2d;
@group(3) @binding(3) var pixel_lut: texture_3d<f32>;
fn srgb_encode(c: vec3<f32>) -> vec3<f32> {
    return select(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, c > vec3(0.0031308));
}
// BAYER4 stellt palette.rs voran (−0,5…0,5, zeilenweise)
fn bayer4(p: vec2<i32>) -> f32 {
    var m = BAYER4;
    return m[(p.y & 3) * 4 + (p.x & 3)];
}
// je kleinem Bildpunkt (Ziel = Szenengröße): Farbabstimmung, Kontur, Streuung, Palette; Ausgabe sRGB-Werte
// Farbe eines kleinen Bildpunkts vor der Palette: Farbabstimmung, sRGB, kräftiger (Sättigung, Kontrast) – sonst
// landen viele Flächen auf den gedämpften Grautönen der Palette und das Bild wirkt verwaschen.
const PIXEL_SAT: f32 = 1.3;
const PIXEL_CONTRAST: f32 = 1.12;
fn pixel_color(q: vec2<i32>, dims: vec2<i32>) -> vec3<f32> {
    let p = clamp(q, vec2(0), dims - 1);
    let c = textureLoad(atlas, p, 0).rgb * grade_factor((vec2<f32>(p) + 0.5) / vec2<f32>(dims));
    var s = srgb_encode(clamp(c, vec3(0.0), vec3(1.0)));
    let l = dot(s, vec3(0.299, 0.587, 0.114));
    s = l + (s - l) * PIXEL_SAT;
    // Kontrast nur für mittlere und helle Töne: dunkle (Nacht, Schatten) würde er nur weiter absenken
    let lifted = (s - 0.5) * PIXEL_CONTRAST + 0.5;
    return clamp(mix(s, lifted, smoothstep(0.18, 0.42, l)), vec3(0.0), vec3(1.0));
}
// je kleinem Bildpunkt (Ziel = Szenengröße): Kontur, Streuung nur in weichen Verläufen, Palette; Ausgabe sRGB-Werte
@fragment fn pixel_quant_fs(in: FullOut) -> @location(0) vec4<f32> {
    let dims = vec2<i32>(textureDimensions(atlas));
    let q = vec2<i32>(floor(in.position.xy));
    var s = pixel_color(q, dims);
    let n = array<vec2<i32>, 4>(vec2(1, 0), vec2(-1, 0), vec2(0, 1), vec2(0, -1));
    // Kontur: ein Nachbar liegt deutlich näher (Tiefe = Lage; Boden 0,85–0,98, Dächer/Kronen ~0,45); Streuung nur, wo
    // sich die Nachbarn kaum unterscheiden (Licht- und Nebelverläufe) – auf Kanten und Strukturen rauscht sie nur
    let d0 = textureLoad(pixel_depth, q, 0);
    var edge = false;
    var vary = 0.0;
    for (var i = 0; i < 4; i++) {
        let qn = clamp(q + n[i], vec2(0), dims - 1);
        let dn = textureLoad(pixel_depth, qn, 0);
        if dn < 0.8 && d0 - dn > 0.04 { edge = true; }
        let c = pixel_color(qn, dims);
        vary = max(vary, max(abs(c.r - s.r), max(abs(c.g - s.g), abs(c.b - s.b))));
    }
    // Kontur in abgedunkelter Eigenfarbe (liest sich als Schattenkante, nicht als fremder Rand)
    if edge { s = s * 0.22; }
    // Streuung nur in echten Verläufen: über ±3 Bildpunkte ändert sich die Farbe merklich, von Nachbar zu Nachbar
    // aber kaum. Eine gleichmäßige Fläche zwischen zwei Palettenfarben bleibt einfarbig (sonst Schachbrett).
    var slope = 0.0;
    for (var i = 0; i < 2; i++) {
        let a = pixel_color(q + select(vec2(0, 3), vec2(3, 0), i == 0), dims);
        let b = pixel_color(q - select(vec2(0, 3), vec2(3, 0), i == 0), dims);
        slope = max(slope, max(abs(a.r - b.r), max(abs(a.g - b.g), abs(a.b - b.b))));
    }
    let gradient = smoothstep(0.03, 0.07, slope) * (1.0 - smoothstep(0.03, 0.06, vary));
    s += bayer4(q) * PIXEL_DITHER / 7.0 * gradient;
    let idx = vec3<i32>(clamp(floor(s * PIXEL_LUT), vec3(0.0), vec3(PIXEL_LUT - 1.0)));
    return vec4(textureLoad(pixel_lut, idx, 0).rgb, 1.0);
}
// ins Ausgabebild: ganzzahlig vergrößert, mittig, schwarzer Rand (Gruppe 1 = quantisiertes Bild)
@fragment fn pixel_post_fs(in: FullOut) -> @location(0) vec4<f32> {
    let fd = vec2<f32>(textureDimensions(atlas));
    let k = max(floor(min(camera.viewport.x / fd.x, camera.viewport.y / fd.y)), 1.0);
    let off = floor((camera.viewport - fd * k) * 0.5);
    let p = floor((in.position.xy - off) / k);
    if p.x < 0.0 || p.y < 0.0 || p.x >= fd.x || p.y >= fd.y { return vec4(0.0, 0.0, 0.0, 1.0); }
    return vec4(linear_color(textureLoad(atlas, vec2<i32>(p), 0).rgb), 1.0);
}
