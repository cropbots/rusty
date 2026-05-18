use macroquad::file::load_string;
use serde::Deserialize;
use std::collections::HashMap;

use crate::helpers::{data_path, load_wasm_manifest_files};

#[derive(Clone, Deserialize)]
pub struct PuzzleDef {
    pub id: String,
    pub prompt: String,
    pub starter_code: String,
    pub validator_contains: String,
    #[serde(default)]
    pub viz_config: Option<serde_json::Value>,
}

#[derive(Default)]
pub struct PuzzleCatalog {
    puzzles: HashMap<String, PuzzleDef>,
}

impl PuzzleCatalog {
    pub fn empty() -> Self {
        Self::default()
    }

    pub async fn load_from(dir: &str) -> Result<Self, String> {
        let mut files: Vec<String> = if cfg!(target_arch = "wasm32") {
            load_wasm_manifest_files(dir, &["basic_lock.json"]).await
        } else {
            let root = std::path::PathBuf::from(data_path(dir));
            let mut out = Vec::new();
            if root.exists() {
                for entry in std::fs::read_dir(&root).map_err(|e| e.to_string())? {
                    let entry = entry.map_err(|e| e.to_string())?;
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("json") {
                        continue;
                    }
                    if path.file_name().and_then(|n| n.to_str()) == Some("index.json") {
                        continue;
                    }
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        out.push(name.to_string());
                    }
                }
            }
            out
        };
        files.sort();

        let mut puzzles = HashMap::new();
        for file in files {
            let path = format!("{}/{}", dir.trim_end_matches('/'), file);
            let raw = load_string(&data_path(&path))
                .await
                .map_err(|e| format!("failed to read puzzle file '{}': {e}", path))?;
            let puzzle: PuzzleDef = serde_json::from_str(&raw)
                .map_err(|e| format!("failed to parse puzzle file '{}': {e}", path))?;
            puzzles.insert(puzzle.id.clone(), puzzle);
        }

        Ok(Self { puzzles })
    }

    pub fn get(&self, id: &str) -> Option<&PuzzleDef> {
        self.puzzles.get(id)
    }
}
