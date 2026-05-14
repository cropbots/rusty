use macroquad::file::load_string;
use serde::Deserialize;
use std::collections::HashMap;

use crate::helpers::{data_path, load_wasm_manifest_files};

#[derive(Clone, Default)]
pub struct LootTables {
    tables: HashMap<String, Vec<LootEntry>>,
}

#[derive(Clone, Deserialize)]
pub struct LootEntry {
    pub item: String,
    pub amount: u32,
}

#[derive(Deserialize)]
struct LootTableFile {
    id: String,
    entries: Vec<LootEntry>,
}

impl LootTables {
    pub fn empty() -> Self {
        Self::default()
    }

    pub async fn load_from(dir: &str) -> Result<Self, String> {
        let mut files: Vec<String> = if cfg!(target_arch = "wasm32") {
            load_wasm_manifest_files(dir, &["basic_crate.json"]).await
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

        let mut tables = HashMap::new();
        for file in files {
            let path = format!("{}/{}", dir.trim_end_matches('/'), file);
            let raw = load_string(&data_path(&path))
                .await
                .map_err(|e| format!("failed to read loot file '{}': {e}", path))?;
            let table: LootTableFile = serde_json::from_str(&raw)
                .map_err(|e| format!("failed to parse loot file '{}': {e}", path))?;
            tables.insert(table.id, table.entries);
        }

        Ok(Self { tables })
    }

    pub fn get(&self, id: &str) -> Option<&[LootEntry]> {
        self.tables.get(id).map(Vec::as_slice)
    }
}
