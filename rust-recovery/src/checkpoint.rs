use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot};
use tokio::task;

use crate::error::{RecoveryError, Result};

const CHECKPOINT_VERSION: u32 = 1;
const HASH_READ_LIMIT: usize = 1_048_576;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub version: u32,
    pub timestamp: u64,
    pub image_path: String,
    pub image_hash: String,
    pub position: u64,
    pub state: serde_json::Value,
}

impl Checkpoint {
    pub fn new(
        image_path: impl Into<String>,
        image_hash: String,
        position: u64,
        state: serde_json::Value,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            version: CHECKPOINT_VERSION,
            timestamp,
            image_path: image_path.into(),
            image_hash,
            position,
            state,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResumeValidation {
    pub is_valid: bool,
    pub reason: Option<String>,
}

impl ResumeValidation {
    fn valid() -> Self {
        Self {
            is_valid: true,
            reason: None,
        }
    }

    fn invalid(reason: impl Into<String>) -> Self {
        Self {
            is_valid: false,
            reason: Some(reason.into()),
        }
    }
}

pub fn compute_image_hash(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    let mut buffer = vec![0u8; HASH_READ_LIMIT];
    let read = file.read(&mut buffer)?;

    let mut hasher = Sha256::new();
    hasher.update(&buffer[..read]);
    hasher.update(metadata.len().to_le_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn create_checkpoint(
    image_path: &Path,
    position: u64,
    state: serde_json::Value,
) -> Result<Checkpoint> {
    let image_hash = compute_image_hash(image_path)?;
    Ok(Checkpoint::new(
        image_path.to_string_lossy().to_string(),
        image_hash,
        position,
        state,
    ))
}

pub fn validate_resume(image_path: &Path, checkpoint: &Checkpoint) -> Result<ResumeValidation> {
    let expected_path = image_path.to_string_lossy();
    if checkpoint.image_path != expected_path {
        return Ok(ResumeValidation::invalid("image path mismatch"));
    }

    let computed_hash = compute_image_hash(image_path)?;
    if checkpoint.image_hash != computed_hash {
        return Ok(ResumeValidation::invalid("image hash mismatch"));
    }

    let size = fs::metadata(image_path)?.len();
    if checkpoint.position > size {
        return Ok(ResumeValidation::invalid("checkpoint position exceeds image size"));
    }

    Ok(ResumeValidation::valid())
}

pub fn load_checkpoint(path: &Path) -> Result<Checkpoint> {
    let data = fs::read(path)?;
    serde_json::from_slice(&data).map_err(|err| RecoveryError::Parse(err.to_string()))
}

pub fn save_checkpoint_blocking(path: &Path, checkpoint: &Checkpoint, backup: bool) -> Result<()> {
    let serialized = serde_json::to_vec_pretty(checkpoint)
        .map_err(|err| RecoveryError::Parse(err.to_string()))?;
    let tmp_path = path.with_extension("tmp");

    {
        let mut file = File::create(&tmp_path)?;
        file.write_all(&serialized)?;
        file.sync_all()?;
    }

    if backup && path.exists() {
        let backup_path = path.with_extension("bak");
        let _ = fs::copy(path, backup_path);
    }

    fs::rename(tmp_path, path)?;
    Ok(())
}

pub async fn save_checkpoint_atomic(path: &Path, checkpoint: &Checkpoint, backup: bool) -> Result<()> {
    let path = path.to_path_buf();
    let checkpoint = checkpoint.clone();
    task::spawn_blocking(move || save_checkpoint_blocking(&path, &checkpoint, backup))
        .await
        .map_err(|err| RecoveryError::Config(format!("Checkpoint task failed: {err}")))?
}

#[derive(Debug)]
pub struct CheckpointManager {
    sender: mpsc::Sender<CheckpointRequest>,
}

impl CheckpointManager {
    pub fn start(path: impl AsRef<Path>, backup: bool) -> Self {
        let (sender, mut receiver) = mpsc::channel(32);
        let path = path.as_ref().to_path_buf();

        tokio::spawn(async move {
            while let Some(request) = receiver.recv().await {
                match request {
                    CheckpointRequest::Save { checkpoint, responder } => {
                        let result = save_checkpoint_atomic(&path, &checkpoint, backup).await;
                        if let Some(responder) = responder {
                            let _ = responder.send(result);
                        }
                    }
                    CheckpointRequest::Shutdown { responder } => {
                        if let Some(responder) = responder {
                            let _ = responder.send(Ok(()));
                        }
                        break;
                    }
                }
            }
        });

        Self { sender }
    }

    pub async fn save(&self, checkpoint: Checkpoint) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(CheckpointRequest::Save {
                checkpoint,
                responder: Some(tx),
            })
            .await
            .map_err(|_| RecoveryError::Config("Checkpoint manager channel closed".to_string()))?;
        rx.await
            .map_err(|_| RecoveryError::Config("Checkpoint manager channel closed".to_string()))?
    }

    pub async fn save_fire_and_forget(&self, checkpoint: Checkpoint) -> Result<()> {
        self.sender
            .send(CheckpointRequest::Save {
                checkpoint,
                responder: None,
            })
            .await
            .map_err(|_| RecoveryError::Config("Checkpoint manager channel closed".to_string()))
    }

    pub async fn shutdown(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(CheckpointRequest::Shutdown { responder: Some(tx) })
            .await
            .map_err(|_| RecoveryError::Config("Checkpoint manager channel closed".to_string()))?;
        rx.await
            .map_err(|_| RecoveryError::Config("Checkpoint manager channel closed".to_string()))?
    }
}

#[derive(Debug)]
enum CheckpointRequest {
    Save {
        checkpoint: Checkpoint,
        responder: Option<oneshot::Sender<Result<()>>>,
    },
    Shutdown {
        responder: Option<oneshot::Sender<Result<()>>>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut dir = std::env::temp_dir();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        dir.push(format!("rust_recovery_checkpoint_{unique}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn create_image(path: &Path, content: &[u8]) {
        let mut file = File::create(path).unwrap();
        file.write_all(content).unwrap();
        file.sync_all().unwrap();
    }

    #[test]
    fn test_checkpoint_save_load_backup() {
        let dir = temp_dir();
        let image_path = dir.join("image.bin");
        create_image(&image_path, b"example_data");

        let checkpoint = create_checkpoint(&image_path, 128, serde_json::json!({"step": 1})).unwrap();
        let checkpoint_path = dir.join("checkpoint.json");
        save_checkpoint_blocking(&checkpoint_path, &checkpoint, true).unwrap();

        let loaded = load_checkpoint(&checkpoint_path).unwrap();
        assert_eq!(loaded.image_hash, checkpoint.image_hash);
        assert_eq!(loaded.position, 128);

        let checkpoint2 = Checkpoint::new(
            checkpoint.image_path.clone(),
            checkpoint.image_hash.clone(),
            256,
            serde_json::json!({"step": 2}),
        );
        save_checkpoint_blocking(&checkpoint_path, &checkpoint2, true).unwrap();
        assert!(checkpoint_path.with_extension("bak").exists());
    }

    #[test]
    fn test_resume_validation_detects_hash_mismatch() {
        let dir = temp_dir();
        let image_path = dir.join("image.bin");
        create_image(&image_path, b"example_data");

        let mut checkpoint = create_checkpoint(&image_path, 64, serde_json::json!({})).unwrap();
        checkpoint.image_hash = "invalid".to_string();

        let validation = validate_resume(&image_path, &checkpoint).unwrap();
        assert!(!validation.is_valid);
        assert!(validation.reason.unwrap_or_default().contains("hash"));
    }

    #[tokio::test]
    async fn test_checkpoint_manager_async_save() {
        let dir = temp_dir();
        let image_path = dir.join("image.bin");
        create_image(&image_path, b"example_data");

        let checkpoint_path = dir.join("checkpoint.json");
        let manager = CheckpointManager::start(&checkpoint_path, true);
        let checkpoint = create_checkpoint(&image_path, 512, serde_json::json!({"mode": "async"}))
            .unwrap();
        manager.save(checkpoint.clone()).await.unwrap();
        manager.shutdown().await.unwrap();

        let loaded = load_checkpoint(&checkpoint_path).unwrap();
        assert_eq!(loaded.position, 512);
    }
}
