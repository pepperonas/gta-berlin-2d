//! Raster des Fahrzeugatlas (gemalt von `game/carart.rs`) – eine Quelle für den Maler und den Shader. Je Modell zwei
//! Zellen nebeneinander (Lack, Details). Der Shader rechnet die Zellgröße aus der Atlasbreite und `VEH_COLS`, ein
//! halb aufgelöster Atlas (Qualitätsstufe) braucht also keine zweite Zahl.
//!
//! Lackzelle: R = Schattierung (s+1)/2, G = Glanzstärke, B = Material (0 Lack, ½ Glas, 1 Chrom), A = Deckkraft.

pub const CELL_W: usize = 512;
pub const CELL_H: usize = 256;
pub const COLS: usize = 8;
pub const MAT_GLASS: f32 = 0.5;
pub const MAT_CHROME: f32 = 1.0;

pub fn shader_constants() -> String {
    format!(
        "const VEH_COLS: f32 = {:?};\nconst VEH_ASPECT: f32 = {:?};\nconst VEH_MAT_GLASS: f32 = {MAT_GLASS:?};\n",
        COLS as f32,
        CELL_W as f32 / CELL_H as f32
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn grid_reaches_the_shader_without_literals() {
        let src = crate::renderer::shader_source();
        assert!(src.contains(&super::shader_constants()));
        // die alten festen Zellmaße dürfen nicht mehr im Shader stehen
        for bad in ["vec2(256.0, 128.0)", "vec2(512.0, 256.0)", "% 8u", "/ 8u"] {
            assert!(!src.contains(bad), "festes Fahrzeugraster im Shader: {bad}");
        }
    }
}
