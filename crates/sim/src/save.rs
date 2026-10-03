//! Spielstand (Port von `save.js`): ein Speicherplatz, jetzt als JSON-Datei statt `localStorage`. Das Format ist
//! dasselbe wie in der Browserfassung (Version 2), ein exportierter Stand lässt sich also übernehmen.
//! Geschrieben wird atomar (temporäre Datei + Umbenennen); beschädigte oder unplausible Stände werden verworfen.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const SAVE_VERSION: u32 = 2;
const MAX_XY: f64 = 1e7;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedPoint {
    pub x: f64,
    pub y: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedCar {
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub health: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub model: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveData {
    pub version: u32,
    pub saved_at: f64,
    pub money: f64,
    pub completed: f64,
    pub best_time: Option<f64>,
    pub clock: Option<f64>,
    pub day: Option<u32>,
    pub day_count: Option<u32>,
    pub wet: Option<f64>,
    pub snow: Option<f64>,
    pub ice: Option<f64>,
    pub player: SavedPoint,
    pub car: Option<SavedCar>,
}

fn num(v: &Value, lo: f64, hi: f64) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite() && *n >= lo && *n <= hi)
}

/// Prüft einen gelesenen Stand wie `validateSave`: falsche Version oder unplausible Werte → None; einzelne
/// unbrauchbare Zusatzfelder (Auto, Uhr, Wetter) fallen weg, statt den ganzen Stand zu verwerfen.
pub fn validate(v: &Value) -> Option<SaveData> {
    if v.get("version")?.as_u64()? != SAVE_VERSION as u64 {
        return None;
    }
    let money = num(&v["money"], 0., 1e9)?;
    let completed = num(&v["completed"], 0., 1e6)?;
    let best_time = match &v["bestTime"] {
        Value::Null => None,
        b => Some(num(b, 0., 1e5)?),
    };
    let p = &v["player"];
    let player = SavedPoint {
        x: num(&p["x"], 0., MAX_XY)?,
        y: num(&p["y"], 0., MAX_XY)?,
    };
    let c = &v["car"];
    let car = (|| {
        Some(SavedCar {
            x: num(&c["x"], 0., MAX_XY)?,
            y: num(&c["y"], 0., MAX_XY)?,
            angle: num(&c["angle"], -100., 100.)?,
            health: num(&c["health"], 1., 100.)?,
            model: c["model"]
                .as_str()
                .filter(|m| crate::carmodels::CAR_MODELS.iter().any(|(k, _)| k == m))
                .map(str::to_owned),
        })
    })();
    let int = |k: &str, max: u64| v[k].as_u64().filter(|d| *d < max).map(|d| d as u32);
    Some(SaveData {
        version: SAVE_VERSION,
        saved_at: v["savedAt"].as_f64().unwrap_or(0.),
        money,
        completed,
        best_time,
        clock: num(&v["clock"], 0., 1440.),
        day: int("day", 7),
        day_count: int("dayCount", u32::MAX as u64),
        wet: num(&v["wet"], 0., 1.),
        snow: num(&v["snow"], 0., 1.),
        ice: num(&v["ice"], 0., 1.),
        player,
        car,
    })
}

/// Ablage eines Spielstands.
pub trait Storage {
    fn read(&self) -> Option<String>;
    fn write(&mut self, data: &str) -> Result<()>;
}

/// Für Tests: im Speicher.
#[derive(Debug, Default, Clone)]
pub struct MemoryStorage(pub Option<String>);
impl Storage for MemoryStorage {
    fn read(&self) -> Option<String> {
        self.0.clone()
    }
    fn write(&mut self, data: &str) -> Result<()> {
        self.0 = Some(data.to_owned());
        Ok(())
    }
}

/// Datei im Benutzerdatenordner.
#[derive(Debug, Clone)]
pub struct FileStorage {
    pub path: PathBuf,
}
impl FileStorage {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
    /// macOS: ~/Library/Application Support/GTA Berlin/save.json · Windows: %APPDATA%\GTA Berlin\save.json ·
    /// sonst $XDG_DATA_HOME/gta-berlin/save.json (bzw. ~/.local/share/…).
    pub fn default_path() -> PathBuf {
        let env = |k: &str| {
            std::env::var_os(k)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        if cfg!(target_os = "windows") {
            if let Some(a) = env("APPDATA") {
                return a.join("GTA Berlin").join("save.json");
            }
        } else if cfg!(target_os = "macos") {
            if let Some(h) = env("HOME") {
                return h.join("Library/Application Support/GTA Berlin/save.json");
            }
        } else if let Some(x) = env("XDG_DATA_HOME") {
            return x.join("gta-berlin/save.json");
        } else if let Some(h) = env("HOME") {
            return h.join(".local/share/gta-berlin/save.json");
        }
        PathBuf::from("gta-berlin-save.json")
    }
}
impl Storage for FileStorage {
    fn read(&self) -> Option<String> {
        std::fs::read_to_string(&self.path).ok()
    }
    fn write(&mut self, data: &str) -> Result<()> {
        write_atomic(&self.path, data)
    }
}
fn write_atomic(path: &Path, data: &str) -> Result<()> {
    if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("{} anlegen", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, data).with_context(|| format!("{} schreiben", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("{} ersetzen", path.display()))?;
    Ok(())
}

pub fn write_save(storage: &mut dyn Storage, data: &SaveData) -> Result<()> {
    storage.write(&serde_json::to_string(data)?)
}
pub fn read_save(storage: &dyn Storage) -> Option<SaveData> {
    let text = storage.read()?;
    validate(&serde_json::from_str(&text).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> SaveData {
        SaveData {
            version: 2,
            saved_at: 1.7e12,
            money: 1234.,
            completed: 3.,
            best_time: Some(88.5),
            clock: Some(1000.),
            day: Some(4),
            day_count: Some(2),
            wet: Some(0.4),
            snow: None,
            ice: Some(0.),
            player: SavedPoint {
                x: 237936.,
                y: 196107.,
            },
            car: Some(SavedCar {
                x: 237695.,
                y: 195989.,
                angle: -2.55,
                health: 77.,
                model: Some("rallye".into()),
            }),
        }
    }

    #[test]
    fn roundtrip_memory_and_js_field_names() {
        let mut st = MemoryStorage::default();
        assert!(read_save(&st).is_none());
        write_save(&mut st, &sample()).unwrap();
        let text = st.0.clone().unwrap();
        assert!(
            text.contains("\"bestTime\":88.5")
                && text.contains("\"dayCount\":2")
                && text.contains("\"savedAt\"")
        );
        assert_eq!(read_save(&st), Some(sample()));
    }

    #[test]
    fn rejects_corrupt_and_implausible_saves() {
        let st = MemoryStorage(Some("{kaputt".into()));
        assert!(read_save(&st).is_none());
        let ok = serde_json::to_value(sample()).unwrap();
        let bad = |k: &str, v: Value| {
            let mut x = ok.clone();
            x[k] = v;
            validate(&x)
        };
        assert!(bad("version", json!(1)).is_none());
        assert!(bad("money", json!(-1)).is_none());
        assert!(bad("money", json!("viel")).is_none());
        assert!(bad("player", json!({"x": 1e9, "y": 0})).is_none());
        // Zusatzfelder fallen einzeln weg
        let s = bad("car", json!({"x": 1, "y": 1, "angle": 0, "health": 0})).unwrap();
        assert!(s.car.is_none());
        let s = bad("clock", json!(5000)).unwrap();
        assert!(s.clock.is_none() && s.money == 1234.);
        let s = bad("day", json!(9)).unwrap();
        assert!(s.day.is_none());
        let mut x = ok.clone();
        x["car"]["model"] = json!("raumschiff");
        assert_eq!(validate(&x).unwrap().car.unwrap().model, None);
        // Stand aus der Browserfassung (ohne Wetter, Uhr, Modell)
        let js = json!({"version": 2, "savedAt": 1, "money": 0, "completed": 0, "bestTime": null, "player": {"x": 10, "y": 20}, "car": null});
        let s = validate(&js).unwrap();
        assert_eq!(
            (s.best_time, s.clock, s.car.is_none(), s.player.x),
            (None, None, true, 10.)
        );
    }

    #[test]
    fn file_storage_is_atomic() {
        let dir = std::env::temp_dir().join(format!("gta-berlin-save-test-{}", std::process::id()));
        let path = dir.join("sub/save.json");
        let mut st = FileStorage::new(&path);
        assert!(read_save(&st).is_none());
        write_save(&mut st, &sample()).unwrap();
        assert!(path.exists() && !path.with_extension("json.tmp").exists());
        assert_eq!(read_save(&st), Some(sample()));
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(FileStorage::default_path().ends_with("save.json"));
    }
}
