//! Recherchierte Bahnsteigebenen der großen Berliner Umsteigebahnhöfe (`data/station-levels.json`, Stand und
//! Quellen dort). Ebene relativ zur Straße: +2 zweite Hochebene, +1 Hochbahn/Damm, 0 ebenerdig, −1 Einschnitt
//! oder flacher Tunnel, −2, −3 darunter. Belegt ist vor allem die Reihenfolge an einem Bahnhof; wo die Recherche
//! unsicher war, steht es in der Datei. Ohne Eintrag entscheidet `station.rs` selbst (Tunnel → −1, Gleisebene der
//! Karte).
use std::collections::HashMap;
use std::sync::OnceLock;

const DATA: &str = include_str!("../../../data/station-levels.json");

/// Ein Bahnsteig aus der Tabelle: Linien darauf und Ebene.
#[derive(Debug, Clone, PartialEq)]
pub struct Curated {
    pub lines: Vec<String>,
    pub level: i8,
    pub unsure: bool,
}

/// Linienkürzel in einem freien Text: „U1 U3 (Richtung …) + U4“ → U1, U3, U4.
pub fn line_tokens(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| {
            let mut c = t.chars();
            matches!(c.next(), Some('U' | 'S'))
                && t.len() >= 2
                && c.next().is_some_and(|d| d.is_ascii_digit())
                && t[1..].chars().all(|d| d.is_ascii_digit())
        })
        .map(str::to_string)
        .collect()
}

fn table() -> &'static HashMap<String, Vec<Curated>> {
    static T: OnceLock<HashMap<String, Vec<Curated>>> = OnceLock::new();
    T.get_or_init(|| {
        let j: serde_json::Value = serde_json::from_str(DATA).unwrap_or_default();
        let mut out: HashMap<String, Vec<Curated>> = HashMap::new();
        for s in j["stations"].as_array().into_iter().flatten() {
            let Some(name) = s["name"].as_str() else {
                continue;
            };
            let plats: Vec<Curated> = s["platforms"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| {
                    let lines = line_tokens(p["line"].as_str()?);
                    let level = p["level"].as_i64()? as i8;
                    (!lines.is_empty()).then(|| Curated {
                        lines,
                        level,
                        unsure: p["unsicher"].as_bool().unwrap_or(false),
                    })
                })
                .collect();
            let mut keys = vec![crate::station::match_key(name)];
            for o in s["osm_names"].as_array().into_iter().flatten() {
                if let Some(o) = o.as_str() {
                    keys.push(crate::station::match_key(o));
                }
            }
            keys.sort();
            keys.dedup();
            for k in keys {
                out.entry(k).or_default().extend(plats.iter().cloned());
            }
        }
        out
    })
}

/// Recherchierte Ebene des Bahnsteigs, an dem `lines` an der Station `name` halten (erster passender Eintrag;
/// bei Richtungsbahnsteigen übereinander ist das der obere).
pub fn curated_level(name: &str, lines: &[String]) -> Option<i8> {
    let plats = table().get(&crate::station::match_key(name))?;
    plats
        .iter()
        .find(|p| p.lines.iter().any(|l| lines.contains(l)))
        .map(|p| p.level)
}

/// Anzahl der Stationen in der Tabelle (für Tests und das Protokoll).
pub fn stations() -> usize {
    serde_json::from_str::<serde_json::Value>(DATA)
        .ok()
        .and_then(|j| j["stations"].as_array().map(Vec::len))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn tokens_pick_line_names_only() {
        assert_eq!(
            line_tokens("U1 U3 (Richtung Warschauer Straße) + U4 (Richtung Innsbrucker Platz)"),
            l(&["U1", "U3", "U4"])
        );
        assert_eq!(
            line_tokens("S41 S42 S8 S85 (Ringbahn)"),
            l(&["S41", "S42", "S8", "S85"])
        );
        assert!(line_tokens("Regional-/Fernverkehr, keine S-Bahn").is_empty());
    }

    #[test]
    fn known_interchanges_have_their_researched_levels() {
        assert!(stations() >= 50, "{}", stations());
        // Alexanderplatz: S-Bahn auf dem Viadukt, darunter U2, U8, U5 (die tiefste)
        assert_eq!(
            curated_level("S+U Alexanderplatz Bhf (Berlin)", &l(&["S5"])),
            Some(1)
        );
        assert_eq!(curated_level("Alexanderplatz", &l(&["U2"])), Some(-1));
        assert_eq!(curated_level("Alexanderplatz", &l(&["U8"])), Some(-2));
        assert_eq!(curated_level("Alexanderplatz", &l(&["U5"])), Some(-3));
        // Kottbusser Tor: U1 oben auf dem Viadukt, U8 unten
        assert_eq!(
            curated_level("U Kottbusser Tor (Berlin)", &l(&["U1"])),
            Some(1)
        );
        assert_eq!(curated_level("Kottbusser Tor", &l(&["U8"])), Some(-2));
        // Gleisdreieck: zwei Hochebenen
        assert_eq!(curated_level("Gleisdreieck", &l(&["U1"])), Some(2));
        assert_eq!(curated_level("Gleisdreieck", &l(&["U2"])), Some(1));
        // Hermannplatz: U8 flach, U7 darunter
        assert_eq!(curated_level("Hermannplatz", &l(&["U8"])), Some(-1));
        assert_eq!(curated_level("Hermannplatz", &l(&["U7"])), Some(-2));
        // unbekannte Station bzw. Linie
        assert_eq!(curated_level("Boddinstraße", &l(&["U8"])), None);
        assert_eq!(curated_level("Hermannplatz", &l(&["U9"])), None);
    }
}
