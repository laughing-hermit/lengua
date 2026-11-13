use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use lengua_idea::lock::{self, Facts};
use wasmtime::Config;
use wasmtime::component::Component;

pub(crate) const SEALED: &str = "target/lengua/sealed";
const KEPT: usize = 32;

#[derive(Clone)]
pub(crate) struct Host {
    inner: lengua_host::Host,
    components: Arc<Mutex<HashMap<String, Component>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Seal {
    pub sealed: String,
    pub lock: PathBuf,
}

fn lock_of<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn config() -> Config {
    let mut config = lengua_host::config();
    config.cranelift_nan_canonicalization(true);
    config
}

pub(crate) fn outside(imports: &[String], world: &[String]) -> Vec<String> {
    imports
        .iter()
        .filter(|name| {
            let (interface, version) = name.split_once('@').unwrap_or((name.as_str(), ""));
            let known = world.iter().any(|w| w == interface);
            let exact = world.iter().any(|w| w == *name);
            !(exact || (known && (version == "0.2" || version.starts_with("0.2."))))
        })
        .cloned()
        .collect()
}

pub(crate) fn grant_for(import: &str) -> Option<&'static str> {
    let interface = import.split('@').next().unwrap_or(import);
    if interface == "wasi:clocks/wall-clock" {
        Some("`- Reads: the clock`")
    } else if interface.starts_with("wasi:random/") {
        Some("`- Reads: random numbers`")
    } else if interface.starts_with("wasi:filesystem/") {
        Some("``- Reads: files in `<folder>` `` or ``- Writes: files in `<folder>` ``")
    } else if interface.starts_with("wasi:sockets/") {
        Some("``- Connects to: `<host>:<port>` ``")
    } else {
        None
    }
}

impl Host {
    pub(crate) fn new() -> Result<Self, String> {
        Ok(Self {
            inner: lengua_host::Host::new(&config())?,
            components: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub(crate) fn engine(&self) -> &wasmtime::Engine {
        self.inner.engine()
    }

    pub(crate) fn component(&self, bytes: &[u8], sha256: &str) -> Result<Component, String> {
        if let Some(found) = lock_of(&self.components).get(sha256) {
            return Ok(found.clone());
        }
        let component = Component::new(self.engine(), bytes).map_err(|e| format!("{e:#}"))?;
        let mut kept = lock_of(&self.components);
        if kept.len() >= KEPT {
            kept.clear();
        }
        kept.insert(sha256.to_string(), component.clone());
        Ok(component)
    }

    pub(crate) fn imports(&self, component: &Component) -> Vec<String> {
        component
            .component_type()
            .imports(self.engine())
            .map(|(name, _)| name.to_string())
            .collect()
    }

    pub(crate) fn seal_code(
        &self,
        folder: &Path,
        bytes: &[u8],
        sha256: &str,
    ) -> Result<String, String> {
        let component = self.component(bytes, sha256)?;
        let code = component
            .serialize()
            .map_err(|e| format!("wasmtime could not precompile the component: {e:#}"))?;
        let sealed = format!("{SEALED}/{sha256}.cwasm");
        let path = folder.join(&sealed);
        let fail = |e: std::io::Error| format!("lengua could not write `{sealed}`: {e}");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(fail)?;
        }
        fs::write(&path, code).map_err(fail)?;
        Ok(sealed)
    }

    pub(crate) fn seal(
        &self,
        folder: &Path,
        bytes: &[u8],
        sha256: &str,
        trusted: &[String],
    ) -> Result<Seal, String> {
        let sealed = self.seal_code(folder, bytes, sha256)?;
        let key = lock::user_key()?;
        let lock = folder.join(format!("{SEALED}/{sha256}.lock"));
        lock::write(folder, &lock, &Facts::test(&sealed, trusted), &key)?;
        Ok(Seal { sealed, lock })
    }

    pub(crate) fn exports(&self, component: &Component) -> Vec<String> {
        lengua_host::exported_interfaces(self.engine(), component)
    }
}

pub(crate) fn clear(folder: &Path) {
    let _ = fs::remove_dir_all(folder.join(SEALED));
}

pub(crate) fn unseal(folder: &Path, seal: &Seal) {
    let _ = fs::remove_file(folder.join(&seal.sealed));
    let _ = fs::remove_file(&seal.lock);
}

#[cfg(test)]
mod tests {
    use super::{Host, grant_for, outside};

    const HEADER: [u8; 8] = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];

    fn module(sections: &[u8]) -> Vec<u8> {
        let mut bytes = HEADER.to_vec();
        bytes.extend_from_slice(sections);
        bytes
    }

    #[test]
    fn the_engine_refuses_relaxed_simd() {
        let relaxed = module(&[
            0x01, 0x07, 0x01, 0x60, 0x02, 0x7b, 0x7b, 0x01, 0x7b, 0x03, 0x02, 0x01, 0x00, 0x0a,
            0x0b, 0x01, 0x09, 0x00, 0x20, 0x00, 0x20, 0x01, 0xfd, 0x80, 0x02, 0x0b,
        ]);
        let plain = wasmtime::Engine::default();
        assert!(wasmtime::Module::new(&plain, &relaxed).is_ok());
        let host = Host::new().expect("a host");
        let Err(refused) = wasmtime::Module::new(host.engine(), &relaxed) else {
            panic!("relaxed SIMD was taken");
        };
        assert!(format!("{refused:#}").contains("relaxed"), "{refused:#}");
    }

    #[test]
    fn the_engine_refuses_shared_memory() {
        let host = Host::new().expect("a host");
        let unshared = module(&[0x05, 0x04, 0x01, 0x01, 0x01, 0x01]);
        assert!(wasmtime::Module::new(host.engine(), &unshared).is_ok());
        let shared = module(&[0x05, 0x04, 0x01, 0x03, 0x01, 0x01]);
        let Err(refused) = wasmtime::Module::new(host.engine(), &shared) else {
            panic!("shared memory was taken");
        };
        let message = format!("{refused:#}").to_lowercase();
        assert!(
            message.contains("thread") || message.contains("shared"),
            "{message}"
        );
    }

    #[test]
    fn imports_outside_the_world_are_named_and_any_0_2_version_is_taken() {
        let world = vec![
            "wasi:cli/stdout".to_string(),
            "wasi:io/streams".to_string(),
            "lengua:tax/rates".to_string(),
        ];
        let imports = vec![
            "wasi:cli/stdout@0.2.12".to_string(),
            "wasi:io/streams@0.2.0".to_string(),
            "wasi:random/random@0.2.12".to_string(),
            "wasi:cli/stdout@0.3.0".to_string(),
            "example:thing/other".to_string(),
            "lengua:tax/rates".to_string(),
        ];
        assert_eq!(
            outside(&imports, &world),
            [
                "wasi:random/random@0.2.12",
                "wasi:cli/stdout@0.3.0",
                "example:thing/other"
            ]
        );
        assert_eq!(
            grant_for("wasi:random/insecure@0.2.12"),
            Some("`- Reads: random numbers`")
        );
        assert!(grant_for("wasi:filesystem/types@0.2.12").is_some());
        assert_eq!(grant_for("example:thing/other"), None);
    }
}
