//! C-Schnittstelle für die UWP-Hülle `xbox/RustProbe` (nur Windows).
//!
//! `probe_start` kommt vom UI-Thread der Hülle (`SetSwapChain` am `SwapChainPanel` verlangt ihn), baut Gerät und Last
//! auf und startet einen eigenen Render-Thread: auf dem UI-Thread blockierte das Warten auf das nächste Bild (Fifo) auf
//! der Xbox das Panel, das selbst den UI-Thread braucht. Geht das Gerät verloren (auf der Konsole einmal mit
//! `DXGI_ERROR_DRIVER_INTERNAL_ERROR` beim Aufbau), beendet sich der Render-Thread; `probe_tick` (Timer der Hülle, UI-Thread)
//! baut dann mit der nächsten Variante aus `VARIANTS` neu auf. Jeder Versuch bleibt im Bericht stehen.
//!
//! Ein Wach-Thread schreibt alle 2 s den Bericht samt Herzschlag (Schritt, in dem das Zeichnen gerade steckt) nach
//! `probe-status.txt` im übergebenen Ordner (LocalState der App, über das Device Portal abholbar) – auch wenn das
//! Zeichnen hängt. Eine Panik schreibt `probe-panic.txt` dorthin.
use crate::{Probe, STAGE, STAGES};
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Versuche: (Beschreibung, Zeitstempel, Swapchain-Teiler). Zeitstempel sind aus: auf der Xbox verlor der Treiber
/// beim Anlegen der Zeitstempel-Abfragen zweimal das Gerät (`DXGI_ERROR_DRIVER_INTERNAL_ERROR`). Ein Neuaufbau direkt
/// danach scheiterte an der zurückgesetzten GPU (`DEVICE_RESET`, dann gar kein Adapter), daher nur ein zweiter Versuch
/// nach `RETRY_PAUSE_MS`.
const VARIANTS: [(&str, bool, u32); 2] = [
    ("ohne Zeitstempel, Fence-Messung", false, 1),
    ("ohne Zeitstempel, nach Pause erneut", false, 1),
];
const RETRY_PAUSE_MS: u64 = 10_000;

/// Bericht des laufenden Versuchs (Prüfschritte bzw. `Probe::status`), vom Render-Thread erneuert.
static TEXT: Mutex<String> = Mutex::new(String::new());
/// Ergebnisse abgeschlossener Versuche.
static HISTORY: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Prüfschritte vor dem ersten Gerät (DLLs), für jeden Versuch wiederholt.
static BASE: Mutex<Vec<String>> = Mutex::new(Vec::new());
static RUNNING: AtomicBool = AtomicBool::new(false);
/// Render-Thread hat aufgegeben (Gerät verloren) bzw. Aufbau gescheitert: `probe_tick` versucht die nächste Variante.
static FAILED: AtomicBool = AtomicBool::new(false);
/// Zeitpunkt (ms seit `START`) des Scheiterns, für die Pause vor dem nächsten Versuch.
static FAILED_MS: AtomicU64 = AtomicU64::new(0);
static ATTEMPT: AtomicU32 = AtomicU32::new(0);
/// Durchläufe des Render-Threads und Zeitpunkt (ms seit `START`) des letzten fertigen.
static LOOPS: AtomicU32 = AtomicU32::new(0);
static BEAT_MS: AtomicU64 = AtomicU64::new(0);
static START: OnceLock<Instant> = OnceLock::new();
/// Größenauftrag der Hülle an den Render-Thread: Breite << 32 | Höhe, 0 = keiner.
static RESIZE: AtomicU64 = AtomicU64::new(0);
/// `ISwapChainPanelNative*` (von der Hülle per QueryInterface geholt, also mit eigener Referenz) und Startgröße.
static PANEL: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static SIZE: AtomicU64 = AtomicU64::new(0);
static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();

fn now_ms() -> u64 {
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

fn dir() -> &'static Option<PathBuf> {
    DIR.get_or_init(|| None)
}

fn wide(p: *const u16) -> Option<String> {
    if p.is_null() {
        return None;
    }
    let mut n = 0;
    while unsafe { *p.add(n) } != 0 {
        n += 1;
    }
    Some(String::from_utf16_lossy(unsafe {
        std::slice::from_raw_parts(p, n)
    }))
}

/// Lädt eine System-DLL so, wie wgpu es tut (`LoadLibraryExW` über libloading) – die heikelste Stelle im UWP-Sandkasten.
fn dll(name: &str) -> String {
    match unsafe { libloading::Library::new(name) } {
        Ok(_) => format!("DLL {name}: geladen"),
        Err(e) => format!("DLL {name}: FEHLER {e}"),
    }
}

/// Zwischenstand vor einem heiklen Aufruf: stürzt der Prozess darin ab, zeigt `probe-status.txt` die Stelle.
fn checkpoint(steps: &[String], next: &str) {
    if let Some(dir) = dir() {
        let _ = std::fs::write(
            dir.join("probe-status.txt"),
            format!(
                "{}{}\nnächster Schritt: {next}",
                history(),
                steps.join("\n")
            ),
        );
    }
}

fn history() -> String {
    match HISTORY.lock() {
        Ok(h) if !h.is_empty() => format!("Frühere Versuche:\n{}\n\n", h.join("\n")),
        _ => String::new(),
    }
}

/// Zeile zum Render-Thread: läuft er, oder in welchem Schritt hängt er seit wann?
fn heartbeat() -> String {
    if !RUNNING.load(Ordering::Acquire) {
        return String::new();
    }
    let gap = now_ms().saturating_sub(BEAT_MS.load(Ordering::Acquire));
    let stage = STAGES
        .get(STAGE.load(Ordering::Relaxed) as usize)
        .copied()
        .unwrap_or("?");
    let loops = LOOPS.load(Ordering::Acquire);
    if gap > 3000 {
        format!(
            "\nRender-Thread HÄNGT seit {:.1} s in: {stage} ({loops} Durchläufe)",
            gap as f64 / 1000.
        )
    } else {
        format!("\nRender-Thread läuft: {loops} Durchläufe")
    }
}

fn report() -> String {
    let text = TEXT.lock().map(|t| t.clone()).unwrap_or_default();
    history() + &text + &heartbeat()
}

fn set_text(text: String) {
    if let Ok(mut t) = TEXT.lock() {
        *t = text;
    }
}

fn fail() {
    FAILED_MS.store(now_ms(), Ordering::Release);
    FAILED.store(true, Ordering::Release);
}

fn remember(line: String) {
    if let Ok(mut h) = HISTORY.lock() {
        h.push(line);
    }
}

/// Zeichnet, bis das Gerät verloren geht; erneuert den Bericht alle 15 Durchläufe.
///
/// Größenänderungen werden nur gemeldet, nicht umgesetzt: auf der Xbox schlug `ResizeBuffers` beim Neukonfigurieren
/// fehl, und wgpu hatte die Swapchain da schon verworfen (danach „Surface is not configured“). Die Last hat ohnehin eine
/// feste Größe; die Swapchain behält ihre Startgröße und wird von DXGI aufs Panel gestreckt.
fn render_loop(mut probe: Probe, attempt: u32) {
    let mut resizes = 0;
    LOOPS.store(0, Ordering::Release);
    loop {
        let r = RESIZE.swap(0, Ordering::AcqRel);
        if r != 0 {
            resizes += 1;
            if resizes <= 3 {
                let (w, h) = probe.surface_size();
                probe.info.push(format!(
                    "Fenster jetzt {}×{}: Swapchain bleibt {w}×{h} (Neukonfigurieren zerstörte auf der Xbox die Oberfläche)",
                    r >> 32,
                    r as u32
                ));
            }
        }
        let drawn_before = probe.frame_count();
        if let Err(e) = probe.frame()
            && probe.info.len() < 60
        {
            probe.info.push(format!("Bildfehler: {e:#}"));
        }
        let loops = LOOPS.fetch_add(1, Ordering::AcqRel) + 1;
        BEAT_MS.store(now_ms(), Ordering::Release);
        if loops == 1 || loops % 15 == 0 {
            set_text(probe.status());
        }
        if probe.lost() {
            let status = probe.status();
            let detail = status
                .lines()
                .filter(|l| l.contains("VERLOREN") || l.contains("DX12-Grund"))
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(" | ");
            remember(format!(
                "Versuch {} ({}): Gerät verloren nach {} Bildern – {detail}",
                attempt + 1,
                VARIANTS[attempt as usize].0,
                probe.frame_count()
            ));
            set_text(status);
            RUNNING.store(false, Ordering::Release);
            fail();
            // Nicht abräumen: beim Freigeben wartet wgpu auf die GPU und bricht nach einer Zeitgrenze mit einer Panik
            // ab; ein verlorenes Gerät liegen zu lassen kostet nur Speicher (höchstens zwei Versuche).
            std::mem::forget(probe);
            return;
        }
        if probe.frame_count() == drawn_before {
            // kein Bild bekommen (z. B. Occluded): nicht im Kreis drehen
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

/// Ein Versuch: Instanz, Oberfläche am Panel, Gerät und Last mit Variante `attempt`, dann Render-Thread. Nur vom
/// UI-Thread (das erste `configure` ruft `SetSwapChain`).
fn build(attempt: u32) -> bool {
    let (label, timestamps, div) = VARIANTS[attempt as usize];
    let size = SIZE.load(Ordering::Acquire);
    let (width, height) = (
        ((size >> 32) as u32 / div).max(1),
        (size as u32 / div).max(1),
    );
    let mut steps = BASE.lock().map(|b| b.clone()).unwrap_or_default();
    steps.push(format!(
        "Versuch {}: {label}, Swapchain {width}×{height}",
        attempt + 1
    ));
    checkpoint(&steps, "wgpu-Instanz (DX12) anlegen");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::DX12,
        backend_options: wgpu::BackendOptions {
            dx12: wgpu::Dx12BackendOptions {
                // FXC liegt im System, DXC müsste mitgeliefert werden
                shader_compiler: wgpu::Dx12Compiler::Fxc,
                ..Default::default()
            },
            ..Default::default()
        },
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    checkpoint(&steps, "DX12-Adapter aufzählen");
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::DX12));
    if adapters.is_empty() {
        steps.push("DX12-Adapter: KEINER".into());
    }
    for a in &adapters {
        let i = a.get_info();
        steps.push(format!("DX12-Adapter: {} ({:?})", i.name, i.device_type));
    }
    let panel = PANEL.load(Ordering::Acquire);
    let result = if panel.is_null() {
        Err(anyhow::anyhow!("kein SwapChainPanel übergeben"))
    } else {
        checkpoint(&steps, "Oberfläche am SwapChainPanel anlegen");
        match unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::SwapChainPanel(panel))
        } {
            Ok(surface) => {
                steps.push("Oberfläche am SwapChainPanel: angelegt".into());
                checkpoint(&steps, "Adapter, Gerät und Last aufbauen (Probe::new)");
                Probe::new(&instance, surface, width, height, steps.clone(), timestamps)
            }
            Err(e) => Err(anyhow::anyhow!("Oberfläche: {e}")),
        }
    };
    let probe = match result {
        Ok(p) => p,
        Err(e) => {
            remember(format!(
                "Versuch {} ({label}): Aufbau abgebrochen – {e:#}",
                attempt + 1
            ));
            set_text(steps.join("\n"));
            fail();
            return false;
        }
    };
    set_text(probe.status());
    BEAT_MS.store(now_ms(), Ordering::Release);
    RUNNING.store(true, Ordering::Release);
    if let Err(e) = std::thread::Builder::new()
        .name("probe-render".into())
        .spawn(move || render_loop(probe, attempt))
    {
        RUNNING.store(false, Ordering::Release);
        remember(format!("Versuch {}: Render-Thread: {e}", attempt + 1));
        fail();
        return false;
    }
    true
}

/// Start: `panel` = `ISwapChainPanelNative*` des XAML-`SwapChainPanel`, Größe in Bildpunkten, `log_dir` = nullterminierter
/// UTF-16-Pfad (darf null sein). Rückgabe 0 = Last läuft, negativ = erster Versuch gescheitert (Grund im Bericht;
/// `probe_tick` versucht weitere Varianten).
///
/// # Safety
/// `panel` muss ein gültiger `ISwapChainPanelNative`-Zeiger sein, der gültig bleibt, `log_dir` null oder
/// nullterminiert.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn probe_start(
    panel: *mut c_void,
    width: u32,
    height: u32,
    log_dir: *const u16,
) -> i32 {
    now_ms();
    let dir = DIR.get_or_init(|| wide(log_dir).map(PathBuf::from)).clone();
    if let Some(d) = dir.clone() {
        std::panic::set_hook(Box::new(move |info| {
            let _ = std::fs::write(d.join("probe-panic.txt"), info.to_string());
        }));
    }
    PANEL.store(panel, Ordering::Release);
    SIZE.store(((width as u64) << 32) | height as u64, Ordering::Release);
    let mut steps = vec![
        format!(
            "berlin-probe {} · Ziel {}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::ARCH
        ),
        format!("Fenster {width}×{height}"),
    ];
    for name in ["d3d12.dll", "dxgi.dll", "d3dcompiler_47.dll", "dcomp.dll"] {
        checkpoint(&steps, &format!("DLL {name} laden"));
        steps.push(dll(name));
    }
    if let Ok(mut b) = BASE.lock() {
        *b = steps;
    }
    if let Some(d) = dir {
        let _ = std::thread::Builder::new()
            .name("probe-watch".into())
            .spawn(move || {
                loop {
                    let _ = std::fs::write(d.join("probe-status.txt"), report());
                    std::thread::sleep(Duration::from_secs(2));
                }
            });
    }
    if build(0) { 0 } else { -1 }
}

/// Vom Timer der Hülle (UI-Thread): nach einem Geräteverlust und `RETRY_PAUSE_MS` Pause mit der nächsten Variante neu
/// aufbauen. Rückgabe: Nummer des laufenden Versuchs (1…), 0 = alle Varianten gescheitert.
#[unsafe(no_mangle)]
pub extern "C" fn probe_tick() -> u32 {
    if FAILED.load(Ordering::Acquire) {
        let next = ATTEMPT.load(Ordering::Acquire) + 1;
        if next as usize >= VARIANTS.len() {
            return 0;
        }
        if now_ms() < FAILED_MS.load(Ordering::Acquire) + RETRY_PAUSE_MS {
            return ATTEMPT.load(Ordering::Acquire) + 1;
        }
        ATTEMPT.store(next, Ordering::Release);
        FAILED.store(false, Ordering::Release);
        build(next);
    }
    ATTEMPT.load(Ordering::Acquire) + 1
}

/// Neue Fenstergröße in Bildpunkten; der Render-Thread meldet sie (siehe `render_loop`).
#[unsafe(no_mangle)]
pub extern "C" fn probe_resize(width: u32, height: u32) {
    if width > 0 && height > 0 {
        RESIZE.store(((width as u64) << 32) | height as u64, Ordering::Release);
    }
}

/// Bericht als UTF-16 in `buf` (höchstens `cap` Zeichen, ohne Nullzeichen); Rückgabe = geschriebene Zeichen.
/// Blockiert nie auf das Zeichnen.
///
/// # Safety
/// `buf` muss auf `cap` beschreibbare `u16` zeigen.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn probe_status(buf: *mut u16, cap: u32) -> u32 {
    let mut text = report();
    if text.is_empty() {
        text = "nicht gestartet".into();
    }
    let w: Vec<u16> = text.encode_utf16().take(cap as usize).collect();
    if !buf.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(w.as_ptr(), buf, w.len()) };
    }
    w.len() as u32
}
