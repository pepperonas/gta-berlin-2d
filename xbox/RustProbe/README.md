# Rust-Probe für die Xbox (Machbarkeitsbeweis)

Prüft, ob die Rust-Fassung als UWP-App im Xbox Developer Mode zeichnen kann, und misst eine feste Grafiklast, die
dem Szenendurchgang des Spiels nachgebildet ist (Hintergrund und Ergebnisse: `docs/NATIVE-RUST.md`, „Recherche zu
Schritt 3“ und „Ergebnis auf der Konsole“).

- `crates/xbox_probe` – Rust: Prüfschritte (DLLs laden, DX12-Adapter, Oberfläche am `SwapChainPanel`, Gerät, Prüfpunkte
  im Aufbau) und Last (8 Vollbild-Schichten in 2560 × 1440, drei Betriebsarten: Float mit 4× MSAA, Float ohne, 8 Bit
  ohne). Gezeichnet wird in einem eigenen Render-Thread.
- `xbox/RustProbe` – UWP-Hülle (C#, XAML): holt `ISwapChainPanelNative`, ruft `probe_start`, zeigt per Timer den Bericht
  und stößt nach einem Geräteverlust den Neuaufbau an (`probe_tick`).

**Stand 06.10.2026:** läuft auf der Series X (Hardware-GPU, siehe `docs/NATIVE-RUST.md`). Gemessen bisher nur im
App-Modus; der Spielmodus ließ sich auf der Konsole nicht einstellen (siehe unten).

## Vergleichswerte vom Mac

```bash
cargo run --release -p berlin-probe --example mac                  # GPU-Zeitstempel
PROBE_FENCE=1 cargo run --release -p berlin-probe --example mac    # wie auf der Xbox (Fence), für den Vergleich
```

M1 Pro (05.10.2026, Zeitstempel): Float 4× MSAA 4,46 ms · Float ohne MSAA 5,34 ms · 8 Bit 4,95 ms (GPU-Median).

## In der Windows-VM bauen

1. Visual Studio 2022 mit „Entwicklung für die universelle Windows-Plattform“, MSVC v143 **x64 und ARM64** (in einer
   ARM-VM laufen Rusts Buildskripte auf dem ARM64-Host), C++-UWP-Unterstützung, Windows SDK 10.0.22621+; rustup, Git.
   Windows-Entwicklermodus einschalten (zum lokalen Registrieren).
2. DLL bauen: `xbox\RustProbe\build-probe.ps1`. Unter Windows PowerShell 5.1 bricht das Skript ab, sobald rustup etwas auf
   stderr schreibt (`$ErrorActionPreference = 'Stop'`); dann die beiden Befehle von Hand:
   ```powershell
   rustup toolchain install nightly --component rust-src --profile minimal
   $env:RUSTFLAGS = '-C panic=abort'
   cargo +nightly build -Z build-std=std,panic_abort --release -p berlin-probe --lib --target x86_64-uwp-windows-msvc
   copy target\x86_64-uwp-windows-msvc\release\berlin_probe.dll xbox\RustProbe\RustProbe\
   ```
3. Hülle bauen: **Release | x64** (in der ARM-VM stürzt Debug beim Start ab: x64-CoreCLR unter Emulation; Release nutzt
   .NET Native). Signiert mit dem eigenen Zertifikat (Publisher im Manifest = `CN=GtaBerlinDev`):
   ```powershell
   MSBuild xbox\RustProbe\RustProbe.sln /restore /p:Configuration=Release /p:Platform=x64 `
     /p:AppxPackageSigningEnabled=true /p:PackageCertificateThumbprint=<Fingerabdruck aus Cert:\CurrentUser\My>
   ```
   Paket: `RustProbe\AppPackages\RustProbe_<Version>_x64_Test\` (`.msix`, `.cer`, `Dependencies\x64\`).
4. Lokal testen: `Add-AppxPackage -Register RustProbe\bin\x64\Release\ilc\AppxManifest.xml` (Abhängigkeiten aus
   `Dependencies\x64` vorher mit `Add-AppxPackage` installieren). In der VM gibt es nur WARP (~1 s je Bild): prüft die Kette,
   nicht die Leistung.

Bei jeder neuen Fassung die Paketversion im Manifest erhöhen, sonst lehnt das Device Portal die Installation ab.

## Auf der Xbox

1. Device Portal öffnen (`https://<xbox-ip>:11443`, IP in Dev Home), alte Fassung deinstallieren.
2. *Add* → `.msix`, bei den Abhängigkeiten alle drei Pakete aus `Dependencies\x64` (VCLibs, .NET Native Framework und
   Runtime 2.2) und gegebenenfalls die `.cer`.
3. Spielmodus: Die bekannten Anleitungen (Dev Home → Ansichtstaste → *View details* → *App type: Game*) passen auf
   OS 10.0.26100.9438 nicht, der Eintrag fehlt. Noch nicht probiert: Device Portal *Settings → Preference Settings*
   „Treat UWP apps as games by default“. Als App bekommt die Probe höchstens 45 % der GPU.
4. Probe starten, ~70 s warten (je Betriebsart 360 Bilder), dann *File Explorer → LocalAppData → GtaBerlin.RustProbe_… →
   LocalState* `probe-status.txt` holen (alle 2 s neu geschrieben, auch wenn das Zeichnen hängt). Daneben gegebenenfalls
   `shell-status.txt` (Meldungen der Hülle), `shell-error.txt` (Ausnahmen der Hülle), `probe-panic.txt` (Panik in Rust).

## Was der Bericht beantwortet

| Zeile | Frage |
|---|---|
| `DLL …: geladen/FEHLER` | Darf die App im Sandkasten `d3d12.dll`, `dxgi.dll`, `d3dcompiler_47.dll`, `dcomp.dll` laden? |
| `DX12-Adapter` | Hardware-GPU (`SraKmd_arden` auf der Series X) oder nur WARP? |
| `Aufbau …: Gerät ok / GERÄT VERLOREN` | Nach welchem Schritt (Gerät, Oberfläche, Rauschtextur, Pipelines, Ausgabe) ging das Gerät verloren, mit wgpu-Meldung und DX12-Grund (`GetDeviceRemovedReason`)? |
| `Fence Median …` | Lastzeit: Last einzeln abgeschickt, Zeit bis die GPU fertig ist (GPU-Zeitstempel sind aus, sie warfen auf der Konsole den Treiber um). |
| `Oberfläche ohne Bild: N× (zuletzt …)` | Lieferte die Swapchain keine Bilder, und warum (`Validation` ohne wgpu-Fehler = Gerät verloren)? |
| `Fenster jetzt …: Swapchain bleibt …` | Größenänderungen werden nur gemeldet (`ResizeBuffers` zerstörte auf der Konsole die Oberfläche). |
| `Render-Thread läuft / HÄNGT seit … in: …` | Zeichnet die Probe, oder in welchem Schritt hängt sie? |
| `Frühere Versuche` | Nach einem Geräteverlust baut die Probe nach 10 s Pause einmal neu auf; hier stehen die gescheiterten Versuche. |
| `wgpu-Fehler` | Validierungsfehler (Shader, Formate, Oberfläche). |
