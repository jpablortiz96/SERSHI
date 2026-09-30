//! Local storage for models: speech recognition (`models\stt`) and the
//! semantic model (`models\semantic`).
//!
//! Models live in SERSHI's per-user app-data directory
//! (`%LOCALAPPDATA%\dev.sershi.desktop\models\…` on Windows; the desktop
//! shell supplies the path). They are data, never executed.
//!
//! Installing is fail-closed:
//!
//! ```text
//! source (HTTPS) → <name>.partial  (never larger than the expected size)
//!   → exact size + SHA-256 match → fsync → atomic rename to <name>
//! ```
//!
//! Anything else — a short or long file, a hash mismatch, a cancelled or
//! failed transfer — deletes the partial file and leaves no model behind.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use sershi_core::understanding::models::SemanticModel;
use sershi_core::voice::{ModelError, ModelState, SttModel};
use sha2::{Digest, Sha256};

/// What the store needs to know about a model file: all fixed by SERSHI's
/// catalogs, never by input.
pub trait ModelFile {
    fn file_name(&self) -> &str;
    fn size_bytes(&self) -> u64;
    fn sha256(&self) -> &str;
}

impl ModelFile for SttModel {
    fn file_name(&self) -> &str {
        self.file_name
    }
    fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
    fn sha256(&self) -> &str {
        self.sha256
    }
}

impl ModelFile for SemanticModel {
    fn file_name(&self) -> &str {
        self.file_name
    }
    fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
    fn sha256(&self) -> &str {
        self.sha256
    }
}

const PARTIAL_SUFFIX: &str = ".partial";
const CHUNK: usize = 256 * 1024;

#[derive(Debug, Clone)]
pub struct ModelStore {
    dir: PathBuf,
}

impl ModelStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where `model` lives once installed.
    pub fn path(&self, model: &dyn ModelFile) -> PathBuf {
        self.dir.join(model.file_name())
    }

    fn partial_path(&self, model: &dyn ModelFile) -> PathBuf {
        self.dir
            .join(format!("{}{PARTIAL_SUFFIX}", model.file_name()))
    }

    /// Cheap check (size only). The full hash is verified when the model is
    /// installed and again by [`Self::verify`] before it is first loaded.
    pub fn state(&self, model: &dyn ModelFile) -> ModelState {
        match fs::metadata(self.path(model)) {
            Ok(meta) if meta.is_file() && meta.len() == model.size_bytes() => ModelState::Installed,
            Ok(_) => ModelState::Corrupt,
            Err(_) => ModelState::NotInstalled,
        }
    }

    /// Hashes the installed file. `Ok(true)` only for an exact match.
    pub fn verify(&self, model: &dyn ModelFile) -> io::Result<bool> {
        let mut file = File::open(self.path(model))?;
        if file.metadata()?.len() != model.size_bytes() {
            return Ok(false);
        }
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; CHUNK];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hex(&hasher.finalize()) == model.sha256())
    }

    /// Streams `source` into place with integrity checks. `progress`
    /// receives the running byte count; `cancel` aborts between chunks.
    pub fn install(
        &self,
        model: &dyn ModelFile,
        source: &mut dyn Read,
        progress: &mut dyn FnMut(u64),
        cancel: &AtomicBool,
    ) -> Result<(), ModelError> {
        fs::create_dir_all(&self.dir).map_err(|_| ModelError::Storage)?;
        let partial = self.partial_path(model);
        let result = self.write_verified(model, source, &partial, progress, cancel);
        if result.is_err() {
            let _ = fs::remove_file(&partial);
            return result;
        }
        // Same directory, so the rename is atomic; it replaces a corrupt
        // file of the same name.
        fs::rename(&partial, self.path(model)).map_err(|_| {
            let _ = fs::remove_file(&partial);
            ModelError::Storage
        })
    }

    fn write_verified(
        &self,
        model: &dyn ModelFile,
        source: &mut dyn Read,
        partial: &Path,
        progress: &mut dyn FnMut(u64),
        cancel: &AtomicBool,
    ) -> Result<(), ModelError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(partial)
            .map_err(|_| ModelError::Storage)?;
        let mut hasher = Sha256::new();
        let mut received: u64 = 0;
        let mut buf = vec![0u8; CHUNK];
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(ModelError::Cancelled);
            }
            let n = match source.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(ModelError::Network),
            };
            received += n as u64;
            // Never store more than the model can be.
            if received > model.size_bytes() {
                return Err(ModelError::Integrity);
            }
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n]).map_err(|_| ModelError::Storage)?;
            progress(received);
        }
        if received != model.size_bytes() || hex(&hasher.finalize()) != model.sha256() {
            return Err(ModelError::Integrity);
        }
        file.sync_all().map_err(|_| ModelError::Storage)?;
        Ok(())
    }

    /// Removes a model that failed verification.
    pub fn discard(&self, model: &dyn ModelFile) {
        let _ = fs::remove_file(self.path(model));
    }

    /// Removes leftovers of interrupted downloads (e.g. SERSHI was closed
    /// mid-download).
    pub fn remove_partials(&self) {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            if entry
                .file_name()
                .to_string_lossy()
                .ends_with(PARTIAL_SUFFIX)
            {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny fake model whose "download" is in memory.
    fn fake_model(content: &[u8]) -> SttModel {
        let sha = hex(&Sha256::digest(content));
        SttModel {
            id: "test",
            profile: sershi_core::voice::SpeechProfile::Fast,
            file_name: "ggml-test.bin",
            quantization: "q8_0",
            size_bytes: content.len() as u64,
            sha256: Box::leak(sha.into_boxed_str()),
            memory_mb: 1,
        }
    }

    fn temp_store(name: &str) -> ModelStore {
        let dir =
            std::env::temp_dir().join(format!("sershi-model-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        ModelStore::new(dir)
    }

    fn install(store: &ModelStore, model: &SttModel, bytes: &[u8]) -> Result<(), ModelError> {
        let mut progress = Vec::new();
        let result = store.install(
            model,
            &mut &bytes[..],
            &mut |n| progress.push(n),
            &AtomicBool::new(false),
        );
        if result.is_ok() {
            assert_eq!(progress.last().copied(), Some(bytes.len() as u64));
        }
        result
    }

    #[test]
    fn a_verified_download_is_installed_atomically() {
        let store = temp_store("ok");
        let content = b"model weights".repeat(1000);
        let model = fake_model(&content);
        assert_eq!(store.state(&model), ModelState::NotInstalled);
        install(&store, &model, &content).unwrap();
        assert_eq!(store.state(&model), ModelState::Installed);
        assert!(store.verify(&model).unwrap());
        assert!(!store.partial_path(&model).exists());
        let _ = fs::remove_dir_all(store.dir());
    }

    #[test]
    fn tampered_short_or_oversized_downloads_leave_nothing_behind() {
        let store = temp_store("bad");
        let content = b"genuine".repeat(100);
        let model = fake_model(&content);

        let mut tampered = content.clone();
        tampered[10] ^= 0xff;
        let mut long = content.clone();
        long.push(0);
        for bad in [tampered, content[..content.len() - 1].to_vec(), long] {
            assert_eq!(install(&store, &model, &bad), Err(ModelError::Integrity));
            assert_eq!(store.state(&model), ModelState::NotInstalled);
            assert!(!store.partial_path(&model).exists());
        }
        let _ = fs::remove_dir_all(store.dir());
    }

    #[test]
    fn cancelling_removes_the_partial_file() {
        let store = temp_store("cancel");
        let content = vec![7u8; 10];
        let model = fake_model(&content);
        let result = store.install(
            &model,
            &mut &content[..],
            &mut |_| {},
            &AtomicBool::new(true),
        );
        assert_eq!(result, Err(ModelError::Cancelled));
        assert!(!store.partial_path(&model).exists());
        assert_eq!(store.state(&model), ModelState::NotInstalled);
        let _ = fs::remove_dir_all(store.dir());
    }

    #[test]
    fn a_damaged_installed_model_is_detected() {
        let store = temp_store("corrupt");
        let content = vec![1u8; 64];
        let model = fake_model(&content);
        install(&store, &model, &content).unwrap();
        // Same size, different bytes: only the hash can tell.
        fs::write(store.path(&model), vec![2u8; 64]).unwrap();
        assert_eq!(store.state(&model), ModelState::Installed);
        assert!(!store.verify(&model).unwrap());
        fs::write(store.path(&model), vec![2u8; 10]).unwrap();
        assert_eq!(store.state(&model), ModelState::Corrupt);
        store.discard(&model);
        assert_eq!(store.state(&model), ModelState::NotInstalled);
        let _ = fs::remove_dir_all(store.dir());
    }

    #[test]
    fn leftover_partials_are_cleaned_up() {
        let store = temp_store("partials");
        let model = fake_model(b"x");
        fs::create_dir_all(store.dir()).unwrap();
        fs::write(store.partial_path(&model), b"half").unwrap();
        store.remove_partials();
        assert!(!store.partial_path(&model).exists());
        let _ = fs::remove_dir_all(store.dir());
    }

    #[test]
    fn network_errors_are_reported_as_such() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("connection reset"))
            }
        }
        let store = temp_store("net");
        let model = fake_model(b"abc");
        let result = store.install(&model, &mut Broken, &mut |_| {}, &AtomicBool::new(false));
        assert_eq!(result, Err(ModelError::Network));
        let _ = fs::remove_dir_all(store.dir());
    }
}
