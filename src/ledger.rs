use sha2::{Digest, Sha256};
use serde::{Serialize, Deserialize};
use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};
use anyhow::{Result, Context};
use std::io::Write;

#[derive(Debug, Serialize, Deserialize)]
pub struct Record {
    pub ts: u64,
    pub action: String,
    pub details: serde_json::Value,
    pub prev_hash: String,
    pub hash: String,
}

pub struct Ledger {
    path: PathBuf,
    last_hash: String,
}

impl Ledger {
    pub fn open(path: PathBuf) -> Result<Self> {
        let mut last_hash = String::new();
        if let Ok(data) = fs::read_to_string(&path) {
            for line in data.lines() {
                if let Ok(rec) = serde_json::from_str::<Record>(line) {
                    last_hash = rec.hash;
                }
            }
        }
        Ok(Self { path, last_hash })
    }

    pub fn append(&mut self, action: &str, details: serde_json::Value) -> Result<()> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let mut hasher = Sha256::new();
        hasher.update(self.last_hash.as_bytes());
        hasher.update(ts.to_le_bytes());
        hasher.update(action.as_bytes());
        hasher.update(details.to_string().as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        let rec = Record { ts, action: action.to_string(), details, prev_hash: self.last_hash.clone(), hash: hash.clone() };
        let line = serde_json::to_string(&rec)? + "\n";
        let mut f = fs::OpenOptions::new().create(true).append(true).open(&self.path)
            .with_context(|| format!("open ledger {}", self.path.display()))?;
        f.write_all(line.as_bytes()).with_context(|| "append ledger")?;
        self.last_hash = hash;
        Ok(())
    }
}
