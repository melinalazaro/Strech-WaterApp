//! Project-owned persistence. Anchor pathname operations at directory descriptors:
//! O_NOFOLLOW applies at every component, including lock, log and backup.
use crate::scheduler::{ReminderInterval, Settings};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::path::Path;
use std::{
    ffi::CString,
    fs::File,
    io::{self, Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    os::unix::fs::MetadataExt,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
const LIMIT: u64 = 64 * 1024;
pub const TEST: bool = cfg!(feature = "test-profile");
pub fn profile() -> &'static str {
    if TEST {
        "test"
    } else {
        "normal"
    }
}
pub fn app_name() -> &'static str {
    if TEST {
        "Stretch & Water PRUEBA.app"
    } else {
        "Stretch & Water.app"
    }
}
fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub schema: u8,
    #[serde(default = "sound_on_by_default")]
    pub sound_enabled: bool,
    pub stretch_minutes: u32,
    #[serde(default = "default_water_minutes")]
    pub water_minutes: u32,
    pub stretch_enabled: bool,
    pub water_enabled: bool,
    pub indefinite: bool,
    pub login_requested: bool,
    pub login_observed: crate::login::State,
}
fn sound_on_by_default() -> bool {
    true
}
fn default_water_minutes() -> u32 {
    60
}
impl Default for Config {
    fn default() -> Self {
        Self {
            schema: 1,
            sound_enabled: sound_on_by_default(),
            stretch_minutes: 30,
            water_minutes: default_water_minutes(),
            stretch_enabled: true,
            water_enabled: true,
            indefinite: false,
            login_requested: !TEST,
            login_observed: crate::login::State::Unknown,
        }
    }
}
impl Config {
    pub fn settings(&self) -> Settings {
        Settings {
            stretch_interval: ReminderInterval::try_from(self.stretch_minutes)
                .unwrap_or(ReminderInterval::Thirty),
            water_interval: ReminderInterval::try_from(self.water_minutes)
                .unwrap_or(ReminderInterval::Sixty),
            stretch_enabled: self.stretch_enabled,
            water_enabled: self.water_enabled,
        }
    }
    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let c: Self = serde_json::from_slice(bytes)
            .map_err(|_| bad("Preferencias inválidas; se conserva el archivo."))?;
        if c.schema != 1
            || ReminderInterval::try_from(c.stretch_minutes).is_err()
            || ReminderInterval::try_from(c.water_minutes).is_err()
        {
            return Err(bad(
                "Esquema o intervalo no admitido; se conserva el archivo.",
            ));
        }
        Ok(c)
    }
}
fn component(s: &str) -> io::Result<CString> {
    if s.is_empty() || s == "." || s == ".." || s.contains('/') {
        return Err(bad("Ruta no permitida"));
    }
    CString::new(s).map_err(|_| bad("Ruta inválida"))
}
fn open_at(dir: &File, name: &str, flags: i32) -> io::Result<File> {
    let n = component(name)?;
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            n.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            0o600 as libc::c_uint,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let f = unsafe { File::from_raw_fd(fd) };
    let m = f.metadata()?;
    if flags & libc::O_DIRECTORY == 0 && (!m.is_file() || m.nlink() != 1) {
        return Err(bad("Se requiere un archivo propio, regular y sin enlaces."));
    }
    Ok(f)
}
fn sub(dir: &File, name: &str) -> io::Result<File> {
    match open_at(dir, name, libc::O_RDONLY | libc::O_DIRECTORY) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let n = component(name)?;
            if unsafe { libc::mkdirat(dir.as_raw_fd(), n.as_ptr(), 0o700) } < 0 {
                let e = io::Error::last_os_error();
                if e.kind() != io::ErrorKind::AlreadyExists {
                    return Err(e);
                }
            }
            open_at(dir, name, libc::O_RDONLY | libc::O_DIRECTORY)
        }
        other => other,
    }
}
#[derive(Clone)]
pub struct Store {
    root: Arc<File>,
    profile: &'static str,
}
pub enum Opened {
    Owner(Store, File),
    AlreadyRunning,
}
impl Store {
    pub fn bundle() -> io::Result<Opened> {
        let exe = std::env::current_exe()?;
        let project = exe
            .ancestors()
            .nth(4)
            .ok_or_else(|| bad("Ubicación de la aplicación inválida"))?;
        if exe
            != project
                .join(app_name())
                .join("Contents/MacOS/stretch-and-water-poc")
        {
            return Err(bad("Abrí la aplicación con su nombre original dentro de la carpeta completa de Stretch & Water."));
        }
        let root = File::open(project)?;
        let expected = include_str!("../../.stretch-water-project");
        let mut marker = String::new();
        open_at(&root, ".stretch-water-project", libc::O_RDONLY)?
            .take(expected.len() as u64 + 1)
            .read_to_string(&mut marker)?;
        if marker != expected {
            return Err(bad(
                "Falta el marcador de la entrega. Volvé a descomprimir la carpeta completa.",
            ));
        }
        Self::acquire_at(root, profile())
    }
    #[cfg(test)]
    pub(crate) fn acquire(root: &Path, profile: &'static str) -> io::Result<Opened> {
        Self::acquire_at(File::open(root)?, profile)
    }
    fn acquire_at(root: File, profile: &'static str) -> io::Result<Opened> {
        let name = format!(".stretch-water-{profile}.lock");
        let lock = match open_at(&root, &name, libc::O_RDONLY) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                open_at(&root, &name, libc::O_CREAT | libc::O_RDONLY)?
            }
            x => x?,
        };
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } < 0 {
            let e = io::Error::last_os_error();
            return if e.kind() == io::ErrorKind::WouldBlock {
                Ok(Opened::AlreadyRunning)
            } else {
                Err(e)
            };
        }
        Ok(Opened::Owner(
            Self {
                root: Arc::new(root),
                profile,
            },
            lock,
        ))
    }
    fn dir(&self) -> io::Result<File> {
        sub(&sub(&self.root, "data")?, self.profile)
    }
    pub fn log(&self) -> io::Result<File> {
        open_at(
            &self.dir()?,
            "events.log",
            libc::O_CREAT | libc::O_APPEND | libc::O_WRONLY,
        )
    }
    fn existing(dir: &File) -> io::Result<Option<Vec<u8>>> {
        let f = match open_at(dir, "preferences.json", libc::O_RDONLY) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let mut bytes = vec![];
        f.take(LIMIT + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > LIMIT {
            return Err(bad("Archivo de preferencias demasiado grande."));
        }
        Config::decode(&bytes)?;
        Ok(Some(bytes))
    }
    pub fn load(&self) -> io::Result<Option<Config>> {
        Self::existing(&self.dir()?)?
            .map(|bytes| Config::decode(&bytes))
            .transpose()
    }
    pub fn save(&self, cfg: &Config) -> io::Result<()> {
        let dir = self.dir()?;
        let previous = Self::existing(&dir)?;
        let bytes = serde_json::to_vec_pretty(cfg)?;
        Config::decode(&bytes)?;
        if let Some(prev) = previous {
            if prev == bytes {
                return Ok(());
            }
            atomic(&dir, "preferences.previous.json", &prev)?;
        }
        atomic(&dir, "preferences.json", &bytes)
    }
}
static NONCE: AtomicU64 = AtomicU64::new(1);
fn atomic(dir: &File, dest: &str, bytes: &[u8]) -> io::Result<()> {
    let (tmp, mut f) = loop {
        let n = format!(
            ".write-{}-{}",
            std::process::id(),
            NONCE.fetch_add(1, Ordering::Relaxed)
        );
        match open_at(dir, &n, libc::O_CREAT | libc::O_EXCL | libc::O_WRONLY) {
            Ok(f) => break (n, f),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    };
    let name = component(&tmp)?;
    let target = component(dest)?;
    let result = (|| {
        f.write_all(bytes)?;
        f.sync_all()?;
        if unsafe {
            libc::renameat(
                dir.as_raw_fd(),
                name.as_ptr(),
                dir.as_raw_fd(),
                target.as_ptr(),
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        dir.sync_all()
    })();
    if result.is_err() {
        unsafe { libc::unlinkat(dir.as_raw_fd(), name.as_ptr(), 0) };
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::symlink};
    #[test]
    fn existing_v030_settings_keep_every_choice_and_gain_enabled_sound() {
        let old = br#"{"schema":1,"stretch_minutes":60,"stretch_enabled":false,"water_enabled":true,"indefinite":true,"login_requested":false,"login_observed":"disabled"}"#;
        let cfg = Config::decode(old).unwrap();
        assert_eq!(
            cfg,
            Config {
                schema: 1,
                stretch_minutes: 60,
                water_minutes: 60,
                stretch_enabled: false,
                water_enabled: true,
                indefinite: true,
                login_requested: false,
                login_observed: crate::login::State::Disabled,
                sound_enabled: true,
            }
        );
        let mut broken: serde_json::Value = serde_json::from_slice(old).unwrap();
        broken["sound_enabled"] = serde_json::json!("no");
        assert!(Config::decode(&serde_json::to_vec(&broken).unwrap()).is_err());
    }
    #[test]
    fn existing_v050_settings_keep_sound_and_default_water_to_sixty() {
        let old = br#"{"schema":1,"sound_enabled":false,"stretch_minutes":60,"stretch_enabled":false,"water_enabled":true,"indefinite":true,"login_requested":false,"login_observed":"disabled"}"#;
        let cfg = Config::decode(old).unwrap();
        assert_eq!(cfg.water_minutes, 60);
        assert!(!cfg.sound_enabled && !cfg.stretch_enabled && cfg.water_enabled && cfg.indefinite);
        assert!(!cfg.login_requested);
        assert_eq!(cfg.settings().stretch_interval, ReminderInterval::Sixty);
        assert_eq!(cfg.settings().water_interval, ReminderInterval::Sixty);
        for field in ["stretch_minutes", "water_minutes"] {
            for invalid in [
                serde_json::json!(0),
                serde_json::json!(90),
                serde_json::json!(121),
                serde_json::json!(-1),
                serde_json::json!("120"),
                serde_json::Value::Null,
            ] {
                let mut value = serde_json::to_value(&cfg).unwrap();
                value[field] = invalid;
                assert!(Config::decode(&serde_json::to_vec(&value).unwrap()).is_err());
            }
        }
    }
    #[test]
    fn isolate_lock_recover_and_preserve_invalid_bytes() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".development")
            .join(format!(
                "store-test-{}-{}",
                std::process::id(),
                NONCE.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&p).unwrap();
        let Opened::Owner(store, lock) = Store::acquire(&p, "test").unwrap() else {
            panic!()
        };
        assert!(matches!(
            Store::acquire(&p, "test").unwrap(),
            Opened::AlreadyRunning
        ));
        let Opened::Owner(other, _) = Store::acquire(&p, "normal").unwrap() else {
            panic!()
        };
        assert!(store.load().unwrap().is_none());
        let mut cfg = Config {
            indefinite: true,
            ..Config::default()
        };
        store.save(&cfg).unwrap();
        cfg.stretch_minutes = 60;
        cfg.sound_enabled = false;
        store.save(&cfg).unwrap();
        assert_eq!(store.load().unwrap(), Some(cfg.clone()));
        assert!(other.load().unwrap().is_none());
        other.save(&Config::default()).unwrap();
        assert!(other.load().unwrap().unwrap().sound_enabled);
        assert!(!store.load().unwrap().unwrap().sound_enabled);
        let file = p.join("data/test/preferences.json");
        let mut future = cfg.clone();
        future.schema = 2;
        let invalid = serde_json::to_vec(&future).unwrap();
        fs::write(&file, &invalid).unwrap();
        assert!(store.load().is_err() && store.save(&cfg).is_err());
        assert_eq!(fs::read(&file).unwrap(), invalid);
        fs::remove_file(&file).unwrap();
        symlink(p.join("private"), &file).unwrap();
        assert!(store.save(&cfg).is_err());
        assert!(!p.join("private").exists());
        fs::remove_file(file).unwrap();
        let data = p.join("data");
        fs::rename(&data, p.join("original")).unwrap();
        symlink(p.join("original"), &data).unwrap();
        assert!(store.load().is_err());
        drop(lock);
        assert!(matches!(
            Store::acquire(&p, "test").unwrap(),
            Opened::Owner(..)
        ));
        fs::remove_dir_all(p).unwrap();
    }
}
