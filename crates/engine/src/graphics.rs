//! Grafikmodus und Qualitätsstufe: das Spiel wählt (Einstellungen, Menü, Taste, Konsole, CLI), der Renderer baut
//! danach seine Ziele und Pipelines. Nur Darstellung – die Simulation weiß davon nichts.

/// HD (Standard): Szene in einem Float-Ziel mit Kantenglättung, danach Nachbearbeitung. Pixel: eigener Pfad mit
/// grobem Raster, Konturen und fester Palette (bis Phase 8 der Grafik-Überarbeitung wie HD ohne Kantenglättung).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphicsMode {
    #[default]
    Hd,
    Pixel,
}

/// Qualitätsstufe des HD-Pfads (Kantenglättung, später Texturgrößen und Effektdichte).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quality {
    Niedrig,
    Mittel,
    #[default]
    Hoch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GraphicsSettings {
    pub mode: GraphicsMode,
    pub quality: Quality,
}

impl GraphicsMode {
    pub const ALL: [Self; 2] = [Self::Hd, Self::Pixel];
    /// Name in `settings.json`, Konsole und CLI.
    pub fn key(self) -> &'static str {
        match self {
            Self::Hd => "hd",
            Self::Pixel => "pixel",
        }
    }
    /// Anzeige im Menü.
    pub fn label(self) -> &'static str {
        match self {
            Self::Hd => "HD",
            Self::Pixel => "Pixel",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|m| m.key().eq_ignore_ascii_case(s.trim()))
    }
    pub fn toggled(self) -> Self {
        match self {
            Self::Hd => Self::Pixel,
            Self::Pixel => Self::Hd,
        }
    }
}

impl Quality {
    pub const ALL: [Self; 3] = [Self::Niedrig, Self::Mittel, Self::Hoch];
    pub fn key(self) -> &'static str {
        match self {
            Self::Niedrig => "niedrig",
            Self::Mittel => "mittel",
            Self::Hoch => "hoch",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|q| q.key().eq_ignore_ascii_case(s.trim()))
    }
}

impl GraphicsSettings {
    /// Abtastungen je Bildpunkt der Szene: Pixel-Modus und Niedrig ohne Kantenglättung, sonst 4× (in WebGPU für
    /// `Rgba16Float` und `Depth32Float` garantiert – Metal, DX12 und Vulkan ohne optionale Merkmale).
    pub fn msaa(self) -> u32 {
        match (self.mode, self.quality) {
            (GraphicsMode::Pixel, _) | (_, Quality::Niedrig) => 1,
            _ => 4,
        }
    }
}

impl GraphicsSettings {
    /// Stufe der Nachbearbeitung für den Shader (`camera.padding2.y`): 0 = ohne HDR-Bloom und weiche Schatten
    /// (Niedrig, Pixel), 1 = Bloom ½ und Schattenkante 1,2 px (Mittel), 2 = Bloom ½ + ¼ und 2 px (Hoch).
    pub fn post_level(self) -> u32 {
        match (self.mode, self.quality) {
            (GraphicsMode::Pixel, _) | (_, Quality::Niedrig) => 0,
            (_, Quality::Mittel) => 1,
            (_, Quality::Hoch) => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_msaa() {
        let g = GraphicsSettings::default();
        assert_eq!((g.mode, g.quality), (GraphicsMode::Hd, Quality::Hoch));
        assert_eq!(g.msaa(), 4);
        let q = |mode, quality| GraphicsSettings { mode, quality }.msaa();
        assert_eq!(q(GraphicsMode::Hd, Quality::Mittel), 4);
        assert_eq!(q(GraphicsMode::Hd, Quality::Niedrig), 1);
        assert_eq!(q(GraphicsMode::Pixel, Quality::Hoch), 1);
        let l = |mode, quality| GraphicsSettings { mode, quality }.post_level();
        assert_eq!(l(GraphicsMode::Hd, Quality::Hoch), 2);
        assert_eq!(l(GraphicsMode::Hd, Quality::Mittel), 1);
        assert_eq!(l(GraphicsMode::Hd, Quality::Niedrig), 0);
        assert_eq!(l(GraphicsMode::Pixel, Quality::Hoch), 0);
    }
    #[test]
    fn names_round_trip() {
        for m in GraphicsMode::ALL {
            assert_eq!(GraphicsMode::parse(m.key()), Some(m));
            assert_eq!(m.toggled().toggled(), m);
        }
        for q in Quality::ALL {
            assert_eq!(Quality::parse(q.key()), Some(q));
        }
        assert_eq!(GraphicsMode::parse(" PIXEL "), Some(GraphicsMode::Pixel));
        assert_eq!(GraphicsMode::parse("4k"), None);
        assert_eq!(Quality::parse("ultra"), None);
    }
}
