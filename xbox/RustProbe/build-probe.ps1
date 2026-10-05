# Baut berlin_probe.dll und legt sie neben das UWP-Projekt (xbox\RustProbe\RustProbe\berlin_probe.dll).
#
#   .\build-probe.ps1            UWP-Ziel x86_64-uwp-windows-msvc (Nightly + build-std) – für die Xbox
#   .\build-probe.ps1 -Desktop   normales Windows-Ziel x86_64-pc-windows-msvc (stabiles Rust) – Rückfall, falls das
#                                UWP-Ziel nicht baut; läuft in der Hülle auf dem PC, auf der Xbox evtl. nicht
#
# Voraussetzungen: rustup, Visual Studio 2022 mit „Entwicklung für die universelle Windows-Plattform“ und
# „Desktopentwicklung mit C++“ (MSVC v143 x64/x86-Buildtools), Windows SDK 10.0.22621 oder neuer.
param([switch]$Desktop)
$ErrorActionPreference = 'Stop'
$root = Resolve-Path "$PSScriptRoot\..\.."
Push-Location $root
try {
    if ($Desktop) {
        rustup target add x86_64-pc-windows-msvc
        cargo build --release -p berlin-probe --lib --target x86_64-pc-windows-msvc
        $dll = "$root\target\x86_64-pc-windows-msvc\release\berlin_probe.dll"
    } else {
        # Tier-3-Ziel: kein vorgebautes std, also Nightly mit rust-src und build-std. panic=abort, weil nur
        # std + panic_abort neu gebaut werden.
        rustup toolchain install nightly --component rust-src --profile minimal
        $env:RUSTFLAGS = '-C panic=abort'
        cargo +nightly build -Z build-std=std,panic_abort --release -p berlin-probe --lib --target x86_64-uwp-windows-msvc
        $dll = "$root\target\x86_64-uwp-windows-msvc\release\berlin_probe.dll"
    }
    Copy-Item $dll "$PSScriptRoot\RustProbe\berlin_probe.dll" -Force
    Write-Host "berlin_probe.dll -> xbox\RustProbe\RustProbe\ (" (Get-Item $dll).Length "Bytes)"
    # Abhängigkeiten der DLL zeigen (welche System-DLLs sie importiert)
    $dumpbin = Get-ChildItem "${env:ProgramFiles}\Microsoft Visual Studio\2022\*\VC\Tools\MSVC\*\bin\Host*\x64\dumpbin.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($dumpbin) { & $dumpbin.FullName /dependents $dll | Select-String '\.dll' }
} finally {
    Pop-Location
}
