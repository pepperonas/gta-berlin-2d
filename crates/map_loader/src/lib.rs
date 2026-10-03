pub mod buildcolors;
pub mod citycodes;
pub mod format;
pub mod geom;
pub mod mesh;
pub mod projection;
pub mod roofs;
pub mod stream;
use std::path::PathBuf;
pub fn default_data_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/data/berlin")
}
