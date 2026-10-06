//! Handgranate und Molotow auf den echten Kacheln.
use berlin_sim::car::{Car, Role};
use berlin_sim::city::{City, DiskSource};
use berlin_sim::combat::WEAPONS;
use berlin_sim::events::Event;
use berlin_sim::pedestrians::PedState;
use berlin_sim::throw::{CHARGE_MIN, CHARGE_PERIOD, FLAME_S, FUSE_S, Kind, charge_reach};
use berlin_sim::world::{DT, Input, World};

fn world(seed: u32, peds: usize) -> World {
    let root = berlin_map_loader::default_data_root();
    let city = City::open(&root, Box::new(DiskSource::new(root.clone()))).expect("Karte");
    let mut w = World::new(city, seed, 0, peds);
    for _ in 0..30 {
        w.update(&Input::default(), DT);
    }
    w
}
fn slot(id: &str) -> u8 {
    WEAPONS.iter().position(|w| w.id == id).unwrap() as u8 + 1
}

#[test]
fn grenade_is_thrown_with_the_fire_button_flies_and_explodes_after_the_fuse() {
    let mut w = world(51, 30);
    let (px, py) = (w.player.x, w.player.y);
    // Granate wählen, dann zum Mauspunkt 12 m daneben werfen
    let mut inp = Input::default();
    inp.combat.weapon_slot = slot("grenade");
    w.update(&inp, DT);
    for _ in 0..20 {
        w.update(&Input::default(), DT);
    }
    let mut inp = Input::default();
    inp.combat.fire = true;
    inp.combat.fire_pressed = true;
    inp.combat.aim_world = Some((px + 120., py));
    // drücken holt aus (noch kein Wurf), loslassen wirft
    w.update(&inp, DT);
    assert!(w.player.combat.charge.is_some(), "holt aus");
    assert!(w.thrown.is_empty());
    let mut hold = inp;
    hold.combat.fire_pressed = false;
    for _ in 0..20 {
        w.update(&hold, DT);
    }
    assert!(w.thrown.is_empty(), "solange gehalten, kein Wurf");
    let mut rel = Input::default();
    rel.combat.aim_world = Some((px + 120., py));
    w.update(&rel, DT);
    assert!(w.events.iter().any(|e| matches!(
        e,
        Event::Throw {
            weapon: "grenade",
            ..
        }
    )));
    assert!(w.player.combat.charge.is_none());
    assert_eq!(w.thrown.len(), 1);
    assert_eq!(w.player.combat.mag[6], WEAPONS[6].mag - 1, "eine weniger");
    // ein Passant läuft immer neben die Granate
    let victim = w
        .peds
        .iter()
        .position(|p| p.state != PedState::Dead)
        .expect("Passant");
    let vid = w.peds[victim].id;
    let mut boom = None;
    let mut max_z: f64 = 0.;
    for k in 0..((FUSE_S + 0.5) / DT) as usize {
        if let Some(g) = w.thrown.first() {
            max_z = max_z.max(g.z);
            let (gx, gy) = (g.x, g.y);
            if let Some(p) = w.peds.iter_mut().find(|p| p.id == vid) {
                (p.x, p.y) = (gx, gy + 20.);
            }
        }
        w.update(&Input::default(), DT);
        if w.events
            .iter()
            .any(|e| matches!(e, Event::Explosion { car: None, .. }))
        {
            boom = Some(k);
            break;
        }
    }
    let k = boom.expect("explodiert");
    let t = (k + 1) as f64 * DT;
    assert!((t - FUSE_S).abs() < 0.1, "nach der Zündzeit: {t}");
    assert!(max_z > 15., "Bogen: höchstens {max_z}");
    assert!(w.thrown.is_empty());
    let p = w.peds.iter().find(|p| p.id == vid);
    assert!(
        p.is_none_or(|p| p.state == PedState::Dead || p.hp < 100.),
        "Passant getroffen"
    );
}

#[test]
fn molotov_shatters_and_its_fire_burns_people_and_cars() {
    let mut w = world(52, 30);
    let (px, py) = (w.player.x, w.player.y);
    w.throw(Kind::Molotov, 0., 100.);
    let mut at = None;
    for _ in 0..120 {
        w.update(&Input::default(), DT);
        if let Some(Event::Shatter { x, y, .. }) =
            w.events.iter().find(|e| matches!(e, Event::Shatter { .. }))
        {
            at = Some((*x, *y));
            break;
        }
    }
    let (fx, fy) = at.expect("zerschellt");
    assert!(w.thrown.is_empty() && w.flames.len() == 1);
    assert!(
        (fx - px) > 10. && (fx - px) < 140. && (fy - py).abs() < 20.,
        "Landung {fx},{fy}"
    );
    // ein Auto mitten ins Feuer, ein Passant hinein
    w.cars
        .push(Car::new(9_990, fx, fy, 0., 0x445566, Role::Parked, "car"));
    let vid = w.peds[0].id;
    for _ in 0..(FLAME_S / DT) as usize {
        if let Some(p) = w.peds.iter_mut().find(|p| p.id == vid)
            && p.state != PedState::Dead
        {
            (p.x, p.y) = (fx + 5., fy);
        }
        w.update(&Input::default(), DT);
    }
    let p = w.peds.iter().find(|p| p.id == vid);
    assert!(
        p.is_none_or(|p| p.state == PedState::Dead || p.hp < 100.),
        "Passant verbrannt"
    );
    let c = w.car(9_990).expect("Auto");
    assert!(c.health < 100., "Auto beschädigt: {}", c.health);
    w.update(&Input::default(), DT);
    assert!(w.flames.is_empty(), "Feuer erlischt");
}

#[test]
fn own_grenade_hurts_the_thrower() {
    let mut w = world(53, 0);
    let hp0 = w.player.combat.hp;
    // senkrecht nach oben geworfen landet sie (fast) vor den Füßen
    w.throw(Kind::Grenade, 0., 1.);
    for _ in 0..((FUSE_S + 0.2) / DT) as usize {
        if let Some(g) = w.thrown.first_mut() {
            (g.x, g.y) = (w.player.x + 10., w.player.y);
        }
        w.update(&Input::default(), DT);
    }
    assert!(w.player.combat.hp < hp0, "Werfer selbst verletzt");
}

#[test]
fn grenade_spares_the_coop_partner() {
    let mut w = world(54, 0);
    assert!(w.join_p2(), "Spieler 2 dabei");
    let (x2, y2) = {
        let p2 = &w.p2.as_ref().unwrap().player;
        (p2.x, p2.y)
    };
    let hp2 = w.p2.as_ref().unwrap().player.combat.hp;
    // Spieler 1 weit weg, die Granate liegt Spieler 2 vor den Füßen
    w.player.x += 900.;
    w.throw(Kind::Grenade, 0., 1.);
    for _ in 0..((FUSE_S + 0.2) / DT) as usize {
        if let Some(g) = w.thrown.first_mut() {
            (g.x, g.y) = (x2 + 8., y2);
        }
        w.update(&Input::default(), DT);
    }
    let p2 = &w.p2.as_ref().unwrap().player;
    assert_eq!(p2.combat.hp, hp2, "kein Eigenbeschuss");
}

#[test]
fn reach_swings_back_and_forth_while_holding() {
    let r = 280.;
    assert!(
        (charge_reach(r, 0.) - r * CHARGE_MIN).abs() < 1e-9,
        "kurz beim Drücken"
    );
    assert!(
        (charge_reach(r, CHARGE_PERIOD / 2.) - r).abs() < 1e-9,
        "ganz weit nach einer halben Periode"
    );
    assert!(
        (charge_reach(r, CHARGE_PERIOD) - r * CHARGE_MIN).abs() < 1e-9,
        "und zurück"
    );
    let a = charge_reach(r, CHARGE_PERIOD * 0.25);
    assert!(a > r * CHARGE_MIN && a < r);
    assert!((charge_reach(r, 0.3) - charge_reach(r, CHARGE_PERIOD + 0.3)).abs() < 1e-9);
}

/// Molotow mit der Feuertaste: `frames` Schritte halten, dann loslassen; Abstand des Aufschlags.
fn throw_held(seed: u32, frames: usize) -> f64 {
    let mut w = world(seed, 0);
    let mut inp = Input::default();
    inp.combat.weapon_slot = slot("molotov");
    w.update(&inp, DT);
    for _ in 0..20 {
        w.update(&Input::default(), DT);
    }
    let (px, py) = (w.player.x, w.player.y);
    // schräg zur Straße (am Startpunkt steht die Figur vor einer Hauswand)
    let mut hold = Input::default();
    (hold.combat.aim_x, hold.combat.aim_y) = (2.2f64.cos(), 2.2f64.sin());
    hold.combat.fire = true;
    hold.combat.fire_pressed = true;
    w.update(&hold, DT);
    hold.combat.fire_pressed = false;
    for _ in 0..frames {
        w.update(&hold, DT);
    }
    let mut rel = Input::default();
    (rel.combat.aim_x, rel.combat.aim_y) = (hold.combat.aim_x, hold.combat.aim_y);
    w.update(&rel, DT);
    for _ in 0..200 {
        w.update(&Input::default(), DT);
        if let Some(Event::Shatter { x, y, .. }) =
            w.events.iter().find(|e| matches!(e, Event::Shatter { .. }))
        {
            return (x - px).hypot(y - py);
        }
    }
    panic!("kein Aufschlag");
}

#[test]
fn holding_longer_throws_farther() {
    let quarter = (CHARGE_PERIOD / 2. / DT) as usize / 2;
    let short = throw_held(61, 2);
    let mid = throw_held(61, quarter);
    assert!(
        mid > short + 20.,
        "länger gehalten, weiter: {short} → {mid}"
    );
}

#[test]
fn cancel_drops_the_charge_without_throwing() {
    let mut w = world(62, 0);
    let mut inp = Input::default();
    inp.combat.weapon_slot = slot("grenade");
    w.update(&inp, DT);
    for _ in 0..20 {
        w.update(&Input::default(), DT);
    }
    let n0 = w.player.combat.mag[6];
    let mut hold = Input::default();
    hold.combat.fire = true;
    hold.combat.fire_pressed = true;
    w.update(&hold, DT);
    hold.combat.fire_pressed = false;
    for _ in 0..10 {
        w.update(&hold, DT);
    }
    let mut c = hold;
    c.combat.cancel = true;
    w.update(&c, DT);
    assert!(w.player.combat.charge.is_none(), "abgebrochen");
    // Taste noch gehalten, dann losgelassen: kein Wurf, Vorrat unverändert
    for _ in 0..5 {
        w.update(&hold, DT);
    }
    w.update(&Input::default(), DT);
    for _ in 0..30 {
        w.update(&Input::default(), DT);
    }
    assert!(w.thrown.is_empty());
    assert_eq!(w.player.combat.mag[6], n0);
}

#[test]
fn preview_ends_where_the_molotov_shatters() {
    let mut w = world(63, 0);
    for ang in [0.6f64, 2.4, 4.0] {
        let pts = w.throw_preview(Kind::Molotov, ang, 160.);
        assert!(pts.len() > 5);
        let end = *pts.last().unwrap();
        let top = pts.iter().map(|p| p.2).fold(0., f64::max);
        assert!(top > 15., "Bogen");
        w.throw(Kind::Molotov, ang, 160.);
        let mut at = None;
        for _ in 0..200 {
            w.update(&Input::default(), DT);
            if let Some(Event::Shatter { x, y, .. }) =
                w.events.iter().find(|e| matches!(e, Event::Shatter { .. }))
            {
                at = Some((*x, *y));
                break;
            }
        }
        let (x, y) = at.expect("zerschellt");
        let off = (x - end.0).hypot(y - end.1);
        assert!(off < 6., "Vorschau trifft: {off} px daneben (Winkel {ang})");
        w.flames.clear();
    }
}
