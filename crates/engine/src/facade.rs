//! Fensterraster der Fassaden – eine Quelle für die Tagesansicht (`scene.wgsl fs`) und die erleuchteten Fenster
//! (`window_fs`): sonst brennt nachts Licht neben der gezeichneten Scheibe. Die Werte gehen als WGSL-Konstanten in den
//! Shader (`shader_constants`, vorangestellt in `renderer::shader_source`).

/// Rasterzelle eines Fensters in Metern (Breite entlang der Wand, Geschosshöhe).
pub const WINDOW_CELL_M: [f32; 2] = [2.5, 3.0];
/// Scheibe in der Zelle (Anteil von links bzw. von unten).
pub const GLASS_X: [f32; 2] = [0.35, 0.72];
pub const GLASS_Y: [f32; 2] = [0.28, 0.80];

pub fn shader_constants() -> String {
    format!(
        "const WINDOW_CELL: vec2<f32> = vec2({:?}, {:?});\nconst GLASS_X: vec2<f32> = vec2({:?}, {:?});\nconst GLASS_Y: vec2<f32> = vec2({:?}, {:?});\n",
        WINDOW_CELL_M[0], WINDOW_CELL_M[1], GLASS_X[0], GLASS_X[1], GLASS_Y[0], GLASS_Y[1]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_grid_has_one_source() {
        let c = shader_constants();
        assert!(
            c.contains("const WINDOW_CELL: vec2<f32> = vec2(2.5, 3.0);"),
            "{c}"
        );
        let src = crate::renderer::shader_source();
        assert!(src.contains(&c));
        // weder die Tagesansicht noch die erleuchteten Fenster dürfen eigene Zahlen tragen
        for bad in ["vec2(2.5, 3.0)", "0.35 && ", "> 0.28 &&"] {
            assert_eq!(
                src.matches(bad).count(),
                usize::from(bad == "vec2(2.5, 3.0)"),
                "fest verdrahtetes Fensterraster: {bad}"
            );
        }
        // beide Durchgänge nutzen dieselbe Funktion
        assert!(
            src.matches("window_cell(").count() >= 3,
            "fs und window_fs über window_cell"
        );
        const { assert!(GLASS_X[0] < GLASS_X[1] && GLASS_Y[0] < GLASS_Y[1]) };
    }
}
