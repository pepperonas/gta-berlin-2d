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
/// Figurenteile (Phase 6, Variante B): ein Zellenpaar des Atlas ist in FIG_COLS × FIG_ROWS Teilzellen geteilt, je
/// Teil eine. Body-Form `FIG_BASE + Paar · FIG_PARTS + Teil` (das Paar liefert das Spiel, `carart::figure_pair`).
pub const FIG_BASE: u32 = 4096;
pub const FIG_COLS: u32 = 8;
pub const FIG_ROWS: u32 = 4;
pub const FIG_PARTS: u32 = FIG_COLS * FIG_ROWS;
/// Bildpunkte einer Teilzelle (Kante); die Zeichnung läuft bis 1/8 vor den Rand (Mip-Stufen bluten sonst über).
pub const FIG_CELL: usize = CELL_W / FIG_COLS as usize;
/// Anteil der Teilzelle, den die Zeichnung belegt (q = ±1 ist der Zellrand)
pub const FIG_ART: f32 = 0.875;

pub fn shader_constants() -> String {
    format!(
        "const VEH_COLS: f32 = {:?};\nconst VEH_ASPECT: f32 = {:?};\nconst VEH_MAT_GLASS: f32 = {MAT_GLASS:?};\n\
         const FIG_BASE: f32 = {:?};\nconst FIG_COLS: f32 = {:?};\nconst FIG_ROWS: f32 = {:?};\n",
        COLS as f32,
        CELL_W as f32 / CELL_H as f32,
        FIG_BASE as f32,
        FIG_COLS as f32,
        FIG_ROWS as f32
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn figure_cells_tile_the_pair() {
        use super::*;
        assert_eq!(FIG_CELL * FIG_COLS as usize, CELL_W);
        assert_eq!(FIG_CELL * FIG_ROWS as usize, CELL_H);
        assert!(
            FIG_BASE as usize > 16 + 4 * COLS * 64,
            "Fahrzeugmodelle und Figurenteile überlappen nicht"
        );
    }
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
