//! Bar-Auslastung live aus dem Netz (Port von `tools/bars-source.mjs`): `GET …/bars/busyness` liefert alle Bars mit
//! Koordinaten, Live-Auslastung, „üblich“ und 24-h-Verlauf; endet die Adresse so, kommt der Wochenschnitt aller Bars
//! je Wochentag dazu (`…/weekly?dow=0..6`, 0 = Sonntag; Fehler dort sind egal). Abgerufen wird nur auf ausdrücklichen
//! Wunsch (`--bars live|URL`, Befehl `bars URL`), in einem eigenen Faden, alle zwei Minuten neu. Ein Fehlschlag
//! lässt den zuletzt geladenen Stand stehen.
use serde_json::{Value, json};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

pub const GOSTUMBLR: &str = "https://app.gostumblr.com/api/v1/bars/busyness";
pub const REFRESH: Duration = Duration::from_secs(120);

/// `live` steht für die gostumblr-Adresse; sonst muss es eine http(s)-Adresse sein.
pub fn url_of(arg: &str) -> Option<String> {
    if arg == "live" {
        Some(GOSTUMBLR.into())
    } else if arg.starts_with("http://") || arg.starts_with("https://") {
        Some(arg.into())
    } else {
        None
    }
}

fn get_json(agent: &ureq::Agent, url: &str) -> Result<Value, String> {
    let mut req = agent.get(url).set("accept", "application/json");
    if let Ok(t) = std::env::var("BARS_TOKEN") {
        req = req.set("authorization", &format!("Bearer {t}"));
    }
    let body = req
        .call()
        .map_err(|e| format!("{url}: {e}"))?
        .into_string()
        .map_err(|e| format!("{url}: {e}"))?;
    serde_json::from_str(&body).map_err(|e| format!("{url}: {e}"))
}

/// Endet der Pfad (ohne Abfrage) auf `/bars/busyness`?
pub fn wants_weekly(url: &str) -> bool {
    let path = url.split('?').next().unwrap_or(url).trim_end_matches('/');
    path.ends_with("/bars/busyness")
}

/// Wochenschnitt-Antworten in das Feed-Format (`weekly: [{dow, hours: [{hour, avg_occupancy}]}]`).
pub fn merge_weekly(doc: Value, days: &[Value]) -> Value {
    let mut out = match doc {
        Value::Array(a) => json!({ "bars": a }),
        v => v,
    };
    if !days.is_empty() && out.get("weekly").is_none() {
        let weekly: Vec<Value> = days
            .iter()
            .map(|d| {
                let hours: Vec<Value> = d["hours"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|h| json!({ "hour": h["hour"], "avg_occupancy": h["avg_occupancy"] }))
                    .collect();
                json!({ "dow": d["dow"], "hours": hours })
            })
            .collect();
        out["weekly"] = Value::Array(weekly);
    }
    if out.get("at").is_none() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        out["at"] = json!(now);
    }
    out
}

/// Ein vollständiger Abruf wie `fetchBars`.
pub fn fetch(url: &str) -> Result<Value, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .build();
    let doc = get_json(&agent, url)?;
    let days: Vec<Value> = if wants_weekly(url) {
        let base = url.split('?').next().unwrap_or(url).trim_end_matches('/');
        (0..7)
            .filter_map(|dow| get_json(&agent, &format!("{base}/weekly?dow={dow}")).ok())
            .collect()
    } else {
        Vec::new()
    };
    Ok(merge_weekly(doc, &days))
}

/// Abruf im Hintergrund: liefert jedes Ergebnis über einen Kanal; endet, wenn der Empfänger weg ist.
pub struct Live {
    pub url: String,
    rx: Receiver<Result<Value, String>>,
    _stop: Sender<()>,
}
impl Live {
    pub fn start(url: String) -> Self {
        let (tx, rx) = channel();
        let (stop_tx, stop_rx) = channel::<()>();
        let u = url.clone();
        std::thread::Builder::new()
            .name("bar-feed".into())
            .spawn(move || {
                loop {
                    let t = Instant::now();
                    if tx.send(fetch(&u)).is_err() {
                        return;
                    }
                    // warten, bis die nächste Runde fällig ist oder das Spiel den Abruf beendet
                    match stop_rx.recv_timeout(REFRESH.saturating_sub(t.elapsed())) {
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        _ => return,
                    }
                }
            })
            .expect("Faden für den Bar-Feed");
        Self {
            url,
            rx,
            _stop: stop_tx,
        }
    }
    /// Neueste fertige Antwort (ältere werden übersprungen).
    pub fn poll(&self) -> Option<Result<Value, String>> {
        self.rx.try_iter().last()
    }
    /// Blockierend auf die erste Antwort warten (Aufnahmen, `--befehl`).
    pub fn wait(&self, max: Duration) -> Option<Result<Value, String>> {
        self.rx.recv_timeout(max).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_and_weekly_detection() {
        assert_eq!(url_of("live").as_deref(), Some(GOSTUMBLR));
        assert_eq!(url_of("https://x.de/a").as_deref(), Some("https://x.de/a"));
        assert!(url_of("web/data/bars.json").is_none());
        assert!(wants_weekly(GOSTUMBLR));
        assert!(wants_weekly(
            "https://x.de/api/v1/bars/busyness/?city=berlin"
        ));
        assert!(!wants_weekly("https://x.de/bars.json"));
    }

    #[test]
    fn merge_wraps_lists_and_adds_the_week() {
        let doc = json!([{ "name": "A", "latitude": 52.5, "longitude": 13.4 }]);
        let days =
            vec![json!({ "dow": 5, "hours": [{ "hour": 22, "avg_occupancy": 61, "x": 1 }] })];
        let out = merge_weekly(doc, &days);
        assert_eq!(out["bars"][0]["name"], "A");
        assert_eq!(out["weekly"][0]["dow"], 5);
        assert_eq!(
            out["weekly"][0]["hours"][0],
            json!({ "hour": 22, "avg_occupancy": 61 })
        );
        assert!(out["at"].as_u64().is_some_and(|t| t > 1_700_000_000));
        // vorhandener Wochenschnitt und Zeitstempel bleiben
        let kept = merge_weekly(json!({ "bars": [], "weekly": [], "at": 5 }), &days);
        assert_eq!(kept["weekly"], json!([]));
        assert_eq!(kept["at"], 5);
        assert!(
            merge_weekly(json!({ "bars": [] }), &[])
                .get("weekly")
                .is_none()
        );
    }

    /// Netz: `cargo test -p gta-berlin -- --ignored live_feed`
    #[test]
    #[ignore = "braucht Netz"]
    fn live_feed_has_bars_and_the_week() {
        let doc = fetch(GOSTUMBLR).expect("gostumblr erreichbar");
        let (_, _, bars) = berlin_sim::nightlife::parse_bar_feed(&doc).expect("Feed lesbar");
        assert!(bars.len() > 20, "{} Bars", bars.len());
        assert_eq!(
            doc["weekly"].as_array().map(Vec::len),
            Some(7),
            "Wochenschnitt"
        );
    }
}
