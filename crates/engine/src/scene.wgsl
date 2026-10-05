struct Camera {
    position: vec2<f32>, scale: f32, fog: f32,
    viewport: vec2<f32>, padding2: vec2<f32>,
    sun: vec4<f32>, params: vec4<f32>,
    // shadow: Richtung (x, y), Länge je Höhe, Stärke · ambient: Umgebungslicht (sRGB) und Dunkelheit
    shadow: vec4<f32>, ambient: vec4<f32>,
};
@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;
// Bodenmaterialien (materials.rs): Detail (Farbe/Mittelwert um 0,5) und Normale/Rauheit/Umgebungsverdeckung je
// Schicht; Parameter je Material-ID: a = (Schicht + 1, Kachel m, Stärke, Farbanteil), b = (Relief, AO, –, –).
struct MatParams { a: array<vec4<f32>, 32>, b: array<vec4<f32>, 32> };
@group(2) @binding(0) var mat_detail: texture_2d_array<f32>;
@group(2) @binding(1) var mat_nr: texture_2d_array<f32>;
@group(2) @binding(2) var mat_sampler: sampler;
@group(2) @binding(3) var<uniform> mat: MatParams;
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>, @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>, @location(3) @interpolate(flat) material: f32,
    @location(4) @interpolate(flat) center: vec2<f32>,
    // Lage quer über den Rasenrand (mesh.rs fringe: center.x), anders als center interpoliert
    @location(5) across: f32,
};
fn project(point: vec3<f32>, center: vec2<f32>, depth: f32) -> vec4<f32> {
    var h = 0.0;
    // Minikarte (params.y = 1) zeigt alles flach von oben
    if point.z > 0.0 && camera.params.y < 0.5 { h = max(18.0, point.z * 0.5); }
    return project_h(point, center, depth, h);
}
fn project_h(point: vec3<f32>, center: vec2<f32>, depth: f32, h: f32) -> vec4<f32> {
    let delta = point.xy - camera.position;
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
    // Fassadendetails (Tür, Ladenfront): z ist der Anteil der projizierten Wandhöhe, uv.y die Gebäudehöhe
    if material == 13.0 {
        var h = 0.0;
        if camera.params.y < 0.5 { h = max(18.0, uv.y * 0.5) * point.z; }
        out.position = project_h(point, center, depth, h);
    }
    out.color = color; out.normal = normal; out.uv = uv; out.material = material; out.center = center;
    out.across = select(1.0, center.x, material == 16.0);
    // Minikarte: Häuser als dunkle Grundrisse (hud.js MINI: #2b2d33), Boden in seinen Farben
    if camera.params.y > 0.5 && point.z > 0.0 { out.color = vec3(0.169, 0.176, 0.2) / 0.78; }
    return out;
}
fn noise(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2(0.1031, 0.11369));
    return fract((q.x + q.y) * (q.x * q.y * 97.31 + 19.19));
}
// weiches Wertrauschen 0…1 (bilinear zwischen Gitterpunkten)
fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = noise(i);
    let b = noise(i + vec2(1.0, 0.0));
    let c = noise(i + vec2(0.0, 1.0));
    let d = noise(i + vec2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
fn linear_color(c: vec3<f32>) -> vec3<f32> {
    return select(c / 12.92, pow((c + vec3(0.055)) / 1.055, vec3(2.4)), c > vec3(0.04045));
}
@fragment fn fs(in: Out) -> @location(0) vec4<f32> {
    // Minikarte: flache Farben, etwas abgedunkelt, damit Markierungen darauf lesbar bleiben
    if camera.params.y > 0.5 {
        return vec4(linear_color(in.color * 0.78), 1.0);
    }
    let px_per_m = camera.params.x;
    let p = in.uv / px_per_m;
    // Ableitungen vor jeder Verzweigung (einheitlicher Kontrollfluss); die Texturen werden damit per
    // textureSampleGrad auch in Zweigen richtig gefiltert
    let gx = dpdx(p);
    let gy = dpdy(p);
    let n = noise(floor(p * 12.0));
    var factor = 1.0;
    let detail = 1.0 - smoothstep(0.04, 0.3, max(fwidth(p.x), fwidth(p.y)));
    let mid = min(u32(in.material + 0.5), 31u);
    // Rasenrand (mesh.rs fringe): außen ausgefranst, center.x = Lage quer (0 außen … 1 innen)
    if in.material == 16.0 {
        let ragged = vnoise(p * 9.0) * 0.65 + vnoise(p * 23.0 + vec2(5.0, 1.0)) * 0.35;
        if in.across < ragged * 0.95 { discard; }
    }
    let pa = mat.a[mid];
    let pb = mat.b[mid];
    var tint = vec3(1.0);
    var bump = vec2(0.0);
    var occlusion = 1.0;
    var rough = 1.0;
    if pa.x > 0.5 {
        let g = ground_sample(p, gx, gy, pa, pb);
        tint = g.tint;
        bump = g.bump;
        occlusion = g.occlusion;
        rough = g.rough;
    }
    if pa.x > 0.5 {
        // Textur ersetzt die prozeduralen Muster der Fläche
    } else if in.material == 1.0 { factor = 0.96 + (n - 0.5) * 0.12 * detail; }
    if pa.x < 0.5 && (in.material == 2.0 || in.material == 3.0 || in.material == 6.0 || in.material == 7.0) {
        var cell = vec2(0.28, 0.20);
        if in.material == 3.0 { cell = vec2(0.70, 0.60); }
        if in.material == 6.0 || in.material == 7.0 { cell = vec2(0.26, 0.70); }
        let row = floor(p.y / cell.y);
        let st = fract((p + vec2(select(0.0, cell.x * 0.5, i32(row) % 2 == 0), 0.0)) / cell);
        let edge = min(min(st.x, 1.0-st.x), min(st.y, 1.0-st.y));
        factor = mix(1.0, select(0.76, 1.0, edge > 0.04) + (n-0.5)*0.08, detail);
    }
    if in.material == 4.0 && pa.x < 0.5 { factor = 0.95 + noise(floor(p * 2.0)) * 0.12; }
    if in.material == 5.0 { factor = 0.96 + 0.04 * sin(p.x * 1.7 + p.y * 2.1); }
    if in.material == 8.0 { factor = mix(1.0, 0.90 + 0.1*sin(p.y*30.0), detail); }
    if in.material == 9.0 || (in.material == 10.0 && pa.x < 0.5) { factor = 0.97 + (n-0.5)*0.10*detail; }
    var color = in.color * factor * tint * occlusion;
    // am Rasenrand etwas trockener und lichter
    if in.material == 16.0 { color = mix(color * vec3(1.08, 1.02, 0.86), color, in.across); }
    // Nässe (camera.padding2.x): dunklere, glattere Oberfläche; glatte Stellen (geringe Rauheit) stärker
    let wet = camera.padding2.x * select(0.0, 1.0, pa.x > 0.5);
    color *= 1.0 - 0.3 * wet * (1.0 - 0.5 * rough);
    // Gebrauchsspuren (grime.js): großräumig Schmutz in den Senken, ausgeblichene Kuppen – zwei gegeneinander
    // gedrehte Maßstäbe, damit keine Kachel sichtbar wird
    let wp = in.uv;
    let rot = vec2(wp.x * 0.8 - wp.y * 0.6, wp.x * 0.6 + wp.y * 0.8);
    let grime = vnoise(wp * 0.0042) * 0.6 + vnoise(rot * 0.013 + vec2(17.0, 3.0)) * 0.4;
    if in.material == 1.0 || in.material == 2.0 || in.material == 3.0 || in.material == 10.0 {
        let dirt = smoothstep(0.55, 0.85, grime) * 0.16;
        let bleach = smoothstep(0.45, 0.2, grime) * 0.07;
        color = mix(color, vec3(0.16, 0.14, 0.11), dirt);
        color = mix(color, vec3(0.86, 0.84, 0.78), bleach);
    }
    // Dächer: Moos auf Ziegel und Schiefer, Ruß auf Blech und Flachdach
    if in.material >= 6.0 && in.material <= 9.0 {
        let n2 = vnoise(wp * 0.02 + in.center * 0.001);
        let moss = smoothstep(0.62, 0.9, n2) * select(0.1, 0.22, in.material < 8.0);
        color = mix(color, select(vec3(0.15, 0.15, 0.15), vec3(0.3, 0.36, 0.22), in.material < 8.0), moss);
    }
    // Wasser: Lichtreflexe, die mit der Zeit wandern und funkeln
    if in.material == 5.0 {
        let t = camera.sun.w * 3.0;
        var q = p * vec2(0.9, 1.6) + vec2(t * 0.15, 0.0);
        q.x += noise(vec2(floor(q.y), 7.0)) * 3.0; // Zeilen gegeneinander versetzt (kein Raster)
        let cellw = floor(q);
        let spark = noise(cellw + vec2(floor(t), 0.0));
        let inner = 1.0 - length(fract(q) - 0.5) * 2.2;
        let glint = select(0.0, clamp(inner, 0.0, 1.0), spark > 0.965) * detail;
        color = color + vec3(0.55, 0.62, 0.66) * glint * 0.5;
    }
    if in.material == 11.0 || in.material == 12.0 {
        let window = fract(p / vec2(2.5, 3.0));
        let glass = window.x > 0.35 && window.x < 0.72 && window.y > 0.28 && window.y < 0.80;
        color = mix(color, vec3(0.26, 0.32, 0.35), select(0.0, 0.85 * detail, glass));
    }
    return vec4(linear_color(lit(color, normalize(in.normal), bump, rough, wet)), 1.0);
}
// Sonne auf eine (texturierte) Fläche: Grundlicht wie bisher (0,6 + 0,4 · n·Sonne) mit der Reliefnormale, dazu bei
// Nässe ein Sonnenreflex in Richtung der Kamera (von oben) auf der Struktur der Oberfläche.
fn lit(color: vec3<f32>, normal: vec3<f32>, bump: vec2<f32>, rough: f32, wet: f32) -> vec3<f32> {
    let nrm = normalize(normal + vec3(bump, 0.0));
    let sun = normalize(camera.sun.xyz);
    let light = 0.60 + 0.40 * max(0.0, dot(nrm, sun));
    let h = normalize(sun + vec3(0.0, 0.0, 1.0));
    let gloss = pow(max(dot(nrm, h), 0.0), mix(16.0, 96.0, 1.0 - rough));
    let spec = wet * (1.0 - rough) * gloss * 0.25 * clamp(camera.sun.z * 2.0, 0.0, 1.0);
    return clamp(color * light + vec3(spec), vec3(0.0), vec3(1.0));
}
struct Ground { tint: vec3<f32>, bump: vec2<f32>, occlusion: f32, rough: f32 };
// Bodentextur an p (Meter): zwei Maßstäbe (der zweite gedreht und versetzt), weich nach Rauschen gemischt – so
// wiederholt sich keine Kachel sichtbar, und es gibt keine Nähte. gx/gy = Ableitungen von p (vor Verzweigungen
// berechnet), damit die Filterung auch in Zweigen stimmt.
fn ground_sample(p: vec2<f32>, gx: vec2<f32>, gy: vec2<f32>, pa: vec4<f32>, pb: vec4<f32>) -> Ground {
    let layer = i32(pa.x - 0.5);
    let k1 = 1.0 / pa.y;
    let k2 = 1.0 / (pa.y * 2.37);
    let uv1 = p * k1;
    let r = vec2(p.x * 0.8 - p.y * 0.6, p.x * 0.6 + p.y * 0.8);
    let uv2 = r * k2 + vec2(0.37, 0.71);
    let rx = vec2(gx.x * 0.8 - gx.y * 0.6, gx.x * 0.6 + gx.y * 0.8) * k2;
    let ry = vec2(gy.x * 0.8 - gy.y * 0.6, gy.x * 0.6 + gy.y * 0.8) * k2;
    let w = smoothstep(0.3, 0.7, vnoise(p / (pa.y * 3.1) + vec2(11.0, 5.0)));
    let d = mix(
        textureSampleGrad(mat_detail, mat_sampler, uv1, layer, gx * k1, gy * k1).rgb,
        textureSampleGrad(mat_detail, mat_sampler, uv2, layer, rx, ry).rgb, w) * 2.0;
    let q = mix(
        textureSampleGrad(mat_nr, mat_sampler, uv1, layer, gx * k1, gy * k1),
        textureSampleGrad(mat_nr, mat_sampler, uv2, layer, rx, ry), w);
    let lum = dot(d, vec3(0.299, 0.587, 0.114));
    var g: Ground;
    g.tint = mix(vec3(1.0), mix(vec3(lum), d, pa.w), pa.z);
    // OpenGL-Normale: Grün zeigt im Bild nach oben = Norden = −y der Karte
    g.bump = vec2(q.r * 2.0 - 1.0, 1.0 - q.g * 2.0) * pb.x;
    g.occlusion = mix(1.0, q.a, pb.y);
    g.rough = q.b;
    return g;
}
// Hintergrund: wo keine Fläche liegt (unkartiertes Land, Vorgärten, Höfe), zeigte früher die Löschfarbe durch. Jetzt
// ein Vollbild-Durchgang nach den Kacheln, ganz hinten (füllt nur, was frei blieb): Boden in Weltkoordinaten mit dem Material der ID 14 und großen
// Flecken zwischen Rasen und trockener Erde.
@fragment fn ground_fs(in: FullOut) -> @location(0) vec4<f32> {
    let world = camera.position + (in.position.xy - camera.viewport * 0.5) / camera.scale;
    let p = world / camera.params.x;
    let gx = dpdx(p);
    let gy = dpdy(p);
    let g = ground_sample(p, gx, gy, mat.a[14], mat.b[14]);
    let blot = vnoise(p / 23.0) * 0.6 + vnoise(p / 7.0 + vec2(3.1, 8.7)) * 0.4;
    // früher: Löschfarbe linear (0,14 / 0,20 / 0,12) ≈ sRGB (0,41 / 0,48 / 0,38)
    // etwas heller als die alte Löschfarbe: Textur, Verdeckung und Sonnenlicht dunkeln ab; trockene Stellen nur
    // leicht gelblicher (sonst liest sich der Boden schmutzig)
    let base = mix(vec3(0.45, 0.56, 0.39), vec3(0.51, 0.55, 0.40), smoothstep(0.55, 0.85, blot));
    var color = base * g.tint * g.occlusion;
    let wet = camera.padding2.x;
    color *= 1.0 - 0.3 * wet * (1.0 - 0.5 * g.rough);
    return vec4(linear_color(lit(color, vec3(0.0, 0.0, 1.0), g.bump, g.rough, wet)), 1.0);
}
// Erleuchtete Fenster (windows.js): eigener Durchgang nach dem Licht, damit Glühlampenlicht nachts nicht mit der
// Umgebung abgedunkelt wird. Gleiche Fassaden, Tiefe LessEqual ohne Schreiben; alles außer brennenden Scheiben wird
// verworfen. Haus = Grundrissmitte, Fassade = Richtung der Wand, je Etage bilden 2–4 Fenster eine Wohnung.
// camera.params.z = Anteil brennender Fenster (Tagesgang), camera.sun.w = Spieluhr in Minuten.
fn mix32(x0: u32) -> u32 {
    var x = x0;
    x = (x ^ (x >> 16u)) * 0x7feb352du;
    x = (x ^ (x >> 15u)) * 0x846ca68bu;
    return x ^ (x >> 16u);
}
fn whash(a: u32, b: u32, c: u32, d: u32, e: u32) -> f32 {
    var x = mix32(a ^ 0x9e3779b9u);
    x = mix32(x ^ b);
    x = mix32(x ^ c);
    x = mix32(x ^ d);
    x = mix32(x ^ e);
    return f32(x >> 8u) / 16777216.0;
}
fn office_light(m: f32) -> f32 {
    let t = array<vec2<f32>, 8>(vec2(0.0, 0.03), vec2(360.0, 0.04), vec2(420.0, 0.35), vec2(1020.0, 0.45),
        vec2(1140.0, 0.3), vec2(1230.0, 0.1), vec2(1320.0, 0.04), vec2(1440.0, 0.03));
    for (var i = 1; i < 8; i++) {
        if m <= t[i].x {
            let a = t[i - 1];
            let b = t[i];
            return a.y + (b.y - a.y) * (m - a.x) / max(b.x - a.x, 1.0);
        }
    }
    return 0.03;
}
@fragment fn window_fs(in: Out) -> @location(0) vec4<f32> {
    let frac = camera.params.z;
    let minutes = camera.sun.w;
    let m = minutes - floor(minutes / 1440.0) * 1440.0;
    let late = m < 330.0 || m > 1380.0;
    if (in.material != 11.0 && in.material != 12.0) || camera.params.y > 0.5 || (frac <= 0.001 && !late) {
        discard;
    }
    let p = in.uv / camera.params.x;
    let cell_size = vec2(2.5, 3.0);
    let cell = floor(p / cell_size);
    let st = fract(p / cell_size);
    let glass = st.x > 0.35 && st.x < 0.72 && st.y > 0.28 && st.y < 0.80;
    if !glass { discard; }
    let seed = bitcast<u32>(i32(floor(in.center.x))) * 73856093u ^ bitcast<u32>(i32(floor(in.center.y))) * 19349663u;
    let n = normalize(in.normal.xy + vec2(1e-6, 0.0));
    let face = u32(i32(round(atan2(n.y, n.x) / 6.2831853 * 16.0)) + 16) % 16u;
    let col = u32(max(cell.x, 0.0));
    let row = u32(max(cell.y, 0.0));
    let flat_w = 2.0 + floor(whash(seed, face, row, 1u, 0u) * 3.0);
    let flat_i = u32(floor((f32(col) + floor(whash(seed, face, row, 2u, 0u) * flat_w)) / flat_w));
    let home = whash(seed, face, row, flat_i, 3u);
    let room = whash(seed, face, row, col, 4u);
    // Arbeitsstätten: abends noch Licht, nachts fast dunkel, tagsüber nur bei Trübe sichtbar (windows.js OFFICE)
    var lit = frac;
    if in.material == 12.0 {
        lit = office_light(m);
        if m > 420.0 && m < 1140.0 { lit *= 0.7; }
    }
    var on = 0.78 * home + 0.22 * room < lit;
    // nachts kurz Licht in einem einzelnen Raum (Bad, Küche), je 9 Minuten neu ausgewürfelt
    if !on && late { on = whash(seed, face, row, col ^ (u32(floor(minutes / 9.0)) * 2654435761u), 5u) < 0.012; }
    if !on { discard; }
    let k = whash(seed, face, row, flat_i, 6u);
    let evening = m > 1080.0 || m < 120.0;
    // warm, neutral, kaltweiß, Fernseher (flackert bläulich), gedimmt hinter dem Vorhang
    var c = vec3(1.0, 0.812, 0.471);
    var glow = 1.0;
    if k >= 0.58 && k < 0.78 { c = vec3(1.0, 0.925, 0.753); }
    else if k >= 0.78 && k < 0.87 { c = vec3(0.894, 0.925, 1.0); }
    else if k >= 0.87 && k < 0.95 && evening {
        c = vec3(0.616, 0.737, 1.0);
        let fx = f32(col) * 0.7 + in.center.x * 0.001;
        let fy = f32(row) * 1.3 + in.center.y * 0.001;
        glow = 0.55 + 0.45 * abs(sin(minutes * 3.1 + fx) * sin(minutes * 7.3 + fy));
    }
    else if k >= 0.95 { c = vec3(0.878, 0.592, 0.353); }
    // Vorhang: nur der untere Teil der Scheibe leuchtet
    let curtain = whash(seed, face, row, col, 7u) < 0.22;
    let gy = (st.y - 0.28) / 0.52;
    if curtain && gy > 0.55 { discard; }
    // Nebel schluckt das Fensterlicht (render.js fogK)
    let fog_k = 1.0 - 0.65 * min(1.0, camera.fog / 1.4);
    return vec4(linear_color(c * glow) * fog_k, 1.0);
}
// Atlaskoordinate einer Zelle (ATLAS_* stellt atlas.rs voran); q ∈ [−0,5; 0,5]². Ein halber Texel Abstand zum
// Zellrand, damit nicht die Nachbarzelle mitgefiltert wird.
fn atlas_uv(cell: f32, q: vec2<f32>) -> vec2<f32> {
    let uv = vec2(0.5 / ATLAS_CELL) + (q + 0.5) * ((ATLAS_CELL - 1.0) / ATLAS_CELL);
    return (vec2(cell % ATLAS_COLS, floor(cell / ATLAS_COLS)) + uv) / vec2(ATLAS_COLS, ATLAS_ROWS);
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
    out.uv = atlas_uv(cell, q);
    return out;
}
@fragment fn sprite_fs(in: SpriteOut) -> @location(0) vec4<f32> {
    let texel = textureSample(atlas, atlas_sampler, in.uv);
    if texel.a < 0.04 { discard; }
    return vec4(linear_color(texel.rgb * in.color), texel.a);
}
// Bewegte Objekte (Autos, Personen, Marker): instanzierte Rechtecke/Kreise mit weicher Kante (SDF).
// shape 0 = abgerundetes Rechteck, 1 = Ellipse, 2 = Ring, 3 = weicher Fleck (Deckkraft fällt zum Rand auf 0),
// 4/5 = Rechteck/Ellipse mit harter Kante (Bodenschichten: kein halbdeckender Rand, der schon Tiefe schreibt).
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
// Fahrzeugbild aus dem Atlas (shape ≥ 16, Modell = shape − 16): zwei Zellen nebeneinander (Lack, Details), 8 je
// Zeile, je 256 × 128 px. Der Lack trägt die Schattierung s als (s+1)/2; die Farbe des Autos (in.color) wird wie
// assets.js shade aufgehellt bzw. abgedunkelt, die Details liegen darüber.
fn vehicle(in: BodyOut, gx: vec2<f32>, gy: vec2<f32>) -> vec4<f32> {
    let q = in.local / in.extent;
    if abs(q.x) > 1.0 || abs(q.y) > 1.0 { return vec4(0.0); }
    let dims = vec2<f32>(textureDimensions(atlas));
    let cell = vec2(256.0, 128.0);
    let idx = u32(in.shape - 16.0 + 0.5);
    let uv = clamp((q + 1.0) * 0.5, vec2(1.0) / cell, vec2(1.0) - vec2(1.0) / cell);
    let cb = idx * 2u;
    let cd = cb + 1u;
    let pb = (vec2(f32(cb % 8u), f32(cb / 8u)) + uv) * cell / dims;
    let pd = (vec2(f32(cd % 8u), f32(cd / 8u)) + uv) * cell / dims;
    let k = 0.5 * cell / dims;
    let dx = gx / in.extent * k;
    let dy = gy / in.extent * k;
    let b = textureSampleGrad(atlas, atlas_sampler, pb, dx, dy);
    let d = textureSampleGrad(atlas, atlas_sampler, pd, dx, dy);
    let s = b.r * 2.0 - 1.0;
    let c = in.color.rgb;
    let paint = select(c * (1.0 + s), c + (vec3(1.0) - c) * s, s >= 0.0);
    let a = d.a + b.a * (1.0 - d.a);
    let rgb = (paint * b.a * (1.0 - d.a) + d.rgb * d.a) / max(a, 1e-4);
    return vec4(rgb, a * in.color.a);
}
// Deckung des Fahrzeugbilds (Lack und Details) an q ∈ [−1, 1]² – ohne Ableitungen, darf in Verzweigungen stehen.
fn sprite_cover(shape: f32, q: vec2<f32>) -> f32 {
    if abs(q.x) > 1.0 || abs(q.y) > 1.0 { return 0.0; }
    let dims = vec2<f32>(textureDimensions(atlas));
    let cell = vec2(256.0, 128.0);
    let idx = u32(shape - 16.0 + 0.5);
    let uv = clamp((q + 1.0) * 0.5, vec2(1.0) / cell, vec2(1.0) - vec2(1.0) / cell);
    let cb = idx * 2u;
    let cd = cb + 1u;
    let pb = (vec2(f32(cb % 8u), f32(cb / 8u)) + uv) * cell / dims;
    let pd = (vec2(f32(cd % 8u), f32(cd / 8u)) + uv) * cell / dims;
    let b = textureSampleLevel(atlas, atlas_sampler, pb, 0.0).a;
    let d = textureSampleLevel(atlas, atlas_sampler, pd, 0.0).a;
    return d + b * (1.0 - d);
}
// Silhouette verdeckter Figuren und Fahrzeuge („X-Ray“): feine helle Kontur genau am Rand der echten Form, innen
// ein schmaler dunkler Saum (Kontrast auf hellen wie dunklen Dächern) und eine kaum getönte Fläche. in.color.rgb =
// Linienfarbe, in.color.a = Stärke. Formen: Fahrzeugbild (≥ 16), Ellipse (1), Ortungsring (2, nur Linie), Rechteck.
// Alles in Bildschirmpixeln gemessen, damit die Linie bei jedem Zoom gleich fein bleibt.
@fragment fn silhouette_fs(in: BodyOut) -> @location(0) vec4<f32> {
    let px = max(length(fwidth(in.local)) * 0.70710678, 1e-4); // Einheiten je Bildschirmpixel
    let s = in.color.a;
    if in.shape == 2.0 {
        // Ortungsring um die Figur: nur eine feine Linie
        let r = abs(length(in.local) - in.extent.x * 0.92) / px;
        let a = s * 0.6 * (1.0 - smoothstep(0.5, 1.3, r));
        if a < 0.004 { discard; }
        return vec4(linear_color(in.color.rgb), a);
    }
    var d: f32; // Abstand zum Rand in Pixeln, innen negativ
    if in.shape >= 16.0 {
        let q = in.local / in.extent;
        if sprite_cover(in.shape, q) < 0.5 { discard; }
        // Abstand in Ringen zu 1–4 Pixeln, je acht Richtungen
        var dmin = 4.5;
        for (var r = 1; r <= 4; r++) {
            for (var k = 0; k < 8; k++) {
                let a = f32(k) * 0.78539816;
                let o = vec2(cos(a), sin(a)) * f32(r) * px;
                if sprite_cover(in.shape, (in.local + o) / in.extent) < 0.5 { dmin = min(dmin, f32(r)); }
            }
            if dmin < 4.5 { break; }
        }
        d = -dmin + 0.5;
    } else if in.shape == 1.0 {
        let k = length(in.local / in.extent);
        d = (k - 1.0) * min(in.extent.x, in.extent.y) / px;
    } else {
        let e = abs(in.local) - in.extent;
        d = max(e.x, e.y) / px;
    }
    if d > 0.75 { discard; }
    let depth_in = -d;
    let wl = 1.0 - smoothstep(1.2, 1.9, depth_in);                       // Kontur
    let wb = (1.0 - wl) * (1.0 - smoothstep(2.9, 3.6, depth_in));       // dunkler Innensaum
    let wf = max(1.0 - wl - wb, 0.0);                                    // Fläche
    let dark = vec3(0.035, 0.04, 0.055);
    let al = s * wl;
    let ab = s * 0.42 * wb;
    let af = s * 0.1 * wf;
    var a = al + ab + af;
    let rgb = (in.color.rgb * (al + af) + dark * ab) / max(a, 1e-4);
    a *= 1.0 - smoothstep(-0.25, 0.75, d); // weicher Außenrand
    if a < 0.004 { discard; }
    return vec4(linear_color(rgb), a);
}
@fragment fn body_fs(in: BodyOut) -> @location(0) vec4<f32> {
    // Ableitungen vor jeder Verzweigung (einheitlicher Kontrollfluss)
    let gx = dpdx(in.local);
    let gy = dpdy(in.local);
    if in.shape >= 16.0 {
        let v = vehicle(in, gx, gy);
        if v.a < 0.03 { discard; }
        return vec4(linear_color(v.rgb), v.a);
    }
    var d: f32;
    if in.shape == 3.0 {
        let q = length(in.local / in.extent);
        let k = clamp(1.0 - q * q, 0.0, 1.0);
        let alpha = k * k * in.color.a;
        if alpha < 0.004 { discard; }
        return vec4(linear_color(clamp(in.color.rgb, vec3(0.0), vec3(1.0))), alpha);
    }
    if in.shape >= 4.0 {
        let q = in.local / in.extent;
        var inside: bool;
        if in.shape == 5.0 { inside = dot(q, q) <= 1.0; } else { inside = max(abs(q.x), abs(q.y)) <= 1.0; }
        if !inside || in.color.a < 0.004 { discard; }
        return vec4(linear_color(clamp(in.color.rgb, vec3(0.0), vec3(1.0))), in.color.a);
    }
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
    return vec4(linear_color(clamp(in.color.rgb, vec3(0.0), vec3(1.0))), alpha);
}
