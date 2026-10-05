# Rust-Probe für die Xbox (Machbarkeitsbeweis)

Prüft, ob die Rust-Fassung als UWP-Spiel im Xbox Developer Mode zeichnen kann, und misst eine feste Grafiklast, die
dem Szenendurchgang des Spiels nachgebildet ist (Hintergrund: `docs/NATIVE-RUST.md`, „Recherche zu Schritt 3“).

- `crates/xbox_probe` – Rust: Prüfschritte (DLLs laden, DX12-Adapter, Oberfläche am `SwapChainPanel`, Gerät) und
  Last (8 Vollbild-Schichten in 2560 × 1440, drei Betriebsarten: Float mit 4× MSAA, Float ohne, 8 Bit ohne).
- `xbox/RustProbe` – UWP-Hülle (C#, XAML): `SwapChainPanel`, ruft `berlin_probe.dll` je Bild, zeigt den Bericht.

## Vergleichswerte vom Mac

```bash
cargo run --release -p berlin-probe --example mac
```

M1 Pro (05.10.2026): Float 4× MSAA 4,46 ms · Float ohne MSAA 5,34 ms · 8 Bit 4,95 ms (GPU-Median).

## In der Windows-VM bauen

1. Visual Studio 2022 (Workloads „Entwicklung für die universelle Windows-Plattform“ und „Desktopentwicklung mit C++“,
   dazu MSVC v143 **x64/x86**-Buildtools und Windows SDK 10.0.22621+), rustup, Git.
2. Repo klonen, in PowerShell: `xbox\RustProbe\build-probe.ps1` (UWP-Ziel, Nightly). Scheitert das, zum Vergleich
   `build-probe.ps1 -Desktop`.
3. `xbox\RustProbe\RustProbe.sln` öffnen, **Debug | x64**, Zertifikat wählen (Package.appxmanifest → Packaging),
   zuerst auf dem Rechner selbst starten (**Local Machine**): in der VM gibt es kein echtes DX12 – der Bericht sollte
   dann den Software-Adapter (WARP/Basic Render Driver) zeigen; das prüft die ganze Kette bis auf die Konsole.

## Auf der Xbox

1. Konsole im Developer Mode, Dev Home zeigt die IP. In Visual Studio: Ziel **Remote Machine**, IP eintragen, Pairing-PIN
   aus Dev Home (*Pair with Visual Studio*). Alternativ: *Project → Publish → Create App Packages* (Sideloading, x64)
   und das `.msix` im Device Portal (`https://<xbox-ip>:11443`) installieren.
2. **Spielmodus einschalten** (sonst nur Software-DX12): im Device Portal *Settings → Preference Settings* „Treat UWP
   apps as games by default“ oder in Dev Home bei der App *View details → App type: Game*.
3. Probe starten, Bericht ablesen; nach allen drei Betriebsarten (~20 s) steht er vollständig da und liegt auch in
   *LocalState\probe-status.txt* (Device Portal → File Explorer). Bei einem Absturz: *probe-panic.txt* im selben Ordner.

## Was der Bericht beantwortet

| Zeile | Frage |
|---|---|
| `DLL …: geladen/FEHLER` | Darf die App im Sandkasten `d3d12.dll`, `dxgi.dll`, `d3dcompiler_47.dll`, `dcomp.dll` laden (wgpu tut das per `LoadLibraryExW`)? |
| `DX12-Adapter` | Hardware-GPU oder nur WARP (dann ist der Spielmodus aus)? |
| `Oberfläche …`, `Adapter …` | Nimmt wgpu das `SwapChainPanel` an, welches Format, gibt es Zeitstempel? |
| `GPU Median …` | Lastzeiten – durch die Mac-Werte geteilt ergibt das den Faktor für die Spielmessungen. |
| `wgpu-Fehler` | Validierungsfehler (Shader, Formate). |
