use std::{
    fs::File,
    io::Write,
    sync::{Mutex, OnceLock},
};
static LOG: OnceLock<Mutex<File>> = OnceLock::new();
pub fn initialize(store: &crate::preferences::Store) {
    if let Ok(f) = store.log() {
        let _ = LOG.set(Mutex::new(f));
    }
}
pub fn record(message: &str) {
    eprintln!("[SW {}] {message}", env!("CARGO_PKG_VERSION"));
    if let Some(mut f) = LOG.get().and_then(|m| m.lock().ok()) {
        let _ = writeln!(f, "[{}] {message}", env!("CARGO_PKG_VERSION"));
    }
}
