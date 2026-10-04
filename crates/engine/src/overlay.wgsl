// Stadtplan (große Karte): Flächen und Linien in Weltkoordinaten, Linienbreite in Bildschirmpunkten.
// camera.params: z = HUD-Maßstab (Pixel je Basiseinheit), w = 1 → feine Linienstufe (ab Zoom 4).
struct OverlayOut { @builtin(position) position: vec4<f32>, @location(0) color: vec4<f32> };
@vertex fn overlay_vs(
    @location(0) pos: vec2<f32>, @location(1) offset: vec2<f32>,
    @location(2) width: vec2<f32>, @location(3) color: vec4<f32>,
) -> OverlayOut {
    let w = select(width.x, width.y, camera.params.w > 0.5) * camera.params.z * 0.5;
    let screen = (pos - camera.position) * camera.scale + offset * w;
    var out: OverlayOut;
    out.position = vec4(screen.x * 2.0 / camera.viewport.x, -screen.y * 2.0 / camera.viewport.y, 0.0, 1.0);
    out.color = color;
    return out;
}
@fragment fn overlay_fs(in: OverlayOut) -> @location(0) vec4<f32> {
    return vec4(linear_color(in.color.rgb), in.color.a);
}
