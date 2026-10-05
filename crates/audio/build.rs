//! Dateiliste der Klang-Samples (`data/audio/sfx/*.wav`, gebaut von tools/audio/build_sfx.py) als `include_bytes!`.
//! So muss niemand eine Liste von Hand pflegen; ein neuer Lauf des Build-Skripts genügt.
use std::fmt::Write;
use std::path::Path;

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/audio/sfx");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".wav"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let mut out = String::from(
        "/// (Dateiname ohne Endung, Inhalt) – erzeugt von build.rs\npub static SFX_FILES: &[(&str, &[u8])] = &[\n",
    );
    for f in &files {
        println!("cargo:rerun-if-changed={}", dir.join(f).display());
        let stem = f.trim_end_matches(".wav");
        let path = dir.join(f).canonicalize().unwrap();
        writeln!(
            out,
            "    ({stem:?}, include_bytes!({:?})),",
            path.display().to_string()
        )
        .unwrap();
    }
    out.push_str("];\n");
    let dest = Path::new(&std::env::var("OUT_DIR").unwrap()).join("sfx_files.rs");
    std::fs::write(dest, out).unwrap();
}
