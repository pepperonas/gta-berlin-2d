//! C-Schnittstelle für die UWP-Hülle `xbox/RustProbe` (nur Windows). Alle Aufrufe kommen vom UI-Thread der Hülle
//! (`SetSwapChain` am `SwapChainPanel` verlangt ihn). Jeder Prüfschritt landet im Bericht (`probe_status`) und in
//! `probe-status.txt` im übergebenen Ordner (LocalState der App, über das Device Portal abholbar); eine Panik schreibt
//! `probe-panic.txt` dorthin.
use crate::Probe;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::Mutex;

struct State {
    probe: Option<Probe>,
    steps: Vec<String>,
    dir: Option<PathBuf>,
}
static STATE: Mutex<Option<State>> = Mutex::new(None);

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

fn report(state: &State) -> String {
    match &state.probe {
        Some(p) => p.status(),
        None => state.steps.join("\n"),
    }
}

/// Zwischenstand vor einem heiklen Aufruf: stürzt der Prozess darin ab, zeigt `probe-status.txt` die Stelle.
fn checkpoint(dir: &Option<PathBuf>, steps: &[String], next: &str) {
    if let Some(dir) = dir {
        let _ = std::fs::write(
            dir.join("probe-status.txt"),
            format!("{}\nnächster Schritt: {next}", steps.join("\n")),
        );
    }
}

fn write_status(state: &State) {
    if let Some(dir) = &state.dir {
        let _ = std::fs::write(dir.join("probe-status.txt"), report(state));
    }
}

/// Start: `panel` = `ISwapChainPanelNative*` des XAML-`SwapChainPanel`, Größe in Bildpunkten, `log_dir` = nullterminierter
/// UTF-16-Pfad (darf null sein). Rückgabe 0 = Last läuft, negativ = abgebrochen (Grund im Bericht).
///
/// # Safety
/// `panel` muss ein gültiger `ISwapChainPanelNative`-Zeiger sein, `log_dir` null oder nullterminiert.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn probe_start(
    panel: *mut c_void,
    width: u32,
    height: u32,
    log_dir: *const u16,
) -> i32 {
    let dir = wide(log_dir).map(PathBuf::from);
    if let Some(d) = dir.clone() {
        std::panic::set_hook(Box::new(move |info| {
            let _ = std::fs::write(d.join("probe-panic.txt"), info.to_string());
        }));
    }
    let mut steps = vec![
        format!(
            "berlin-probe {} · Ziel {}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::ARCH
        ),
        format!("Fenster {width}×{height}"),
    ];
    for name in ["d3d12.dll", "dxgi.dll", "d3dcompiler_47.dll", "dcomp.dll"] {
        checkpoint(&dir, &steps, &format!("DLL {name} laden"));
        steps.push(dll(name));
    }
    checkpoint(&dir, &steps, "wgpu-Instanz (DX12) anlegen");
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
    checkpoint(&dir, &steps, "DX12-Adapter aufzählen");
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::DX12));
    if adapters.is_empty() {
        steps.push("DX12-Adapter: KEINER".into());
    }
    for a in &adapters {
        let i = a.get_info();
        steps.push(format!("DX12-Adapter: {} ({:?})", i.name, i.device_type));
    }
    let result = if panel.is_null() {
        Err(anyhow::anyhow!("kein SwapChainPanel übergeben"))
    } else {
        checkpoint(&dir, &steps, "Oberfläche am SwapChainPanel anlegen");
        match unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::SwapChainPanel(panel))
        } {
            Ok(surface) => {
                steps.push("Oberfläche am SwapChainPanel: angelegt".into());
                checkpoint(&dir, &steps, "Adapter, Gerät und Last aufbauen (Probe::new)");
                Probe::new(&instance, surface, width, height, steps.clone())
            }
            Err(e) => Err(anyhow::anyhow!("Oberfläche: {e}")),
        }
    };
    let (probe, code) = match result {
        Ok(p) => (Some(p), 0),
        Err(e) => {
            steps.push(format!("ABBRUCH: {e:#}"));
            (None, -1)
        }
    };
    let state = State { probe, steps, dir };
    write_status(&state);
    if let Ok(mut s) = STATE.lock() {
        *s = Some(state);
    }
    code
}

/// Ein Bild zeichnen (aus `CompositionTarget.Rendering`). 0 = gut, -1 = keine Probe, -2 = Fehler (im Bericht).
#[unsafe(no_mangle)]
pub extern "C" fn probe_frame() -> i32 {
    let Ok(mut guard) = STATE.lock() else {
        return -1;
    };
    let Some(state) = guard.as_mut() else {
        return -1;
    };
    let Some(probe) = state.probe.as_mut() else {
        return -1;
    };
    let code = match probe.frame() {
        Ok(()) => 0,
        Err(e) => {
            probe.info.push(format!("Bildfehler: {e:#}"));
            -2
        }
    };
    if probe.frame_count() % 120 == 0 {
        write_status(state);
    }
    code
}

#[unsafe(no_mangle)]
pub extern "C" fn probe_resize(width: u32, height: u32) {
    if let Ok(mut g) = STATE.lock()
        && let Some(p) = g.as_mut().and_then(|s| s.probe.as_mut())
    {
        p.resize(width, height);
    }
}

/// Bericht als UTF-16 in `buf` (höchstens `cap` Zeichen, ohne Nullzeichen); Rückgabe = geschriebene Zeichen.
///
/// # Safety
/// `buf` muss auf `cap` beschreibbare `u16` zeigen.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn probe_status(buf: *mut u16, cap: u32) -> u32 {
    let text = match STATE.lock() {
        Ok(g) => g
            .as_ref()
            .map(report)
            .unwrap_or_else(|| "nicht gestartet".into()),
        Err(_) => "Zustand gesperrt".into(),
    };
    let w: Vec<u16> = text.encode_utf16().take(cap as usize).collect();
    if !buf.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(w.as_ptr(), buf, w.len()) };
    }
    w.len() as u32
}
