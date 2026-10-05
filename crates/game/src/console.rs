//! Befehlszeile im Spiel (Port von `console.js`, ohne Fenster): Enter öffnet sie, die Welt steht solange still.
//! Befehle setzen Uhrzeit, Wochentag, Wetter, Dichte, teleportieren oder schummeln. Autovervollständigung für
//! Befehlsnamen, feste Werte je Argument und Orte (Straßen, Bahnhöfe, Ortsteile, Kieze, Bezirke); Tab/→ übernimmt,
//! ↑/↓ wählt (ohne Vorschläge: Verlauf), Enter führt aus, Esc leert bzw. schließt. Wie eine Befehlspalette: Tippfehler
//! werden verziehen, ohne Befehlswort versteht die Zeile Uhrzeit („22:30“, „nacht“), Wetter („regen“) und Orte;
//! Enter übernimmt zuerst einen abweichenden Vorschlag, das nächste führt aus. Nach Erfolg schließt sie (Umschalt
//! lässt sie offen), bei Fehlern bleibt sie mit Hinweis offen.
use crate::bigmap::Labels;
use berlin_sim::world::World;

pub const MAX_SUGGESTIONS: usize = 8;
pub const MAX_LOG: usize = 8;
/// s, die eine Meldung sichtbar bleibt
pub const LOG_TIME: f64 = 8.;
pub const HISTORY: usize = 50;

/// Suchform: klein, ohne Akzente, ß → ss.
pub fn norm(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        match c {
            'ß' => out.push_str("ss"),
            'ä' | 'á' | 'à' | 'â' | 'ã' | 'å' => out.push('a'),
            'ö' | 'ó' | 'ò' | 'ô' | 'õ' => out.push('o'),
            'ü' | 'ú' | 'ù' | 'û' => out.push('u'),
            'é' | 'è' | 'ê' | 'ë' => out.push('e'),
            'í' | 'ì' | 'î' | 'ï' => out.push('i'),
            'ç' => out.push('c'),
            'ñ' => out.push('n'),
            _ => out.push(c),
        }
    }
    out
}

/// Wetter-Namen (deutsch, wie `--wetter`) ↔ interne Art.
pub const WX_NAMES: [(&str, &str); 11] = [
    ("sonnig", "clear"),
    ("wolkig", "cloudy"),
    ("bedeckt", "overcast"),
    ("regen", "rain"),
    ("starkregen", "heavyrain"),
    ("sturm", "storm"),
    ("gewitter", "thunder"),
    ("nebel", "fog"),
    ("dichternebel", "densefog"),
    ("schnee", "snow"),
    ("schneesturm", "heavysnow"),
];
/// angenommen, nicht vorgeschlagen
const WX_ALIAS: [(&str, &str); 6] = [
    ("klar", "clear"),
    ("sonne", "clear"),
    ("heiter", "clear"),
    ("bewoelkt", "cloudy"),
    ("gewitterregen", "thunder"),
    ("dunst", "fog"),
];
fn wx_kind(k: &str) -> Option<&'static str> {
    WX_NAMES
        .iter()
        .chain(&WX_ALIAS)
        .find(|(n, _)| *n == k)
        .map(|(_, v)| *v)
        .or_else(|| {
            berlin_sim::weather::KINDS
                .iter()
                .find(|x| **x == k)
                .copied()
        })
}
const TIME_WORDS: [(&str, &str); 7] = [
    ("morgen", "07:30"),
    ("mittag", "12:00"),
    ("nachmittag", "15:30"),
    ("abend", "19:30"),
    ("daemmerung", "20:45"),
    ("nacht", "23:30"),
    ("mitternacht", "00:00"),
];
fn time_word(k: &str) -> Option<&'static str> {
    TIME_WORDS.iter().find(|(w, _)| *w == k).map(|(_, t)| *t)
}
pub const WEEKDAYS: [&str; 7] = [
    "Montag",
    "Dienstag",
    "Mittwoch",
    "Donnerstag",
    "Freitag",
    "Samstag",
    "Sonntag",
];

/// Uhrzeit großzügig lesen: 21:30, 21.30, 2130, 21, 21h, 21uhr → Minuten.
pub fn clock_arg(v: &str) -> Option<f64> {
    let mut t = v.trim().to_lowercase();
    for suf in ["uhr", "h"] {
        if let Some(x) = t.strip_suffix(suf) {
            t = x.trim_end().to_string();
            break;
        }
    }
    let (h, m) = if let Some((a, b)) = t.split_once([':', '.']) {
        if b.len() != 2 {
            return None;
        }
        (a.to_string(), b.to_string())
    } else if t.len() == 4 {
        (t[..2].to_string(), t[2..].to_string())
    } else {
        (t.clone(), "00".to_string())
    };
    if h.is_empty() || h.len() > 2 || !h.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !m.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then(|| (h * 60 + m) as f64)
}
pub fn format_clock(m: f64) -> String {
    let m = (m.rem_euclid(1440.)).floor() as u32;
    format!("{:02}:{:02}", m / 60, m % 60)
}
fn weekday_arg(v: &str) -> Option<u32> {
    let k = norm(v);
    let k = k.trim().trim_end_matches('.');
    if let Ok(n) = k.parse::<u32>() {
        return (1..=7).contains(&n).then(|| n - 1);
    }
    if k == "sonnabend" {
        return Some(5);
    }
    WEEKDAYS
        .iter()
        .position(|d| {
            let d = norm(d);
            d == k || d[..2] == *k
        })
        .map(|i| i as u32)
}

/// Ort für `tp`.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub name: String,
    pub kind: &'static str,
    pub x: f64,
    pub y: f64,
    pub rank: u8,
    pub key: String,
}
/// Orte aus dem Stadtplan: Bezirke, Ortsteile, Bahnhöfe, Kieze, Straßen (je Name und Art einmal).
pub fn place_index(l: &Labels) -> Vec<Place> {
    let mut out: Vec<Place> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |name: &str, kind: &'static str, x: f32, y: f32, rank: u8| {
        if name.is_empty() || !seen.insert((norm(name), kind)) {
            return;
        }
        out.push(Place {
            name: name.to_string(),
            kind,
            x: x as f64,
            y: y as f64,
            rank,
            key: norm(name),
        });
    };
    for b in &l.bezirke {
        push(&b.text, "Bezirk", b.at.x, b.at.y, 0);
    }
    for b in &l.ortsteile {
        push(&b.text, "Ortsteil", b.at.x, b.at.y, 1);
    }
    for s in &l.stations {
        let kind = match s.cat.as_str() {
            "ubahn" => "U-Bahnhof",
            "sbahn" => "S-Bahnhof",
            _ => "Bahnhof",
        };
        push(&s.name, kind, s.at.x, s.at.y, 2);
    }
    for k in &l.kieze {
        push(&k.text, "Kiez", k.at.x, k.at.y, 3);
    }
    for s in &l.streets {
        push(&s.text, "Straße", s.at.x, s.at.y, 4);
    }
    out
}

/// Tippfehler-Abstand (Levenshtein, früh abgebrochen ab max + 1).
pub fn edit_distance(a: &str, b: &str, max: usize) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len().abs_diff(b.len()) > max {
        return max + 1;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        let mut best = i;
        for j in 1..=b.len() {
            cur[j] = (prev[j] + 1)
                .min(cur[j - 1] + 1)
                .min(prev[j - 1] + usize::from(a[i - 1] != b[j - 1]));
            best = best.min(cur[j]);
        }
        if best > max {
            return max + 1;
        }
        prev = cur;
    }
    prev[b.len()]
}
/// Das Getippte ähnelt dem Anfang des Namens oder eines Worts darin (ab 4 Zeichen 1 Fehler, ab 7: 2).
fn fuzzy_hit(k: &str, q: &str) -> bool {
    let ql = q.chars().count();
    if ql < 4 {
        return false;
    }
    let max = if ql >= 7 { 2 } else { 1 };
    std::iter::once(k)
        .chain(k.split([' ', '-']).filter(|w| !w.is_empty()))
        .any(|w| {
            let wc: Vec<char> = w.chars().collect();
            [ql - 1, ql, ql + 1].into_iter().any(|l| {
                l > 0 && l <= wc.len() && {
                    let pre: String = wc[..l].iter().collect();
                    edit_distance(q, &pre, max) <= max
                }
            })
        })
}

/// Treffer bewerten: Anfang < Wortanfang < irgendwo < Tippfehler; dann Rang, dann kürzer. Indizes in `keys`.
pub fn rank_matches(keys: &[(String, u8)], query: &str) -> Vec<usize> {
    let q = norm(query);
    let q = q.trim();
    let mut out: Vec<(f64, usize)> = Vec::new();
    for (i, (k, rank)) in keys.iter().enumerate() {
        let s = if q.is_empty() || k.starts_with(q) {
            0.
        } else if k.contains(&format!(" {q}")) || k.contains(&format!("-{q}")) {
            1.
        } else if k.contains(q) {
            2.
        } else if fuzzy_hit(k, q) {
            3.
        } else {
            continue;
        };
        let len = if q.is_empty() {
            0.
        } else {
            (k.chars().count() as f64 / 6.).min(9.)
        };
        out.push((s * 100. + *rank as f64 * 10. + len, i));
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    out.into_iter().map(|o| o.1).collect()
}
fn rank_places<'a>(places: &'a [Place], q: &str) -> Vec<&'a Place> {
    let keys: Vec<(String, u8)> = places.iter().map(|p| (p.key.clone(), p.rank)).collect();
    rank_matches(&keys, q)
        .into_iter()
        .map(|i| &places[i])
        .collect()
}

fn num(v: &str) -> Option<f64> {
    v.replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
}
fn on_off(v: Option<&str>, cur: bool) -> Option<bool> {
    match v {
        None => Some(!cur),
        Some("an") => Some(true),
        Some("aus") => Some(false),
        _ => None,
    }
}

/// Was ein Befehl außerhalb der Welt auslöst (das Spiel führt es aus).
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Teleport {
        x: f64,
        y: f64,
        name: String,
    },
    /// Wegpunkt mit Route setzen (`None` = entfernen)
    Waypoint(Option<(f64, f64, String)>),
    Stats,
    Cheat(&'static str),
    Money(f64),
    /// Bar-Feed: Datei setzen, `neu` laden, `aus`
    Bars(Option<String>),
}

/// Anzeige-Schalter der Befehlszeile (`fps`, `ebenen`, `silhouetten`); gehören dem Spiel, nicht der Welt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Debug {
    pub fps: bool,
    pub levels: bool,
    pub silhouettes: bool,
}
impl Default for Debug {
    fn default() -> Self {
        Self {
            fps: false,
            levels: false,
            silhouettes: true,
        }
    }
}

/// Was ein Befehl braucht.
pub struct Ctx<'a> {
    pub world: &'a mut World,
    pub places: &'a [Place],
    pub actions: Vec<Action>,
    pub debug: Debug,
}

#[derive(Debug, Clone, Copy)]
pub enum Values {
    None,
    Fixed(&'static [(&'static str, &'static str)]),
    Commands,
    Weather,
    TimeWords,
    Weekdays,
    Vehicles,
}
#[derive(Debug, Clone, Copy)]
pub struct Arg {
    pub name: &'static str,
    pub optional: bool,
    pub rest: bool,
    pub places: bool,
    pub values: Values,
}
const fn arg(name: &'static str, optional: bool, values: Values) -> Arg {
    Arg {
        name,
        optional,
        rest: false,
        places: false,
        values,
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Command {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub help: &'static str,
    pub cheat: bool,
    pub args: &'static [Arg],
}
const ONOFF: Values = Values::Fixed(&[("an", ""), ("aus", "")]);
const ZERO_ONE: Values = Values::Fixed(&[("0", ""), ("0.5", ""), ("1", "")]);
const FACTOR: Values = Values::Fixed(&[
    ("0", ""),
    ("0.5", ""),
    ("1", "normal"),
    ("1.5", ""),
    ("2", ""),
    ("3", ""),
]);

pub const COMMANDS: &[Command] = &[
    Command {
        name: "hilfe",
        aliases: &["help", "?"],
        help: "Befehle anzeigen",
        cheat: false,
        args: &[arg("befehl", true, Values::Commands)],
    },
    Command {
        name: "zeit",
        aliases: &["uhr", "time"],
        help: "Uhrzeit setzen",
        cheat: false,
        args: &[arg("HH:MM", false, Values::TimeWords)],
    },
    Command {
        name: "tag",
        aliases: &["wochentag", "day"],
        help: "Wochentag zeigen oder wählen (Mo–So, 1 = Montag, 7 = Sonntag)",
        cheat: false,
        args: &[arg("wochentag", true, Values::Weekdays)],
    },
    Command {
        name: "wetter",
        aliases: &["weather"],
        help: "Enter öffnet die Wettertafel; Wert festlegen (auto = natürliches Wetter)",
        cheat: false,
        args: &[arg("wetter", false, Values::Weather)],
    },
    Command {
        name: "schnee",
        aliases: &["snow"],
        help: "Schneedecke 0–1",
        cheat: false,
        args: &[arg(
            "0–1",
            false,
            Values::Fixed(&[
                ("0", ""),
                ("0.25", ""),
                ("0.5", ""),
                ("0.75", ""),
                ("1", ""),
            ]),
        )],
    },
    Command {
        name: "nass",
        aliases: &["wet"],
        help: "Nässe der Straßen 0–1",
        cheat: false,
        args: &[arg("0–1", false, ZERO_ONE)],
    },
    Command {
        name: "glaette",
        aliases: &["ice", "glätte"],
        help: "Glätte der Straßen 0–1 (taut über 0 °C, s. temp)",
        cheat: false,
        args: &[arg("0–1", false, ZERO_ONE)],
    },
    Command {
        name: "temp",
        aliases: &["temperatur"],
        help: "Temperatur zeigen; temp -5 erzwingt sie, temp auto gibt sie frei",
        cheat: false,
        args: &[arg(
            "°C",
            true,
            Values::Fixed(&[("auto", ""), ("-5", ""), ("0", ""), ("5", ""), ("20", "")]),
        )],
    },
    Command {
        name: "tempo",
        aliases: &["zeitraffer"],
        help: "Tempo der Spieluhr (1 = normal, 0 = Uhr steht)",
        cheat: false,
        args: &[arg(
            "faktor",
            false,
            Values::Fixed(&[
                ("0", ""),
                ("0.5", ""),
                ("1", "normal"),
                ("2", ""),
                ("5", ""),
                ("10", ""),
                ("30", ""),
            ]),
        )],
    },
    Command {
        name: "verkehr",
        aliases: &["traffic"],
        help: "Verkehrsdichte (1 = normal)",
        cheat: false,
        args: &[arg("faktor", false, FACTOR)],
    },
    Command {
        name: "passanten",
        aliases: &["peds"],
        help: "Fußgängerdichte (1 = normal)",
        cheat: false,
        args: &[arg("faktor", false, FACTOR)],
    },
    Command {
        name: "ziel",
        aliases: &["wegpunkt", "route", "navi"],
        help: "Wegpunkt mit Route zu einem Ort setzen (ziel aus = entfernen)",
        cheat: false,
        args: &[Arg {
            name: "ort",
            optional: false,
            rest: true,
            places: true,
            values: Values::None,
        }],
    },
    Command {
        name: "tp",
        aliases: &["teleport", "gehe"],
        help: "Teleport zu Straße, Bahnhof, Ortsteil, Kiez, Bezirk",
        cheat: false,
        args: &[Arg {
            name: "ort",
            optional: false,
            rest: true,
            places: true,
            values: Values::None,
        }],
    },
    Command {
        name: "geld",
        aliases: &["money"],
        help: "Geld setzen (+n: dazugeben)",
        cheat: true,
        args: &[arg(
            "betrag",
            false,
            Values::Fixed(&[("+1000", ""), ("+10000", ""), ("0", ""), ("100000", "")]),
        )],
    },
    Command {
        name: "leben",
        aliases: &["heal"],
        help: "volle Gesundheit und eigenes Auto reparieren",
        cheat: true,
        args: &[],
    },
    Command {
        name: "munition",
        aliases: &["ammo"],
        help: "alle Magazine voll",
        cheat: true,
        args: &[],
    },
    Command {
        name: "begrenzer",
        aliases: &["limiter", "tempobegrenzer"],
        help: "Tempobegrenzer schwerer Lkw (89 km/h) an/aus",
        cheat: false,
        args: &[arg("an|aus", true, ONOFF)],
    },
    Command {
        name: "esp",
        aliases: &["asr", "fahrhilfen"],
        help: "ASR/ESP im Auto an/aus (aus: Heckantrieb driftet)",
        cheat: false,
        args: &[arg("an|aus", true, ONOFF)],
    },
    Command {
        name: "gott",
        aliases: &["god"],
        help: "unverwundbar an/aus",
        cheat: true,
        args: &[arg("an|aus", true, ONOFF)],
    },
    Command {
        name: "auto",
        aliases: &["car", "fahrzeug", "spawn", "spawnen"],
        help: "Auto oder Fahrzeug neben dir spawnen (Modell oder Art)",
        cheat: true,
        args: &[arg("art", true, Values::Vehicles)],
    },
    Command {
        name: "reparieren",
        aliases: &["repair"],
        help: "eigenes Auto reparieren",
        cheat: true,
        args: &[],
    },
    Command {
        name: "fps",
        aliases: &[],
        help: "Bildrate und Zeichenzeit anzeigen",
        cheat: false,
        args: &[arg("an|aus", true, ONOFF)],
    },
    Command {
        name: "ebenen",
        aliases: &["levels"],
        help: "Ebenen und Portale anzeigen",
        cheat: false,
        args: &[arg("an|aus", true, ONOFF)],
    },
    Command {
        name: "silhouetten",
        aliases: &[],
        help: "Umrisse verdeckter Figuren an/aus",
        cheat: false,
        args: &[arg("an|aus", true, ONOFF)],
    },
    Command {
        name: "bars",
        aliases: &["nachtleben"],
        help: "Bar-Auslastung (Datei, live aus dem Netz, URL, neu laden, aus)",
        cheat: false,
        args: &[arg(
            "Datei|live|URL|neu|aus",
            true,
            Values::Fixed(&[("neu", "Feed neu laden"), ("aus", "nur OSM-Lokale")]),
        )],
    },
    Command {
        name: "stats",
        aliases: &["statistik"],
        help: "Statistik anzeigen",
        cheat: false,
        args: &[],
    },
];

const KIND_LABEL: [(&str, &str); 11] = [
    ("car", "Pkw"),
    ("truck", "Lkw"),
    ("delivery", "Lieferwagen"),
    ("garbage", "Müllauto"),
    ("police", "Polizei"),
    ("ambulance", "Rettungswagen"),
    ("bus", "Bus"),
    ("bicycle", "Fahrrad"),
    ("escooter", "E-Roller"),
    ("motorcycle", "Motorrad"),
    ("scooter", "Roller"),
];

pub fn find_command(name: &str) -> Option<&'static Command> {
    let n = norm(name);
    COMMANDS
        .iter()
        .find(|c| c.name == n || c.aliases.iter().any(|a| norm(a) == n))
}
pub fn usage(c: &Command) -> String {
    let mut s = c.name.to_string();
    for a in c.args {
        s += &if a.optional {
            format!(" [{}]", a.name)
        } else {
            format!(" <{}>", a.name)
        };
    }
    s
}

/// Wort der Zeile mit Lage (Anführungszeichen halten Leerzeichen zusammen).
#[derive(Debug, Clone, PartialEq)]
pub struct Tok {
    pub t: String,
    pub start: usize,
    pub end: usize,
}
pub fn tokenize(text: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let b: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < b.len() {
        let (pos, c) = b[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '"' {
            let mut j = i + 1;
            while j < b.len() && b[j].1 != '"' {
                j += 1;
            }
            let end = if j < b.len() { b[j].0 + 1 } else { text.len() };
            let inner_end = if j < b.len() { b[j].0 } else { text.len() };
            out.push(Tok {
                t: text[pos + 1..inner_end].to_string(),
                start: pos,
                end,
            });
            i = j + 1;
        } else {
            let mut j = i;
            while j < b.len() && !b[j].1.is_whitespace() {
                j += 1;
            }
            let end = if j < b.len() { b[j].0 } else { text.len() };
            out.push(Tok {
                t: text[pos..end].to_string(),
                start: pos,
                end,
            });
            i = j;
        }
    }
    out
}

/// Ohne Befehlswort: was die Zeile meint – Uhrzeit, Wochentag, Wetter oder Ort.
pub fn smart_line(line: &str, places: &[Place]) -> Option<String> {
    let t = line.trim();
    let k = norm(t);
    if t.is_empty() {
        return None;
    }
    if clock_arg(t).is_some() || time_word(&k).is_some() {
        return Some(format!("zeit {t}"));
    }
    if weekday_arg(t).is_some() {
        return Some(format!("tag {t}"));
    }
    if wx_kind(&k).is_some() && !berlin_sim::weather::KINDS.contains(&k.as_str()) {
        return Some(format!("wetter {t}"));
    }
    if k.chars().count() >= 3
        && let Some(hit) = rank_places(places, t).first()
        && hit.key.contains(&k)
    {
        return Some(format!("tp {}", hit.name)); // nur sichere Treffer (kein Tippfehler-Raten)
    }
    None
}

/// Vorschlag: Beschriftung, Hinweis, was eingesetzt wird, ganze Zeile?
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub label: String,
    pub hint: String,
    pub insert: String,
    pub full: bool,
}
fn item(label: &str, hint: &str) -> Item {
    Item {
        label: label.into(),
        hint: hint.into(),
        insert: label.into(),
        full: false,
    }
}

fn smart_items(query: &str, places: &[Place], n: usize) -> Vec<Item> {
    let q = norm(query);
    let q = q.trim();
    let mut out = Vec::new();
    if q.is_empty() {
        return out;
    }
    let full = |label: String, hint: String, insert: String| Item {
        label,
        hint,
        insert,
        full: true,
    };
    if let Some(m) = clock_arg(query).or_else(|| time_word(q).and_then(clock_arg)) {
        out.push(full(
            format!("Uhrzeit {}", format_clock(m)),
            "zeit".into(),
            format!("zeit {}", query.trim()),
        ));
    }
    for d in WEEKDAYS {
        if norm(d).starts_with(q) {
            out.push(full(d.into(), "Wochentag".into(), format!("tag {d}")));
        }
    }
    for (w, kind) in WX_NAMES {
        if w.starts_with(q) || (q.chars().count() >= 4 && fuzzy_hit(w, q)) {
            out.push(full(
                w.into(),
                format!("Wetter: {}", berlin_sim::weather::label(kind)),
                format!("wetter {w}"),
            ));
        }
    }
    if q.chars().count() >= 3 {
        for p in rank_places(places, query).into_iter().take(4) {
            out.push(full(
                p.name.clone(),
                format!("tp · {}", p.kind),
                format!("tp {}", p.name),
            ));
        }
    }
    out.truncate(n);
    out
}

fn values_of(v: Values) -> Vec<Item> {
    match v {
        Values::None => Vec::new(),
        Values::Fixed(list) => list.iter().map(|(l, h)| item(l, h)).collect(),
        Values::Commands => COMMANDS.iter().map(|c| item(c.name, c.help)).collect(),
        Values::Weather => std::iter::once(item("auto", "natürlich"))
            .chain(
                WX_NAMES
                    .iter()
                    .map(|(n, k)| item(n, berlin_sim::weather::label(k))),
            )
            .collect(),
        Values::TimeWords => TIME_WORDS
            .iter()
            .map(|(w, t)| item(w, t))
            .chain(
                [
                    "06:00", "09:00", "12:00", "18:00", "21:00", "00:00", "03:00",
                ]
                .iter()
                .map(|t| item(t, "")),
            )
            .collect(),
        Values::Weekdays => WEEKDAYS
            .iter()
            .enumerate()
            .map(|(i, d)| item(d, &format!("{} · {}", &d[..2], i + 1)))
            .collect(),
        Values::Vehicles => berlin_sim::carmodels::CAR_MODELS
            .iter()
            .map(|(m, _)| item(m, berlin_sim::carmodels::spec_of(m).label))
            .chain(KIND_LABEL.iter().map(|(k, l)| item(k, l)))
            .chain(
                berlin_sim::vehdata::shared()
                    .vehicles
                    .iter()
                    .filter(|d| berlin_sim::carmodels::spec(&d.id).is_none())
                    .map(|d| item(&d.id, &d.name)),
            )
            .collect(),
    }
}

/// Vorschläge für die Stelle am Zeilenende.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Suggest {
    pub items: Vec<Item>,
    /// Byte-Lage, ab der der Vorschlag einsetzt
    pub from: usize,
    pub query: String,
    /// Rest des ersten Vorschlags, grau hinter dem Getippten
    pub ghost: String,
    pub cmd: Option<&'static str>,
    pub argi: usize,
    pub rest: bool,
    pub help: String,
}

pub fn suggest(text: &str, places: &[Place]) -> Suggest {
    let toks = tokenize(text);
    let trailing = text.is_empty() || text.ends_with(char::is_whitespace);
    let argi = if trailing { toks.len() } else { toks.len() - 1 };
    let (mut from, mut query) = if trailing {
        (text.len(), String::new())
    } else {
        let t = toks.last().expect("Wort");
        (t.start, t.t.clone())
    };
    let cmd = if argi > 0 {
        find_command(&toks[0].t)
    } else {
        None
    };
    let mut items: Vec<Item> = Vec::new();
    let mut rest = false;
    if argi == 0 {
        let mut all: Vec<(Item, String, u8)> = Vec::new();
        for c in COMMANDS {
            all.push((item(c.name, c.help), c.name.into(), 0));
            for a in c.aliases {
                all.push((item(a, &format!("→ {}", c.name)), norm(a), 1));
            }
        }
        let keys: Vec<(String, u8)> = all.iter().map(|a| (a.1.clone(), a.2)).collect();
        items = rank_matches(&keys, &query)
            .into_iter()
            .map(|i| all[i].0.clone())
            .collect();
        // Alias nur zeigen, wenn nichts Eigenes passt
        let own: Vec<Item> = items
            .iter()
            .filter(|i| !i.hint.starts_with('→'))
            .cloned()
            .collect();
        if !own.is_empty() {
            items = own;
        }
        if query.chars().count() >= 2 {
            let head: Vec<Item> = items.iter().take(3).cloned().collect();
            let tail: Vec<Item> = items.iter().skip(3).cloned().collect();
            items = head
                .into_iter()
                .chain(smart_items(&query, places, MAX_SUGGESTIONS))
                .chain(tail)
                .collect();
        }
    } else if let Some(c) = cmd {
        let spec = c
            .args
            .get(argi - 1)
            .or_else(|| c.args.iter().find(|a| a.rest));
        if let Some(spec) = spec {
            if spec.rest {
                rest = true;
                match toks.get(1) {
                    Some(first) => {
                        from = first.start;
                        query = text[first.start..].to_string();
                    }
                    None => {
                        from = text.len();
                        query.clear();
                    }
                }
            }
            if spec.places {
                items = rank_places(places, &query)
                    .into_iter()
                    .map(|p| item(&p.name, p.kind))
                    .collect();
            } else {
                let vals = values_of(spec.values);
                let keys: Vec<(String, u8)> = vals.iter().map(|v| (norm(&v.label), 0)).collect();
                items = rank_matches(&keys, &query)
                    .into_iter()
                    .map(|i| vals[i].clone())
                    .collect();
            }
        }
    }
    items.truncate(MAX_SUGGESTIONS);
    let ghost = match items.first() {
        Some(f) if !query.is_empty() && !f.full && norm(&f.insert).starts_with(&norm(&query)) => {
            f.insert.chars().skip(query.chars().count()).collect()
        }
        _ => String::new(),
    };
    let hc = cmd.or_else(|| {
        (argi == 0)
            .then(|| items.first().filter(|f| !f.full))
            .flatten()
            .and_then(|f| find_command(&f.insert))
    });
    let help = match (hc, items.first()) {
        (Some(c), _) => format!("{} – {}", usage(c), c.help),
        (None, Some(f)) if argi == 0 && f.full => format!("Enter: {}", f.insert),
        _ => String::new(),
    };
    Suggest {
        items,
        from,
        query,
        ghost,
        cmd: cmd.map(|c| c.name),
        argi,
        rest,
        help,
    }
}

/// Ergebnis eines Befehls.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub ok: bool,
    pub msg: String,
}
fn ok(m: impl Into<String>) -> Outcome {
    Outcome {
        ok: true,
        msg: m.into(),
    }
}
fn err(m: impl Into<String>) -> Outcome {
    Outcome {
        ok: false,
        msg: m.into(),
    }
}

fn run(c: &Command, ctx: &mut Ctx, args: &[String]) -> Outcome {
    let a0 = args.first().map(String::as_str);
    let w = &mut *ctx.world;
    let unit = |v: Option<&str>, name: &str| -> Result<f64, Outcome> {
        v.and_then(num)
            .filter(|n| (0. ..=1.).contains(n))
            .ok_or_else(|| err(format!("{name} 0 bis 1")))
    };
    match c.name {
        "hilfe" => match a0 {
            Some(x) => match find_command(x) {
                Some(cmd) => ok(format!("{} – {}", usage(cmd), cmd.help)),
                None => err(format!("Unbekannter Befehl „{x}“")),
            },
            None => ok(COMMANDS
                .iter()
                .map(|c| c.name)
                .collect::<Vec<_>>()
                .join(" · ")),
        },
        "zeit" => {
            let v = a0.unwrap_or("");
            let m = time_word(&norm(v)).map_or_else(|| clock_arg(v), clock_arg);
            match m {
                Some(m) => {
                    w.clock = m;
                    ok(format!("Uhrzeit {}", format_clock(m)))
                }
                None => err("Zeit als HH:MM, z. B. zeit 21:30 – oder morgen, mittag, abend, nacht"),
            }
        }
        "tag" => match a0 {
            None => ok(format!("Wochentag: {}", WEEKDAYS[w.day as usize % 7])),
            Some(v) => match weekday_arg(v) {
                Some(d) => {
                    w.day = d;
                    ok(format!("Wochentag: {}", WEEKDAYS[d as usize]))
                }
                None => err("tag Montag bis Sonntag, z. B. tag Freitag (auch Fr oder 5)"),
            },
        },
        "wetter" => {
            let k = norm(a0.unwrap_or(""));
            if k == "auto" {
                w.force_weather = None;
                return ok("Wetter wieder natürlich");
            }
            match wx_kind(&k) {
                Some(kind) => {
                    w.force_weather = Some(kind);
                    if matches!(kind, "rain" | "heavyrain" | "storm" | "thunder") {
                        w.weather.wet = w.weather.wet.max(0.6);
                    }
                    ok(format!("Wetter: {}", berlin_sim::weather::label(kind)))
                }
                None => err(format!(
                    "Wetter: {} oder auto",
                    WX_NAMES.map(|x| x.0).join(", ")
                )),
            }
        }
        "schnee" => match unit(a0, "schnee") {
            Ok(n) => {
                w.weather.snow = n;
                ok(format!("Schneedecke {} %", (n * 100.).round()))
            }
            Err(e) => e,
        },
        "nass" => match unit(a0, "nass") {
            Ok(n) => {
                w.weather.wet = n;
                ok(format!("Nässe {} %", (n * 100.).round()))
            }
            Err(e) => e,
        },
        "glaette" => match unit(a0, "glaette") {
            Ok(n) => {
                w.weather.ice = n;
                ok(format!("Glätte {} %", (n * 100.).round()))
            }
            Err(e) => e,
        },
        "temp" => match a0 {
            None => {
                let t = w.force_temp.unwrap_or(w.temp);
                ok(format!(
                    "{} °C{}",
                    format!("{t:.1}").replace('.', ","),
                    if w.force_temp.is_some() {
                        " (erzwungen)"
                    } else {
                        ""
                    }
                ))
            }
            Some("auto") => {
                w.force_temp = None;
                ok("Temperatur wieder natürlich")
            }
            Some(v) => match num(v).filter(|n| (-30. ..=40.).contains(n)) {
                Some(n) => {
                    w.force_temp = Some(n);
                    w.temp = n;
                    ok(format!("Temperatur {n} °C"))
                }
                None => err("temp -30 bis 40 oder auto"),
            },
        },
        "tempo" => match a0.and_then(num).filter(|n| (0. ..=120.).contains(n)) {
            Some(n) => {
                w.clock_rate = n;
                ok(if n == 1. {
                    "Spieluhr normal".into()
                } else {
                    format!("Spieluhr × {n}")
                })
            }
            None => err("tempo 0 bis 120"),
        },
        "verkehr" | "passanten" => match a0.and_then(num).filter(|n| (0. ..=3.).contains(n)) {
            Some(n) => {
                if c.name == "verkehr" {
                    w.traffic_scale = n;
                    ok(format!("Verkehr × {n}"))
                } else {
                    w.ped_scale = n;
                    ok(format!("Passanten × {n}"))
                }
            }
            None => err(format!("{} 0 bis 3", c.name)),
        },
        "ziel" => {
            let Some(q) = a0 else {
                return err("ziel <Ort> oder ziel aus, z. B. ziel Hermannplatz");
            };
            if matches!(q, "aus" | "weg" | "off") {
                ctx.actions.push(Action::Waypoint(None));
                return ok("Wegpunkt entfernt");
            }
            let Some(hit) = rank_places(ctx.places, q).first().copied() else {
                return err(format!("Kein Ort „{q}“"));
            };
            if !ctx.world.city.inside_border(hit.x, hit.y) {
                return err(format!("{} liegt außerhalb", hit.name));
            }
            let name = hit.name.clone();
            ctx.actions
                .push(Action::Waypoint(Some((hit.x, hit.y, name.clone()))));
            ok(format!("Ziel: {name}"))
        }
        "tp" => {
            let Some(q) = a0 else {
                return err("tp <Ort>, z. B. tp Kottbusser Tor");
            };
            let Some(hit) = rank_places(ctx.places, q).first().copied() else {
                return err(format!("Kein Ort „{q}“"));
            };
            if !ctx.world.city.inside_border(hit.x, hit.y) {
                return err(format!("{} liegt außerhalb", hit.name));
            }
            let (x, y, name, kind) = (hit.x, hit.y, hit.name.clone(), hit.kind);
            ctx.actions.push(Action::Teleport {
                x,
                y,
                name: name.clone(),
            });
            ok(format!("Teleport: {name} ({kind})"))
        }
        "geld" => {
            let v = a0.unwrap_or("");
            let plus = v.starts_with('+');
            match num(v.trim_start_matches('+')).filter(|n| (0. ..=1e9).contains(n)) {
                Some(n) => {
                    w.money = (if plus { w.money + n } else { n }).round();
                    let m = w.money;
                    ctx.actions.push(Action::Money(m));
                    ok(format!("Geld: {} €", group(m as i64)))
                }
                None => err("geld <betrag> oder geld +<betrag>"),
            }
        }
        "leben" => {
            let p = &mut w.player.combat;
            p.hp = berlin_sim::combat::PLAYER_HP;
            p.dead = false;
            w.player.stun = 0.;
            if repair(w) {
                ok("Gesundheit voll, Auto repariert")
            } else {
                ok("Gesundheit voll")
            }
        }
        "munition" => {
            for (i, wp) in berlin_sim::combat::WEAPONS.iter().enumerate() {
                w.player.combat.mag[i] = wp.mag;
            }
            w.player.combat.reload_t = 0.;
            ok("Magazine voll")
        }
        "esp" => match on_off(a0, w.esp) {
            Some(on) => {
                w.esp = on;
                ok(format!("ASR/ESP {}", if on { "an" } else { "aus" }))
            }
            None => err("esp an|aus"),
        },
        "begrenzer" => match on_off(a0, w.truck_limiter) {
            Some(_) if !berlin_sim::vehdata::game_feel().truck_limiter_tunable => {
                err("Begrenzer ist nicht abschaltbar (feel.json)")
            }
            Some(on) => {
                w.truck_limiter = on;
                ok(format!("Lkw-Begrenzer {}", if on { "an" } else { "aus" }))
            }
            None => err("begrenzer an|aus"),
        },
        "gott" => match on_off(a0, w.god) {
            Some(on) => {
                w.god = on;
                ok(format!("Gottmodus {}", if on { "an" } else { "aus" }))
            }
            None => err("gott an|aus"),
        },
        "auto" => {
            use berlin_sim::carmodels::{CAR_MODELS, spec_line, spec_of, vehicle_name};
            let v = a0.map(norm);
            let model = v.as_ref().and_then(|v| {
                CAR_MODELS
                    .iter()
                    .map(|(m, _)| *m)
                    .find(|m| *m == v || norm(spec_of(m).label) == *v)
            });
            if let Some(m) = model {
                return match w.spawn_vehicle("car", Some(m)) {
                    Some(_) => ok(format!(
                        "{} ({}) steht bereit",
                        vehicle_name(m),
                        spec_line(m)
                    )),
                    None => err("Kein Platz für ein Fahrzeug"),
                };
            }
            // Fahrzeug aus den Fahrzeugdaten (Sattelzug, Gelenkbus, Doppeldecker …)
            if let Some(d) = v.as_ref().and_then(|v| {
                berlin_sim::vehdata::shared()
                    .vehicles
                    .iter()
                    .find(|d| norm(&d.id) == *v || norm(&d.name) == *v)
            }) {
                return match w.spawn_data_vehicle(&d.id) {
                    Some(_) => ok(format!("{} ({}) steht bereit", d.name, d.id)),
                    None => err("Kein Platz für ein Fahrzeug"),
                };
            }
            let kind = match &v {
                None => Some("car"),
                Some(v) => KIND_LABEL
                    .iter()
                    .find(|(k, l)| k == v || norm(l) == *v)
                    .map(|(k, _)| *k),
            };
            let Some(kind) = kind else {
                return err(format!("Art: {}", KIND_LABEL.map(|x| x.0).join(", ")));
            };
            let label = KIND_LABEL
                .iter()
                .find(|x| x.0 == kind)
                .map_or(kind, |x| x.1);
            match w.spawn_vehicle(kind, None) {
                Some(_) => ok(format!("{label} steht bereit")),
                None => err("Kein Platz für ein Fahrzeug"),
            }
        }
        "reparieren" => {
            if repair(w) {
                ok("Auto repariert")
            } else {
                err("Kein Auto")
            }
        }
        "bars" => match a0 {
            None => match &w.city.bars {
                Some(b) => ok(format!(
                    "{} Bars im Feed, {} auf der Karte zugeordnet",
                    b.list.len(),
                    b.list.iter().filter(|b| b.osm).count()
                )),
                None => ok("Kein Bar-Feed – bars <Datei|live>"),
            },
            Some(v) => {
                ctx.actions.push(Action::Bars(Some(v.to_string())));
                ok(if v == "aus" {
                    "Bar-Feed aus"
                } else {
                    "Lade Bar-Feed …"
                })
            }
        },
        "stats" => {
            ctx.actions.push(Action::Stats);
            ok("Statistik")
        }
        "fps" => match on_off(a0, ctx.debug.fps) {
            Some(on) => {
                ctx.debug.fps = on;
                ok(format!("FPS-Anzeige {}", if on { "an" } else { "aus" }))
            }
            None => err("fps an|aus"),
        },
        "ebenen" => match on_off(a0, ctx.debug.levels) {
            Some(on) => {
                ctx.debug.levels = on;
                ok(format!("Ebenen-Ansicht {}", if on { "an" } else { "aus" }))
            }
            None => err("ebenen an|aus"),
        },
        "silhouetten" => match on_off(a0, ctx.debug.silhouettes) {
            Some(on) => {
                ctx.debug.silhouettes = on;
                ok(format!("Silhouetten {}", if on { "an" } else { "aus" }))
            }
            None => err("silhouetten an|aus"),
        },
        _ => err("?"),
    }
}

/// Eigenes Auto (das gefahrene oder das Spielerauto) reparieren.
fn repair(w: &mut World) -> bool {
    let id = w.player.in_car.or(w.player_car_id);
    let Some(c) = id.and_then(|id| w.cars.iter_mut().find(|c| c.id == id)) else {
        return false;
    };
    c.health = berlin_sim::car::HEALTH;
    (c.wrecked, c.wreck_t, c.vx, c.vy, c.ang_vel) = (false, 0., 0., 0., 0.);
    true
}
/// 12345 → „12.345“
fn group(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// Befehl ausführen.
pub fn execute(line: &str, ctx: &mut Ctx) -> Outcome {
    let toks = tokenize(line);
    let Some(first) = toks.first() else {
        return ok("");
    };
    let Some(cmd) = find_command(&first.t) else {
        // ohne Befehlswort: Uhrzeit, Wetter oder Ort – außer es ist ein vertippter Befehl („wetr“)
        let typed = norm(&first.t);
        let cmd_typo = toks.len() == 1
            && COMMANDS
                .iter()
                .any(|c| edit_distance(&typed, c.name, 1) <= 1);
        if !cmd_typo && let Some(s) = smart_line(line, ctx.places) {
            return execute(&s, ctx);
        }
        let keys: Vec<(String, u8)> = COMMANDS.iter().map(|c| (c.name.into(), 0)).collect();
        let near = rank_matches(&keys, &first.t)
            .into_iter()
            .find(|&i| edit_distance(&typed, COMMANDS[i].name, 2) <= 2)
            .or_else(|| {
                let two: String = first.t.chars().take(2).collect();
                rank_matches(&keys, &two).first().copied()
            });
        return err(format!(
            "Unbekannter Befehl „{}“{} (hilfe)",
            first.t,
            near.map_or(String::new(), |i| format!(
                " – meintest du „{}“?",
                COMMANDS[i].name
            ))
        ));
    };
    let rest_arg = cmd.args.iter().position(|a| a.rest);
    let args: Vec<String> = match rest_arg {
        Some(r) => {
            let start = toks.get(r + 1).map_or(line.len(), |t| t.start);
            let rest = line[start..].trim().trim_matches('"').to_string();
            toks[1..(r + 1).min(toks.len())]
                .iter()
                .map(|t| t.t.clone())
                .chain((!rest.is_empty()).then_some(rest))
                .collect()
        }
        None => toks[1..].iter().map(|t| t.t.clone()).collect(),
    };
    if cmd
        .args
        .iter()
        .enumerate()
        .any(|(i, a)| !a.optional && args.get(i).is_none_or(String::is_empty))
    {
        // „schnee“ allein meint eher das Wetter als die Schneedecke ohne Wert
        if toks.len() == 1
            && let Some(s) = smart_line(line, ctx.places)
            && !s.starts_with(&format!("{} ", cmd.name))
            && !s.starts_with("tp ")
        {
            return execute(&s, ctx);
        }
        return err(format!("Fehlt: {}", usage(cmd)));
    }
    let r = run(cmd, ctx, &args);
    if r.ok && cmd.cheat {
        ctx.actions.push(Action::Cheat(cmd.name));
    }
    r
}

pub const WEATHER_ROWS: [&str; 5] = [
    "Wettertyp",
    "Temperatur",
    "Schneedecke",
    "Straßennässe",
    "Glätte",
];
/// Wettertafel: Wert einer Zeile verstellen.
pub fn change_weather(w: &mut World, row: usize, dir: i32) {
    match row {
        0 => {
            let kinds: Vec<Option<&'static str>> = std::iter::once(None)
                .chain(berlin_sim::weather::KINDS.iter().map(|k| Some(*k)))
                .collect();
            let i = kinds
                .iter()
                .position(|k| *k == w.force_weather)
                .unwrap_or(0) as i32;
            let n = kinds.len() as i32;
            w.force_weather = kinds[((i + dir).rem_euclid(n)) as usize];
            if matches!(
                w.force_weather,
                Some("rain" | "heavyrain" | "storm" | "thunder")
            ) {
                w.weather.wet = w.weather.wet.max(0.6);
            }
        }
        1 => {
            let t = match w.force_temp {
                None => {
                    if dir > 0 {
                        0.
                    } else {
                        -1.
                    }
                }
                Some(t) => t + dir as f64,
            };
            w.force_temp = (-30. ..=40.).contains(&t).then_some(t);
        }
        _ => {
            let v = match row {
                2 => &mut w.weather.snow,
                3 => &mut w.weather.wet,
                _ => &mut w.weather.ice,
            };
            *v = (((*v + dir as f64 * 0.1) * 10.).round() / 10.).clamp(0., 1.);
        }
    }
}
/// Wert einer Tafelzeile als Text.
pub fn weather_value(w: &World, row: usize) -> String {
    match row {
        0 => w
            .force_weather
            .map_or("auto".into(), |k| berlin_sim::weather::label(k).into()),
        1 => match w.force_temp {
            Some(t) => format!("{t} °C"),
            None => format!("auto ({:.0} °C)", w.temp),
        },
        2 => format!("{:.0} %", w.weather.snow * 100.),
        3 => format!("{:.0} %", w.weather.wet * 100.),
        _ => format!("{:.0} %", w.weather.ice * 100.),
    }
}

/// Taste der Befehlszeile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Escape,
    Tab,
    Right,
    Left,
    Up,
    Down,
    Enter,
    Backspace,
    DeleteWord,
}
/// Was eine Taste bewirkt hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Did {
    Close,
    Run,
    Edit,
    Nav,
    None,
}

/// Zustand der Befehlszeile.
#[derive(Debug, Clone, Default)]
pub struct Console {
    pub open: bool,
    pub text: String,
    pub sel: Option<usize>,
    pub hist: Vec<String>,
    pub hi: Option<usize>,
    /// Meldungen: Text, gelungen, Zeitpunkt
    pub log: Vec<(String, bool, f64)>,
    pub sugg: Suggest,
    pub weather_panel: bool,
    pub weather_row: usize,
}

impl Console {
    pub fn open(&mut self, places: &[Place]) {
        self.open = true;
        self.text.clear();
        self.sel = None;
        self.hi = None;
        self.refresh(places);
    }
    /// Text setzen (Aufnahmen, Tests).
    pub fn set_text(&mut self, t: &str, places: &[Place]) {
        self.text = t.into();
        self.refresh(places);
    }
    fn refresh(&mut self, places: &[Place]) {
        let mut s = suggest(&self.text, places);
        // leere Zeile: zuletzt benutzte Befehle zuerst
        if self.text.is_empty() && !self.hist.is_empty() {
            let mut recent: Vec<Item> = Vec::new();
            for l in self.hist.iter().rev() {
                if recent.len() < 3 && !recent.iter().any(|r| r.insert == *l) {
                    recent.push(Item {
                        label: l.clone(),
                        hint: "zuletzt".into(),
                        insert: l.clone(),
                        full: true,
                    });
                }
            }
            recent.extend(s.items);
            recent.truncate(MAX_SUGGESTIONS);
            s.items = recent;
            s.ghost.clear();
        }
        if self.sel.is_some_and(|i| i >= s.items.len()) {
            self.sel = None;
        }
        self.sugg = s;
    }
    /// Vorschlag i übernehmen (Tab, →, Mausklick).
    pub fn accept(&mut self, i: usize, places: &[Place]) -> bool {
        let s = self.sugg.clone();
        let Some(it) = s.items.get(i) else {
            return false;
        };
        if it.full {
            self.text = it.insert.clone();
        } else {
            let more = match s.cmd.and_then(find_command) {
                Some(c) => !s.rest && c.args.len() > s.argi,
                None => find_command(&it.insert).is_some_and(|c| !c.args.is_empty()),
            };
            let ins = if it.insert.contains(' ') && !s.rest {
                format!("\"{}\"", it.insert)
            } else {
                it.insert.clone()
            };
            self.text = format!(
                "{}{}{}",
                &self.text[..s.from],
                ins,
                if more { " " } else { "" }
            );
        }
        self.sel = None;
        self.refresh(places);
        true
    }
    /// Taste verarbeiten; `shift` lässt die Zeile nach Erfolg offen.
    pub fn key(&mut self, key: Key, ctx: &mut Ctx, now: f64, shift: bool) -> Did {
        let places = ctx.places;
        if self.weather_panel {
            return match key {
                Key::Escape | Key::Backspace | Key::Enter => {
                    self.weather_panel = false;
                    self.text.clear();
                    self.refresh(places);
                    Did::Edit
                }
                Key::Up | Key::Down => {
                    let n = WEATHER_ROWS.len();
                    self.weather_row =
                        (self.weather_row + if key == Key::Down { 1 } else { n - 1 }) % n;
                    Did::Nav
                }
                Key::Left | Key::Right => {
                    change_weather(
                        ctx.world,
                        self.weather_row,
                        if key == Key::Right { 1 } else { -1 },
                    );
                    Did::Nav
                }
                _ => Did::None,
            };
        }
        match key {
            Key::Escape => {
                if !self.text.is_empty() {
                    self.text.clear();
                    self.sel = None;
                    self.hi = None;
                    self.refresh(places);
                    return Did::Edit;
                }
                self.open = false;
                self.sel = None;
                Did::Close
            }
            Key::Tab | Key::Right => {
                if self.accept(self.sel.unwrap_or(0), places) {
                    Did::Edit
                } else {
                    Did::None
                }
            }
            Key::Left => Did::None,
            Key::Up | Key::Down => {
                let n = self.sugg.items.len();
                let down = key == Key::Down;
                if n > 0
                    && self.hi.is_none()
                    && (self.sel.is_some() || down || self.hist.is_empty())
                {
                    self.sel = Some(match self.sel {
                        None => {
                            if down {
                                0
                            } else {
                                n - 1
                            }
                        }
                        Some(i) => (i + if down { 1 } else { n - 1 }) % n,
                    });
                    return Did::Nav;
                }
                if self.hist.is_empty() {
                    return Did::None;
                }
                let cur = self.hi.map_or(-1, |h| h as i64);
                let next = (cur + if down { -1 } else { 1 }).clamp(-1, self.hist.len() as i64 - 1);
                self.hi = (next >= 0).then_some(next as usize);
                self.text = match self.hi {
                    None => String::new(),
                    Some(h) => self.hist[self.hist.len() - 1 - h].clone(),
                };
                self.sel = None;
                self.refresh(places);
                Did::Nav
            }
            Key::Enter => {
                if let Some(i) = self.sel {
                    self.accept(i, places);
                    return Did::Edit;
                }
                let line = self.text.trim().to_string();
                if line.is_empty() {
                    self.open = false;
                    return Did::Close;
                }
                if norm(&line) == "wetter" {
                    self.weather_panel = true;
                    self.weather_row = 0;
                    return Did::Nav;
                }
                // erst einen abweichenden Vorschlag übernehmen, das nächste Enter führt aus
                let s = suggest(&self.text, places);
                if let Some(it) = s.items.first() {
                    let cand = if it.full {
                        it.insert.clone()
                    } else {
                        let ins = if it.insert.contains(' ') && !s.rest {
                            format!("\"{}\"", it.insert)
                        } else {
                            it.insert.clone()
                        };
                        format!("{}{}", &self.text[..s.from], ins)
                    };
                    if norm(cand.trim()) != norm(&line) {
                        self.sugg = s;
                        if self.accept(0, places) {
                            return Did::Edit;
                        }
                    }
                }
                let r = execute(&line, ctx);
                self.log.push((format!("> {line}"), true, now));
                if !r.msg.is_empty() {
                    self.log.push((r.msg.clone(), r.ok, now));
                }
                let over = self.log.len().saturating_sub(MAX_LOG);
                self.log.drain(..over);
                if self.hist.last() != Some(&line) {
                    self.hist.push(line);
                }
                let over = self.hist.len().saturating_sub(HISTORY);
                self.hist.drain(..over);
                self.hi = None;
                self.sel = None;
                if r.ok {
                    self.text.clear();
                    if !shift || ctx.actions.contains(&Action::Stats) {
                        self.open = false;
                    }
                }
                self.refresh(places);
                Did::Run
            }
            Key::Backspace => {
                self.text.pop();
                self.sel = None;
                self.hi = None;
                self.refresh(places);
                Did::Edit
            }
            Key::DeleteWord => {
                let t = self.text.trim_end();
                let cut = t.rfind(char::is_whitespace).map_or(0, |i| i + 1);
                self.text.truncate(cut);
                self.sel = None;
                self.hi = None;
                self.refresh(places);
                Did::Edit
            }
            Key::Char(c) => {
                self.text.push(c);
                self.sel = None;
                self.hi = None;
                self.refresh(places);
                Did::Edit
            }
        }
    }
}

/// Befehlszeile zeichnen (offen: Eingabe, Vorschläge, Hilfezeile, Meldungen; zu: frische Meldungen ausblenden).
pub fn draw(h: &mut berlin_engine::hud::Hud, con: &Console, w: &World) {
    use berlin_engine::hud::Align;
    let now = w.time;
    let (x, bw) = (24., (h.width - 48.).min(820.));
    let yellow = [1., 0.83, 0.24, 1.];
    let dim = [0.66, 0.68, 0.72, 1.];
    if !con.open {
        // letzte Meldungen über der Minikarte, blenden aus
        let mut y = 560.;
        for (t, ok, at) in con.log.iter().rev().take(3) {
            let age = now - at;
            if !(0. ..LOG_TIME).contains(&age) {
                continue;
            }
            let a = (((LOG_TIME - age) / 1.5).min(1.)) as f32;
            let c = if *ok {
                [0.9, 0.92, 0.95, a]
            } else {
                [1., 0.55, 0.5, a]
            };
            h.text(t, x, y, 13., c, Align::Left, true);
            y -= 18.;
        }
        return;
    }
    let base = 690.;
    if con.weather_panel {
        let rows = WEATHER_ROWS.len() as f32;
        let top = base - 40. - rows * 26.;
        h.rect(x, top, bw, base - top + 10., [0.05, 0.06, 0.08, 0.92], 10.);
        h.text(
            "WETTER · ↑↓ Zeile · ←→ Wert · Enter fertig",
            x + 16.,
            top + 24.,
            13.,
            yellow,
            Align::Left,
            false,
        );
        for (i, r) in WEATHER_ROWS.iter().enumerate() {
            let y = top + 54. + i as f32 * 26.;
            if i == con.weather_row {
                h.rect(x + 8., y - 18., bw - 16., 24., [1., 0.83, 0.24, 0.18], 6.);
            }
            h.text(
                r,
                x + 20.,
                y,
                14.,
                [0.92, 0.93, 0.95, 1.],
                Align::Left,
                false,
            );
            h.text(
                &format!("‹ {} ›", weather_value(w, i)),
                x + bw - 20.,
                y,
                14.,
                yellow,
                Align::Right,
                false,
            );
        }
        return;
    }
    let s = &con.sugg;
    let n = s.items.len() as f32;
    let help_h = if s.help.is_empty() { 0. } else { 22. };
    let top = base - 34. - n * 22. - help_h - 8.;
    h.rect(x, top, bw, base - top + 8., [0.05, 0.06, 0.08, 0.9], 10.);
    // Eingabezeile mit grauem Rest des ersten Vorschlags und Schreibmarke
    h.rect(
        x + 8.,
        base - 26.,
        bw - 16.,
        30.,
        [0.11, 0.12, 0.15, 1.],
        6.,
    );
    let prompt = format!("> {}", con.text);
    let tw = h.text(
        &prompt,
        x + 18.,
        base - 6.,
        16.,
        [1.; 4],
        Align::Left,
        false,
    );
    if !s.ghost.is_empty() && con.text.ends_with(&s.query) {
        h.text(
            &s.ghost,
            x + 18. + tw,
            base - 6.,
            16.,
            [0.5, 0.52, 0.56, 1.],
            Align::Left,
            false,
        );
    }
    if (now * 2.).fract() < 0.5 || con.text.is_empty() {
        h.rect(x + 19. + tw, base - 20., 2., 18., yellow, 0.);
    }
    let mut y = top + 22.;
    if !s.help.is_empty() {
        h.text(&s.help, x + 16., y, 13., yellow, Align::Left, false);
        y += 22.;
    }
    for (i, it) in s.items.iter().enumerate() {
        if con.sel == Some(i) {
            h.rect(x + 8., y - 16., bw - 16., 21., [1., 0.83, 0.24, 0.2], 5.);
        }
        h.text(
            &it.label,
            x + 20.,
            y,
            14.,
            [0.94, 0.95, 0.97, 1.],
            Align::Left,
            false,
        );
        h.text(&it.hint, x + bw - 20., y, 12., dim, Align::Right, false);
        y += 22.;
    }
    // Meldungen über der Tafel
    let mut ly = top - 10.;
    for (t, ok, _) in con.log.iter().rev().take(4) {
        let c = if *ok {
            [0.9, 0.92, 0.95, 1.]
        } else {
            [1., 0.55, 0.5, 1.]
        };
        h.text(t, x + 4., ly, 13., c, Align::Left, true);
        ly -= 18.;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn places() -> Vec<Place> {
        let p = |name: &str, kind: &'static str, rank: u8| Place {
            name: name.into(),
            kind,
            x: 1000.,
            y: 1000.,
            rank,
            key: norm(name),
        };
        vec![
            p("Kreuzberg", "Ortsteil", 1),
            p("Kottbusser Tor", "U-Bahnhof", 2),
            p("Alexanderplatz", "S-Bahnhof", 2),
            p("Oranienstraße", "Straße", 4),
        ]
    }
    #[test]
    fn parsing_helpers() {
        assert_eq!(norm("Straße Ä"), "strasse a");
        assert_eq!(clock_arg("21:30"), Some(1290.));
        assert_eq!(clock_arg("21.30"), Some(1290.));
        assert_eq!(clock_arg("2130"), Some(1290.));
        assert_eq!(clock_arg("7 Uhr"), Some(420.));
        assert_eq!(clock_arg("21h"), Some(1260.));
        assert_eq!(clock_arg("25:00"), None);
        assert_eq!(clock_arg("abc"), None);
        assert_eq!(weekday_arg("Fr"), Some(4));
        assert_eq!(weekday_arg("7"), Some(6));
        assert_eq!(weekday_arg("sonnabend"), Some(5));
        assert_eq!(edit_distance("wetter", "wetr", 2), 2);
        assert_eq!(edit_distance("abc", "abcdef", 2), 3);
        assert_eq!(group(1234567), "1.234.567");
        let t = tokenize(r#"tp "Kottbusser Tor" x"#);
        assert_eq!(t.len(), 3);
        assert_eq!(t[1].t, "Kottbusser Tor");
    }
    #[test]
    fn ranking_and_suggestions() {
        let pl = places();
        let r = rank_places(&pl, "kott");
        assert_eq!(r[0].name, "Kottbusser Tor");
        assert_eq!(
            rank_places(&pl, "alexanderplats")[0].name,
            "Alexanderplatz",
            "Tippfehler"
        );
        let s = suggest("we", &pl);
        assert_eq!(s.items[0].label, "wetter");
        assert_eq!(s.ghost, "tter");
        assert!(s.help.starts_with("wetter <wetter>"));
        let s = suggest("wetter re", &pl);
        assert_eq!(s.items[0].label, "regen");
        let s = suggest("tp kott", &pl);
        assert!(s.rest && s.items[0].label == "Kottbusser Tor");
        assert_eq!(smart_line("22:30", &pl).as_deref(), Some("zeit 22:30"));
        assert_eq!(smart_line("regen", &pl).as_deref(), Some("wetter regen"));
        assert_eq!(
            smart_line("kottbusser", &pl).as_deref(),
            Some("tp Kottbusser Tor")
        );
        assert_eq!(
            smart_line("kotbuser", &pl),
            None,
            "ohne Befehlswort kein Tippfehler-Raten"
        );
        let s = suggest("22", &pl);
        assert!(s.items.iter().any(|i| i.full && i.insert == "zeit 22"));
    }

    #[test]
    fn commands_change_the_world() {
        use berlin_sim::city::{City, DiskSource};
        let root = berlin_map_loader::default_data_root();
        let city = City::open(&root, Box::new(DiskSource::new(root.clone()))).unwrap();
        let mut w = World::new(city, 5, 8, 4);
        w.update(&berlin_sim::world::Input::default(), berlin_sim::world::DT);
        let ov = berlin_map_loader::overview::Overview::read(&root).unwrap();
        let labels = Labels {
            bezirke: ov.bezirke,
            ortsteile: ov.ortsteile,
            kieze: ov.kieze,
            stations: ov.stations,
            streets: ov.streets,
            ..Default::default()
        };
        let places = place_index(&labels);
        assert!(
            places
                .iter()
                .any(|p| p.kind == "Straße" && p.name == "Oranienstraße")
        );
        let run = |line: &str, w: &mut World| {
            let mut ctx = Ctx {
                world: w,
                places: &places,
                actions: Vec::new(),
                debug: Default::default(),
            };
            let r = execute(line, &mut ctx);
            (r, ctx.actions)
        };
        assert!(run("zeit 22:15", &mut w).0.ok);
        assert_eq!(w.clock, 22. * 60. + 15.);
        assert!(run("nacht", &mut w).0.ok, "ohne Befehlswort");
        assert_eq!(w.clock, 23. * 60. + 30.);
        assert!(run("wetter regen", &mut w).0.ok);
        assert_eq!(w.force_weather, Some("rain"));
        assert!(w.weather.wet >= 0.6);
        assert!(run("tag sa", &mut w).0.ok);
        assert_eq!(w.day, 5);
        let (r, a) = run("geld +500", &mut w);
        assert!(r.ok && w.money >= 500.);
        assert!(a.contains(&Action::Cheat("geld")) && matches!(a[0], Action::Money(_)));
        let (r, a) = run("tp Kottbusser Tor", &mut w);
        assert!(r.ok, "{}", r.msg);
        assert!(matches!(&a[0], Action::Teleport { name, .. } if name == "Kottbusser Tor"));
        let (r, a) = run("ziel Kottbusser Tor", &mut w);
        assert!(r.ok, "{}", r.msg);
        assert!(matches!(&a[0], Action::Waypoint(Some((_, _, name))) if name == "Kottbusser Tor"));
        let (r, a) = run("route aus", &mut w);
        assert!(r.ok && a == [Action::Waypoint(None)], "Alias + aus");
        assert!(!run("ziel", &mut w).0.ok, "ohne Ort");
        let n = w.cars.len();
        let (r, _) = run("auto motorrad", &mut w);
        assert!(r.ok, "{}", r.msg);
        assert_eq!(w.cars.len(), n + 1);
        assert_eq!(w.cars.last().unwrap().kind, "motorcycle");
        let r = run("wettr", &mut w).0;
        assert!(!r.ok && r.msg.contains("„wetter“"), "{}", r.msg);
        assert!(!run("schnee 7", &mut w).0.ok);
        assert!(run("gott", &mut w).0.ok && w.god);
        assert!(run("temp -5", &mut w).0.ok && w.force_temp == Some(-5.));
        // Anzeige-Schalter: ohne Wert umschalten, an/aus setzen, Unsinn abweisen
        let mut dbg = |line: &str, d: Debug| {
            let mut ctx = Ctx {
                world: &mut w,
                places: &places,
                actions: Vec::new(),
                debug: d,
            };
            let r = execute(line, &mut ctx);
            (r.ok, ctx.debug)
        };
        let d0 = Debug::default();
        assert!(d0.silhouettes && !d0.fps && !d0.levels);
        assert_eq!(dbg("fps", d0), (true, Debug { fps: true, ..d0 }));
        assert_eq!(dbg("fps aus", Debug { fps: true, ..d0 }), (true, d0));
        assert_eq!(
            dbg("levels an", d0),
            (true, Debug { levels: true, ..d0 }),
            "englischer Alias"
        );
        assert_eq!(
            dbg("silhouetten aus", d0),
            (
                true,
                Debug {
                    silhouettes: false,
                    ..d0
                }
            )
        );
        assert_eq!(dbg("fps vielleicht", d0), (false, d0));
        // Konsole: Enter übernimmt erst den Vorschlag, das zweite führt aus
        let mut con = Console::default();
        con.open(&places);
        let mut ctx = Ctx {
            world: &mut w,
            places: &places,
            actions: Vec::new(),
            debug: Default::default(),
        };
        for c in "zeit mit".chars() {
            con.key(Key::Char(c), &mut ctx, 0., false);
        }
        assert_eq!(con.key(Key::Enter, &mut ctx, 0., false), Did::Edit);
        assert_eq!(con.text, "zeit mittag");
        assert_eq!(con.key(Key::Enter, &mut ctx, 0., false), Did::Run);
        assert_eq!(ctx.world.clock, 720.);
        assert!(!con.open, "Erfolg schließt");
        assert_eq!(con.hist, vec!["zeit mittag".to_string()]);
    }
}
