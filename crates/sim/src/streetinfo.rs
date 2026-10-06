//! Straßenname und nächste Kreuzung für die HUD-Anzeige: die benannte Straße unter dem Spieler und – in Lauf- bzw.
//! Fahrtrichtung entlang dieser Straße – die erste Querstraße mit anderem Namen samt Entfernung. Knoten, an denen
//! die Straße nur unter gleichem Namen weiterläuft (die Karte teilt Straßen an jedem Knoten), werden übersprungen.
use crate::world::World;

/// Was das HUD zeigt.
#[derive(Debug, Clone, PartialEq)]
pub struct StreetLabel {
    pub street: String,
    /// nächste Querstraße voraus: Name und Entfernung (m)
    pub cross: Option<(String, f64)>,
}

/// So weit voraus wird nach einer Kreuzung gesucht (m)
pub const AHEAD_M: f64 = 150.;
/// höchste Straßenklasse, die als Straße zählt (Fußwege und Pfade ausgenommen)
const MAX_CLASS: u8 = 10;
/// Querstraßen ab dieser Klasse zählen nicht (Fußwege an der Ecke sind keine Kreuzung)
const MAX_CROSS_CLASS: u8 = 8;

impl World {
    /// Straße unter dem Spieler (aktiver Sitz) und nächste Kreuzung in Bewegungs- bzw. Blickrichtung.
    pub fn street_label(&mut self) -> Option<StreetLabel> {
        let (x, y, hx, hy) = match self.player_car() {
            Some(c) => {
                let sp = c.vx.hypot(c.vy);
                if sp > 20. {
                    (c.x, c.y, c.vx / sp, c.vy / sp)
                } else {
                    (c.x, c.y, c.angle.cos(), c.angle.sin())
                }
            }
            None => (
                self.player.x,
                self.player.y,
                self.player.angle.cos(),
                self.player.angle.sin(),
            ),
        };
        street_label_at(self, x, y, (hx, hy))
    }
}

/// Straße bei (x, y) und nächste Kreuzung in Richtung `heading` (Einheitsvektor).
pub fn street_label_at(w: &mut World, x: f64, y: f64, heading: (f64, f64)) -> Option<StreetLabel> {
    let s = w.city.scale;
    let near = w
        .city
        .nearest_edge(x, y, 25. * s, |e| !e.name.is_empty() && e.cls <= MAX_CLASS)?;
    let e = w.city.edges.get(&near.edge)?.clone();
    if near.d > e.w / 2. + 8. * s {
        return None;
    }
    // Richtung entlang der Kante: zum Ende b, wenn die Kante dort hinzeigt, wohin man sich bewegt
    let forward = near.ux * heading.0 + near.uy * heading.1 >= 0.;
    let (mut node, mut dist) = if forward {
        (e.b, e.len - near.s)
    } else {
        (e.a, near.s)
    };
    let mut cur = e.id;
    let limit = AHEAD_M * s;
    let mut cross = None;
    for _ in 0..8 {
        if dist > limit {
            break;
        }
        let Some(nd) = w.city.nodes.get(&node) else {
            break;
        };
        // andere benannte Straße am Knoten: Kreuzung
        let other = nd
            .edges
            .iter()
            .filter(|&&k| k != cur)
            .filter_map(|k| w.city.edges.get(k))
            .find(|o| !o.name.is_empty() && o.name != e.name && o.cls <= MAX_CROSS_CLASS);
        if let Some(o) = other {
            cross = Some((o.name.clone(), dist / s));
            break;
        }
        // sonst geradeaus weiter auf der Straße gleichen Namens
        let Some(next) = nd
            .edges
            .iter()
            .filter(|&&k| k != cur)
            .filter_map(|k| w.city.edges.get(k))
            .find(|o| o.name == e.name)
        else {
            break;
        };
        let far = if next.a == node { next.b } else { next.a };
        dist += next.len;
        cur = next.id;
        node = far;
    }
    Some(StreetLabel {
        street: e.name,
        cross,
    })
}
