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
    var p = center + vec2(local.x * c - local.y * s, local.x * s + local.y * c);
    // Logo (Formen 10–12): `angle` ist die Kursive (Scherung), keine Drehung
    if shape >= 10.0 && shape <= 12.0 { p = center + vec2(local.x - local.y * angle, local.y); }
    var out: HudOut;
    out.position = vec4(p.x / camera.viewport.x * 2.0 - 1.0, 1.0 - p.y / camera.viewport.y * 2.0, 0.0, 1.0);
    out.local = local; out.color = color; out.shape = shape; out.extent = extent; out.extra = extra;
    return out;
}
// Abstandsfeld bilinear (textureLoad, damit der Sampler der Bitmap scharf bleiben kann)
fn sdf_texel(p: vec2<f32>) -> f32 {
    let q = p - 0.5;
    let i = vec2<i32>(floor(q));
    let f = fract(q);
    let a = textureLoad(atlas, i, 0).r;
    let b = textureLoad(atlas, i + vec2(1, 0), 0).r;
    let c = textureLoad(atlas, i + vec2(0, 1), 0).r;
    let e = textureLoad(atlas, i + vec2(1, 1), 0).r;
    return mix(mix(a, b, f.x), mix(c, e, f.x), f.y);
}
fn coverage(d: f32) -> f32 {
    return clamp(0.5 - d / max(fwidth(d), 0.0001), 0.0, 1.0);
}
@fragment fn hud_fs(in: HudOut) -> @location(0) vec4<f32> {
    var a = 1.0;
    if in.shape == 3.0 {
        // Bitmapzeichen: Zelle unter dem SDF-Teil des Atlas, nächster Texel (scharf)
        let uv01 = in.local / in.extent * 0.5 + 0.5;
        let cell = in.extra.x;
        let t = vec2(cell % 16.0, floor(cell / 16.0)) * 8.0 + clamp(uv01, vec2(0.0), vec2(0.999)) * 8.0;
        a = textureLoad(atlas, vec2<i32>(floor(t + vec2(0.0, HUD_BITMAP_Y))), 0).r;
    } else if in.shape >= 10.0 && in.shape <= 12.0 {
        // Logo (Anton): 10 Füllung, 11 Kontur/Extrusion (dick), 12 weicher Schatten
        let uv01 = in.local / in.extent * 0.5 + 0.5;
        let d = sdf_texel(in.extra.xy + uv01 * in.extra.zw);
        let screen_per_atlas = in.extent.y * 2.0 / max(in.extra.w, 1.0);
        let per_px = 0.5 / HUD_TITLE_SPREAD / screen_per_atlas;
        let w = max(fwidth(d) * 0.75, 1e-4);
        if in.shape == 12.0 {
            a = smoothstep(0.12, 0.5, d) * 0.9;
        } else if in.shape == 11.0 {
            let edge = max(0.5 - 3.2 * per_px, 0.12);
            a = smoothstep(edge - w, edge + w, d);
        } else {
            a = smoothstep(0.5 - w, 0.5 + w, d);
            // Verlauf: oben hell, Mitte Farbe, unten satt; harte Glanzkante knapp über der Mitte (Chrom der 80er)
            let v = uv01.y;
            let c = in.color.rgb;
            let top = mix(c, vec3(1.0), 0.55);
            let low = c * vec3(0.92, 0.66, 0.42);
            var col = select(mix(top, c, v / 0.47), mix(c * 0.86, low, (v - 0.47) / 0.53), v > 0.47);
            // Kantenlicht von oben links, dunkle Kante unten rechts (Abstandsfeld als Höhenfeld)
            let g = vec2(dpdx(d), dpdy(d));
            let n = g / max(length(g), 1e-5);
            let rim = 1.0 - smoothstep(0.5, 0.5 + 2.5 * per_px, d);
            let lit = dot(n, normalize(vec2(0.55, 0.85)));
            col = col + vec3(1.0) * rim * max(lit, 0.0) * 0.55 - col * rim * max(-lit, 0.0) * 0.35;
            let alpha = a * in.color.a;
            if alpha < 0.004 { discard; }
            return vec4(linear_color(clamp(col, vec3(0.0), vec3(1.0))), alpha);
        }
    } else if in.shape == 6.0 || in.shape == 7.0 {
        // SDF-Zeichen (Inter): 0,5 = Kante; Kontur (7) als dickere, dunkle Fassung darunter
        let uv01 = in.local / in.extent * 0.5 + 0.5;
        let d = sdf_texel(in.extra.xy + uv01 * in.extra.zw);
        // Bildschirmpixel je Atlaspixel → Konturbreite (1,4 px) im Feld; 1 Atlaspixel = 0,5 / Spannweite
        let screen_per_atlas = in.extent.y * 2.0 / max(in.extra.w, 1.0);
        let per_px = 0.5 / HUD_SDF_SPREAD / screen_per_atlas;
        let edge = select(0.5, max(0.5 - 1.4 * per_px, 0.08), in.shape == 7.0);
        let w = max(fwidth(d) * 0.75, 1e-4);
        a = smoothstep(edge - w, edge + w, d);
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
