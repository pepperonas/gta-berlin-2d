//! Nachtleben (Port von `nightlife.js`, rein rechnerisch): wie voll eine Bar, Kneipe oder ein Club gerade ist und
//! was man davon an der Kamera hört. Grundlage sind die Lokale aus OpenStreetMap (POI-Kategorie `drink`) mit einem
//! typischen Verlauf nach Uhrzeit und Wochentag. Liegt ein Auslastungs-Feed vor (gostumblr-Format oder allgemein),
//! zählt für die dort genannten Bars dessen Wochenprofil bzw. die gemeldete Auslastung. Alles hier ist Darstellung
//! und Klang: der Welt-Zufall bleibt unberührt, nur `life.rs` stellt vor vollen Feed-Bars mehr Leute auf den Gehweg.
use crate::city::{City, Poi};
use crate::collision::Rect;
use crate::rhythm::{nightlife, wrap};
use serde_json::Value;
use std::collections::HashMap;

/// px: so weit trägt das Stimmengewirr vor einer vollen Bar (70 m)
pub const HEAR: f64 = 700.;
/// px: Bars aus dem Feed (meist größer, Leute stehen draußen)
pub const HEAR_FEED: f64 = 900.;
/// px: Feed-Bar mit gleichem Namen darf so weit vom OSM-Punkt liegen
pub const MATCH: f64 = 1500.;
/// px: Feed-Bar ohne Namensgleichheit gilt als dieselbe, wenn sie so nah am OSM-Punkt liegt
pub const NEAR: f64 = 300.;
/// s: ältere Live-Werte zählen nicht mehr (das Wochenprofil bleibt)
pub const STALE: f64 = 6. * 3600.;

fn clamp01(v: f64) -> f64 {
    v.clamp(0., 1.)
}

/// Name vergleichbar machen: klein, ohne Akzente/Satzzeichen und Allerweltswörter („Bar“, „Berlin“, „Kneipe“ …).
pub fn norm_name(s: &str) -> String {
    const STOP: [&str; 12] = [
        "bar", "the", "berlin", "kneipe", "pub", "club", "cafe", "die", "der", "das", "zum", "zur",
    ];
    let mut flat = String::new();
    for c in s.chars().flat_map(char::to_lowercase) {
        match c {
            'ß' => flat.push_str("ss"),
            'ä' | 'á' | 'à' | 'â' => flat.push('a'),
            'ö' | 'ó' | 'ò' | 'ô' => flat.push('o'),
            'ü' | 'ú' | 'ù' | 'û' => flat.push('u'),
            'é' | 'è' | 'ê' | 'ë' => flat.push('e'),
            'í' | 'ì' | 'î' | 'ï' => flat.push('i'),
            c if c.is_ascii_alphanumeric() => flat.push(c),
            _ => flat.push(' '),
        }
    }
    flat.split(' ')
        .filter(|w| !w.is_empty() && !STOP.contains(w))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Auslastung 0…1 aus 0…1 oder Prozent.
fn level(v: &Value) -> Option<f64> {
    let n = match v {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s.trim().parse().ok()?,
        _ => return None,
    };
    n.is_finite()
        .then(|| clamp01(if n > 1. { n / 100. } else { n }))
}
fn pick<'a>(o: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().filter_map(|k| o.get(k)).find(|v| !v.is_null())
}

/// Wochenprofil: 7 Tage (Mo zuerst) × 24 Stunden, `None` = unbekannt.
pub type Week = [Option<[Option<f64>; 24]>; 7];

const DAY_NAMES: [[&str; 4]; 7] = [
    ["monday", "montag", "mo", "mon"],
    ["tuesday", "dienstag", "di", "tue"],
    ["wednesday", "mittwoch", "mi", "wed"],
    ["thursday", "donnerstag", "do", "thu"],
    ["friday", "freitag", "fr", "fri"],
    ["saturday", "samstag", "sa", "sat"],
    ["sunday", "sonntag", "so", "sun"],
];
fn day_index(name: &str) -> Option<usize> {
    let n: String = name.to_lowercase().chars().take(9).collect();
    let n = n.trim();
    DAY_NAMES.iter().position(|a| a.contains(&n))
}
fn hours(v: &Value) -> Option<[Option<f64>; 24]> {
    let a = v.as_array().filter(|a| a.len() >= 24)?;
    let mut out = [None; 24];
    for (i, x) in a.iter().take(24).enumerate() {
        out[i] = Some(level(x).unwrap_or(0.));
    }
    Some(out)
}
/// Wochenprofil lesen: `[{ name: 'Monday', data: [24] }]` (Google-„Stoßzeiten“), `{ mo: [24], … }`,
/// `[[24] × 7]` oder ein einzelnes `[24]` für alle Tage.
pub fn parse_week(v: &Value) -> Option<Week> {
    let mut week: Week = [None; 7];
    match v {
        Value::Array(a) if a.first().is_some_and(|x| !x.is_object() && !x.is_array()) => {
            let h = hours(v)?;
            return Some([Some(h); 7]);
        }
        Value::Array(a) => {
            for (i, d) in a.iter().enumerate() {
                if d.is_array() {
                    if i < 7 {
                        week[i] = hours(d);
                    }
                } else if d.is_object() {
                    let di = match pick(d, &["name", "day"]) {
                        Some(n) => n.as_str().and_then(day_index),
                        None => Some(i),
                    };
                    if let Some(di) = di.filter(|&x| x < 7) {
                        week[di] = pick(d, &["data", "hours", "values"]).and_then(hours);
                    }
                }
            }
        }
        Value::Object(o) => {
            for (k, d) in o {
                let di = k
                    .parse::<usize>()
                    .ok()
                    .filter(|_| k.len() == 1)
                    .or_else(|| day_index(k));
                if let Some(di) = di.filter(|&x| x < 7) {
                    week[di] = hours(d);
                }
            }
        }
        _ => return None,
    }
    week.iter().any(Option::is_some).then_some(week)
}

/// Tage seit 1970-01-01 → (Jahr, Monat 1–12, Tag).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}
/// (Jahr, Monat, Tag) → Tage seit 1970-01-01.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
/// Letzter Sonntag eines Monats, 01:00 UTC (Beginn/Ende der Sommerzeit in der EU) als Unix-Sekunden.
fn last_sunday_1utc(y: i64, m: u32) -> i64 {
    let next = if m == 12 {
        days_from_civil(y + 1, 1, 1)
    } else {
        days_from_civil(y, m + 1, 1)
    };
    let last = next - 1;
    let wd = (last + 3).rem_euclid(7); // 0 = Montag
    (last - (wd - 6).rem_euclid(7)) * 86400 + 3600
}
/// Berliner Ortszeit eines Unix-Zeitpunkts: Wochentag (0 = Mo) und Stunde (MEZ/MESZ).
pub fn berlin_slot(epoch: f64) -> (usize, usize) {
    let t = epoch.floor() as i64;
    let (y, ..) = civil(t.div_euclid(86400));
    let summer = t >= last_sunday_1utc(y, 3) && t < last_sunday_1utc(y, 10);
    let local = t + if summer { 7200 } else { 3600 };
    let days = local.div_euclid(86400);
    (
        (days + 3).rem_euclid(7) as usize,
        (local.rem_euclid(86400) / 3600) as usize,
    )
}

/// Zeitstempel: Unix-Sekunden (auch Millisekunden) oder ISO 8601 („2026-10-03T21:15:00Z“, „+02:00“).
fn stamp(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => {
            let n = n.as_f64()?;
            Some(if n > 1e11 { n / 1000. } else { n })
        }
        Value::String(s) => parse_iso(s),
        _ => None,
    }
}
fn parse_iso(s: &str) -> Option<f64> {
    let s = s.trim();
    let num = |a: usize, b: usize| -> Option<i64> { s.get(a..b)?.parse().ok() };
    let (y, mo, d) = (num(0, 4)?, num(5, 7)? as u32, num(8, 10)? as u32);
    let (h, mi) = (num(11, 13).unwrap_or(0), num(14, 16).unwrap_or(0));
    let sec = num(17, 19).unwrap_or(0);
    let mut t = days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + sec;
    let tail = s.get(19..).unwrap_or("");
    let tail = tail.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    if let Some(sign) = tail.chars().next().filter(|c| *c == '+' || *c == '-') {
        let oh: i64 = tail.get(1..3)?.parse().ok()?;
        let om: i64 = tail.get(4..6).and_then(|x| x.parse().ok()).unwrap_or(0);
        let off = oh * 3600 + om * 60;
        t -= if sign == '+' { off } else { -off };
    }
    Some(t as f64)
}

/// Wochenschnitt aller Bars: gostumblr liefert `[{ dow, hours: [{ hour, avg_occupancy }] }]` (0 = Sonntag), sonst
/// eine Form wie `parse_week`.
pub fn parse_global_week(v: &Value) -> Option<Week> {
    if let Some(a) = v.as_array()
        && a.iter().any(|d| d.get("dow").is_some())
    {
        let mut week: Week = [None; 7];
        for d in a {
            let (Some(dow), Some(hs)) = (d["dow"].as_i64(), d["hours"].as_array()) else {
                continue;
            };
            let di = (dow + 6).rem_euclid(7) as usize;
            let mut row = [None; 24];
            for h in hs {
                if let Some(hr) = h["hour"].as_i64().filter(|h| (0..24).contains(h)) {
                    row[hr as usize] = level(&h["avg_occupancy"]);
                }
            }
            if row.iter().any(Option::is_some) {
                week[di] = Some(row);
            }
        }
        return week.iter().any(Option::is_some).then_some(week);
    }
    parse_week(v)
}

/// Bar aus dem Feed.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub name: String,
    pub key: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub current: Option<f64>,
    pub usual: Option<f64>,
    pub at: Option<f64>,
    pub trend: Vec<(f64, f64)>,
    pub week: Option<Week>,
    /// Kartenpunkt (nach `attach_bars`)
    pub x: Option<f64>,
    pub y: Option<f64>,
    /// einem OSM-Lokal zugeordnet
    pub osm: bool,
}

/// Wochenprofil einer gostumblr-Bar: Form des Wochenschnitts, skaliert mit ihrer Beliebtheit, darüber die echten
/// Messungen der letzten 24 h je Stunde.
fn bar_week(b: &Bar, global: Option<&Week>) -> Option<Week> {
    let mut week: Option<Week> = None;
    if let Some(g) = global {
        let mut ratio = 1.;
        let r = b.usual.or(b.current);
        if let (Some(r), Some(at)) = (r, b.at) {
            let (dow, hour) = berlin_slot(at);
            if let Some(avg) = g[dow].and_then(|row| row[hour]).filter(|a| *a > 0.) {
                ratio = (r / avg).clamp(0.3, 2.5);
            }
        }
        let mut w: Week = [None; 7];
        for (d, row) in g.iter().enumerate() {
            w[d] = row.map(|row| row.map(|v| v.map(|v| clamp01(v * ratio))));
        }
        week = Some(w);
    }
    if !b.trend.is_empty() {
        let mut sum: HashMap<usize, (f64, f64)> = HashMap::new();
        for &(t, pct) in &b.trend {
            let Some(v) = level(&Value::from(pct)) else {
                continue;
            };
            if !t.is_finite() {
                continue;
            }
            let (dow, hour) = berlin_slot(t);
            let o = sum.entry(dow * 24 + hour).or_default();
            o.0 += v;
            o.1 += 1.;
        }
        if !sum.is_empty() {
            let w = week.get_or_insert([None; 7]);
            for (k, (a, n)) in sum {
                let row = w[k / 24].get_or_insert([None; 24]);
                row[k % 24] = Some(a / n);
            }
        }
    }
    week
}

/// Gelesener Feed: Zeitstempel, Wochenschnitt aller Bars, Bars.
pub type Feed = (Option<f64>, Option<Week>, Vec<Bar>);
/// Stelle vor einem Lokal (Gehweg) oder `None`.
pub type FrontFn<'a> = dyn FnMut(&mut City, f64, f64) -> Option<(f64, f64)> + 'a;

/// Feed lesen: gostumblr (`{ bars: [{ name, latitude, longitude, occupancy_percent, usual_percent, last_scraped,
/// trend }], weekly }`) und allgemein eine Liste oder `{ bars | venues | data | … }` bzw. GeoJSON.
pub fn parse_bar_feed(json: &Value) -> Result<Feed, String> {
    let list = if json.is_array() {
        Some(json)
    } else {
        pick(
            json,
            &[
                "bars", "venues", "data", "results", "items", "places", "features",
            ],
        )
    };
    let Some(list) = list.and_then(Value::as_array) else {
        return Err("Bar-Feed: keine Liste gefunden (erwartet Array oder { bars: [...] })".into());
    };
    let at = pick(
        json,
        &[
            "at",
            "updated",
            "updated_at",
            "updatedAt",
            "timestamp",
            "time",
        ],
    )
    .and_then(stamp)
    .filter(|_| !json.is_array());
    let global = if json.is_array() {
        None
    } else {
        pick(json, &["week", "weekly"]).and_then(parse_global_week)
    };
    let mut bars = Vec::new();
    for raw in list {
        if !raw.is_object() {
            continue;
        }
        // GeoJSON-Feature: Eigenschaften plus Koordinaten
        let b = match raw.get("properties") {
            Some(p) if p.is_object() => {
                let mut o = p.clone();
                o["coordinates"] = raw["geometry"]["coordinates"].clone();
                o
            }
            _ => raw.clone(),
        };
        let name = pick(&b, &["name", "title", "bar", "venue", "label"])
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let pos = pick(
            &b,
            &["coordinates", "location", "geo", "position", "coords"],
        )
        .unwrap_or(&b);
        let num = |v: Option<&Value>| {
            v.and_then(|x| {
                x.as_f64()
                    .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
            })
        };
        let (mut lat, mut lon) = (
            num(pick(pos, &["lat", "latitude"])),
            num(pick(pos, &["lon", "lng", "long", "longitude"])),
        );
        if let Some(a) = pos.as_array().filter(|a| a.len() >= 2) {
            (lon, lat) = (a[0].as_f64(), a[1].as_f64()); // GeoJSON-Reihenfolge
        }
        let has_pos = matches!((lat, lon), (Some(a), Some(o)) if a.abs() <= 90. && o.abs() <= 180. && (a != 0. || o != 0.));
        if name.is_empty() && !has_pos {
            continue;
        }
        let current = pick(
            &b,
            &[
                "occupancy_percent",
                "current_popularity",
                "currentPopularity",
                "current",
                "live",
                "occupancy",
                "auslastung",
                "load",
                "busy",
                "busyness",
                "level",
                "value",
            ],
        )
        .and_then(level);
        let usual = pick(&b, &["usual_percent", "usual"]).and_then(level);
        let t = pick(
            &b,
            &[
                "last_scraped",
                "at",
                "updated",
                "updated_at",
                "updatedAt",
                "timestamp",
                "time",
            ],
        )
        .and_then(stamp)
        .or(at);
        let trend = b["trend"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|p| {
                        let p = p.as_array().filter(|p| p.len() >= 2)?;
                        Some((stamp(&p[0])?, p[1].as_f64()?))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut bar = Bar {
            key: norm_name(&name),
            name,
            lat: lat.filter(|_| has_pos),
            lon: lon.filter(|_| has_pos),
            current,
            usual,
            at: t,
            trend,
            week: None,
            x: None,
            y: None,
            osm: false,
        };
        bar.week = pick(
            &b,
            &[
                "populartimes",
                "popular_times",
                "popularTimes",
                "week",
                "weekly",
                "profile",
                "woche",
            ],
        )
        .and_then(parse_week)
        .or_else(|| bar_week(&bar, global.as_ref()));
        bars.push(bar);
    }
    if bars.is_empty() {
        return Err("Bar-Feed: keine Bar mit Namen oder Koordinaten".into());
    }
    Ok((at, global, bars))
}

/// Feed an der Stadt: Bars mit Kartenpunkt, Namensindex, Generation (für die Zuordnung zu POIs).
#[derive(Debug, Clone, Default)]
pub struct Bars {
    pub generation: u64,
    pub list: Vec<Bar>,
    pub by_name: HashMap<String, Vec<usize>>,
    /// Zuordnung OSM-Lokal (gerundeter Ort) → Feed-Bar
    matched: HashMap<(i64, i64), Option<usize>>,
}

/// Feed an die Karte hängen (Koordinaten in Karten-px).
pub fn attach_bars(city: &mut City, mut list: Vec<Bar>) -> usize {
    for b in &mut list {
        if let (Some(lat), Some(lon), Some(meta)) = (b.lat, b.lon, city.meta.as_ref()) {
            let p = berlin_map_loader::projection::geo_to_px(meta, lat, lon);
            (b.x, b.y) = (Some(p.x), Some(p.y));
        }
    }
    let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, b) in list.iter().enumerate() {
        if !b.key.is_empty() {
            by_name.entry(b.key.clone()).or_default().push(i);
        }
    }
    let generation = city.bars.as_ref().map_or(0, |b| b.generation) + 1;
    let n = list.len();
    city.bars = Some(Bars {
        generation,
        list,
        by_name,
        matched: HashMap::new(),
    });
    n
}

/// Die Feed-Bar zu einem OSM-Lokal: gleicher Name in der Nähe, sonst eine Feed-Bar direkt daneben.
pub fn feed_bar_for(bars: &mut Bars, q: &Poi) -> Option<usize> {
    let key = (q.x.round() as i64, q.y.round() as i64);
    if let Some(&m) = bars.matched.get(&key) {
        return m;
    }
    let mut best: Option<(usize, f64)> = None;
    for &i in bars.by_name.get(&norm_name(&q.name)).into_iter().flatten() {
        let b = &bars.list[i];
        let d = match (b.x, b.y) {
            (Some(x), Some(y)) => (x - q.x).hypot(y - q.y),
            _ => MATCH - 1.,
        };
        if d < MATCH && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    if best.is_none() {
        for (i, b) in bars.list.iter().enumerate() {
            let (Some(x), Some(y)) = (b.x, b.y) else {
                continue;
            };
            let d = (x - q.x).hypot(y - q.y);
            if d < NEAR && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
    }
    let m = best.map(|b| b.0);
    if let Some(i) = m {
        bars.list[i].osm = true;
    }
    bars.matched.insert(key, m);
    m
}

/// Typischer Verlauf je Lokalart (ohne Feed): Kneipen ab Feierabend, Biergärten am Abend, Clubs ab 23 Uhr und
/// vor allem Freitag-/Samstagnacht.
pub fn typical_level(kind: &str, minutes: f64, day: u32) -> f64 {
    let m = wrap(minutes);
    let night = nightlife(m, day);
    let evening = if (1020. ..1260.).contains(&m) {
        (m - 1020.) / 240.
    } else if m >= 1260. {
        1.
    } else {
        0.
    };
    match kind {
        "nightclub" => {
            if m >= 1380. || m < 360. {
                clamp01((night - 0.3) / 0.7)
            } else {
                0.
            }
        }
        "pub" => (night * 0.8).max(evening * 0.45),
        "biergarten" => {
            if (720. ..1380.).contains(&m) {
                0.25f64.max(evening * 0.7) * if day == 5 || day == 6 { 1. } else { 0.7 }
            } else {
                0.
            }
        }
        _ => (night * 0.9).max(evening * 0.3),
    }
}

/// Wochenprofil an Minute m (linear zwischen den vollen Stunden).
pub fn week_level(week: &Week, minutes: f64, day: u32) -> Option<f64> {
    let m = wrap(minutes);
    let h = (m / 60.).floor() as usize;
    let f = (m % 60.) / 60.;
    let a = week[(day % 7) as usize].and_then(|r| r[h])?;
    let nd = if h == 23 { (day + 1) % 7 } else { day % 7 };
    let b = week[nd as usize].and_then(|r| r[(h + 1) % 24]).unwrap_or(a);
    Some(a + (b - a) * f)
}

/// Wie voll ist das Lokal gerade (0…1)? Mit Wochenprofil gilt es zur Spielzeit; nur eine Live-Auslastung macht die
/// Bar gegenüber ihresgleichen voller oder leerer.
pub fn bar_level(kind: &str, bar: Option<&Bar>, minutes: f64, day: u32, now: Option<f64>) -> f64 {
    let typ = typical_level(kind, minutes, day);
    let Some(bar) = bar else {
        return typ;
    };
    if let Some(wk) = bar.week.as_ref().and_then(|w| week_level(w, minutes, day)) {
        return clamp01(wk);
    }
    let live = bar.current.or(bar.usual);
    let fresh = live.is_some()
        && match (now, bar.at) {
            (Some(n), Some(a)) => n - a < STALE,
            _ => true,
        };
    let base = typ.max(typical_level("bar", minutes, day));
    clamp01(
        base * if fresh {
            0.45 + 1.1 * live.unwrap_or(0.)
        } else {
            1.1
        },
    )
}

/// Hörbares Lokal.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub d: f64,
    pub lvl: f64,
    pub feed: bool,
    pub gain: f64,
    pub club: bool,
}
/// Was man vom Nachtleben an (x, y) hört.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Heard {
    pub crowd: f64,
    pub music: f64,
    /// −1 links … 1 rechts (lauteste Quelle)
    pub pan: f64,
    /// Anteil aus Feed-Bars
    pub feed: f64,
    pub sources: Vec<Source>,
}

/// Nachtleben an (x, y). `front(x, y)` → Stelle vor dem Lokal (sonst der POI selbst); Regen und Schnee treiben die
/// Leute nach drinnen (draußen leiser, die Musik bleibt).
#[allow(clippy::too_many_arguments)]
pub fn nightlife_at(
    city: &mut City,
    front: &mut FrontFn,
    x: f64,
    y: f64,
    minutes: f64,
    day: u32,
    now: Option<f64>,
    rain: f64,
    snow: f64,
) -> Heard {
    let r = HEAR_FEED;
    let pois: Vec<Poi> = city
        .pois
        .query(&Rect::around(x, y, r))
        .into_iter()
        .map(|h| city.pois.get(h).clone())
        .filter(|q| q.cat == "drink")
        .collect();
    let mut out: Vec<Source> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut add = |city: &mut City,
                   qx: f64,
                   qy: f64,
                   name: &str,
                   kind: &str,
                   bar: Option<&Bar>,
                   hear: f64| {
        let lvl = bar_level(kind, bar, minutes, day, now);
        if lvl <= 0.01 {
            return;
        }
        let (fx, fy) = front(city, qx, qy).unwrap_or((qx, qy));
        let d = (fx - x).hypot(fy - y);
        if d >= hear {
            return;
        }
        let fall = (1. - d / hear).powf(1.6);
        out.push(Source {
            name: name.to_string(),
            x: fx,
            y: fy,
            d,
            lvl,
            feed: bar.is_some(),
            gain: lvl * fall * if bar.is_some() { 1.35 } else { 1. },
            club: kind == "nightclub",
        });
    };
    for q in &pois {
        let bi = city.bars.as_mut().and_then(|b| feed_bar_for(b, q));
        let bar = bi.and_then(|i| city.bars.as_ref().map(|b| b.list[i].clone()));
        if let Some(i) = bi {
            seen.insert(i);
        }
        let kind = if q.kind.is_empty() { "bar" } else { &q.kind };
        let hear = if bar.is_some() { HEAR_FEED } else { HEAR };
        add(city, q.x, q.y, &q.name, kind, bar.as_ref(), hear);
    }
    // Feed-Bars, die OSM (noch) nicht kennt: direkt an ihrer Koordinate
    let extra: Vec<Bar> = city
        .bars
        .as_ref()
        .map(|b| {
            b.list
                .iter()
                .enumerate()
                .filter(|(i, b)| !seen.contains(i) && b.x.is_some())
                .filter(|(_, b)| {
                    (b.x.unwrap_or(0.) - x).abs() <= r && (b.y.unwrap_or(0.) - y).abs() <= r
                })
                .map(|(_, b)| b.clone())
                .collect()
        })
        .unwrap_or_default();
    for b in &extra {
        let (bx, by) = (b.x.unwrap_or(0.), b.y.unwrap_or(0.));
        add(city, bx, by, &b.name, "bar", Some(b), HEAR_FEED);
    }
    out.sort_by(|a, b| b.gain.total_cmp(&a.gain));
    let inside = 1. - 0.55 * clamp01(rain) - 0.35 * clamp01(snow);
    let (mut crowd, mut music, mut feed) = (0., 0., 0.);
    for s in &out {
        crowd += s.gain * if s.club { 0.6 } else { 1. };
        music += s.gain * if s.club { 1. } else { 0.35 };
        if s.feed {
            feed += s.gain;
        }
    }
    let pan = out
        .first()
        .map_or(0., |t| ((t.x - x) / 500.).clamp(-1., 1.));
    Heard {
        crowd: clamp01(crowd * 0.8 * inside),
        music: clamp01(music * 0.7),
        pan,
        feed: if crowd > 0. {
            clamp01(feed / crowd)
        } else {
            0.
        },
        sources: out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn typical_levels_follow_the_night() {
        assert_eq!(typical_level("nightclub", 720., 4), 0.);
        assert!(
            typical_level("nightclub", 60., 5) > 0.9,
            "Freitagnacht 1 Uhr voll"
        );
        assert!(typical_level("pub", 1200., 1) > 0.3);
        assert_eq!(typical_level("biergarten", 600., 6), 0.);
        assert!(typical_level("biergarten", 1300., 6) > typical_level("biergarten", 1300., 1));
    }
    #[test]
    fn berlin_time_knows_summer_time() {
        // 2026-07-03 20:00 UTC = Freitag 22 Uhr MESZ
        let t = days_from_civil(2026, 7, 3) as f64 * 86400. + 20. * 3600.;
        assert_eq!(berlin_slot(t), (4, 22));
        // 2026-01-09 20:00 UTC = Freitag 21 Uhr MEZ
        let t = days_from_civil(2026, 1, 9) as f64 * 86400. + 20. * 3600.;
        assert_eq!(berlin_slot(t), (4, 21));
        assert_eq!(civil(days_from_civil(2024, 2, 29)), (2024, 2, 29));
        let summer = days_from_civil(2026, 7, 3) as f64 * 86400. + 20. * 3600.;
        assert_eq!(parse_iso("2026-07-03T20:00:00Z"), Some(summer));
        assert_eq!(
            parse_iso("2026-07-03T22:00:00+02:00"),
            parse_iso("2026-07-03T20:00:00Z")
        );
    }
    #[test]
    fn feeds_are_read_tolerantly() {
        let (_, _, bars) = parse_bar_feed(&json!({
            "bars": [
                { "name": "Bar Raval", "latitude": 52.5, "longitude": 13.42, "occupancy_percent": 80 },
                { "title": "Keine Koordinate" },
                { "type": "Feature", "properties": { "name": "Geo" }, "geometry": { "coordinates": [13.4, 52.51] } },
                {}
            ]
        }))
        .unwrap();
        assert_eq!(bars.len(), 3);
        assert_eq!(bars[0].key, "raval", "Allerweltswörter fallen weg");
        assert_eq!(bars[0].current, Some(0.8));
        assert_eq!(bars[2].lat, Some(52.51));
        assert!(parse_bar_feed(&json!({ "nix": 1 })).is_err());
        let w = parse_week(&json!([{ "name": "Friday", "data": vec![50; 24] }])).unwrap();
        assert_eq!(w[4].unwrap()[3], Some(0.5));
        assert_eq!(week_level(&w, 3. * 60. + 30., 4), Some(0.5));
        let bar = Bar {
            week: Some(w),
            ..bars[0].clone()
        };
        assert_eq!(bar_level("bar", Some(&bar), 200., 4, None), 0.5);
        assert!(bar_level("bar", Some(&bars[0]), 1320., 4, None) > typical_level("bar", 1320., 4));
    }
}
