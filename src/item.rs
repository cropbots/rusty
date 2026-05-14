use crate::helpers::{asset_path, data_path, load_wasm_manifest_files};
use crate::map::TileSet;
use macroquad::file::load_string;
use macroquad::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemCategory {
    All,
    Resources,
    Materials,
    Tiles,
    Utility,
}

#[derive(Clone)]
pub enum ItemIcon {
    Tile(u8),
    Texture(Texture2D),
}

pub struct ItemDefinition {
    pub key: String,
    pub name: String,
    pub short_name: String,
    pub category: ItemCategory,
    pub icon: ItemIcon,
    pub accent: Color,
}

pub struct InventoryCatalog {
    items: Vec<ItemDefinition>,
    by_key: HashMap<String, ItemId>,
}

impl InventoryCatalog {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            by_key: HashMap::new(),
        }
    }

    pub fn register(&mut self, def: ItemDefinition) -> ItemId {
        let id = ItemId(self.items.len());
        self.by_key.insert(def.key.clone(), id);
        self.items.push(def);
        id
    }

    pub fn get(&self, id: ItemId) -> &ItemDefinition {
        &self.items[id.0]
    }

    pub fn demo(tileset_count: usize, gear: Texture2D, gear_outline: Texture2D) -> Self {
        let mut catalog = Self::new();
        let resources = [
            (
                "Copper Scrap",
                "Scrap",
                ItemCategory::Resources,
                Color::from_hex(0xCC7A4C),
            ),
            (
                "Fiber Bundle",
                "Fiber",
                ItemCategory::Resources,
                Color::from_hex(0x85B864),
            ),
            (
                "Resin Clump",
                "Resin",
                ItemCategory::Resources,
                Color::from_hex(0xD3B65F),
            ),
            (
                "Seed Pack",
                "Seeds",
                ItemCategory::Resources,
                Color::from_hex(0x77C2A8),
            ),
            (
                "Clay Lump",
                "Clay",
                ItemCategory::Materials,
                Color::from_hex(0x9D6B53),
            ),
            (
                "Stone Dust",
                "Dust",
                ItemCategory::Materials,
                Color::from_hex(0x8A8F99),
            ),
            (
                "Core Gear",
                "Gear",
                ItemCategory::Utility,
                Color::from_hex(0x7FB5E6),
            ),
            (
                "Spare Cog",
                "Cog",
                ItemCategory::Utility,
                Color::from_hex(0xB4C4CF),
            ),
        ];
        for (idx, (name, short, category, accent)) in resources.into_iter().enumerate() {
            let texture = if idx % 2 == 0 {
                gear.clone()
            } else {
                gear_outline.clone()
            };
            catalog.register(ItemDefinition {
                key: format!("demo_resource_{idx}"),
                name: name.to_string(),
                short_name: short.to_string(),
                category,
                icon: ItemIcon::Texture(texture),
                accent,
            });
        }

        let tile_palette = [
            ("Soil", Color::from_hex(0x7C5A46)),
            ("Grass", Color::from_hex(0x6EAA4F)),
            ("Path", Color::from_hex(0xB89A6A)),
            ("Stone", Color::from_hex(0x8D949B)),
            ("Water", Color::from_hex(0x5A90D6)),
            ("Floor", Color::from_hex(0xC9B98A)),
        ];
        let tile_total = tileset_count.min(36);
        for tile_id in 0..tile_total {
            let (prefix, accent) = tile_palette[tile_id % tile_palette.len()];
            catalog.register(ItemDefinition {
                key: format!("demo_tile_{tile_id:02}"),
                name: format!("{prefix} Tile {tile_id:02}"),
                short_name: format!("{prefix} {tile_id:02}"),
                category: ItemCategory::Tiles,
                icon: ItemIcon::Tile(tile_id as u8),
                accent,
            });
        }
        catalog
    }

    pub fn iter_ids(&self) -> impl Iterator<Item = ItemId> + '_ {
        (0..self.items.len()).map(ItemId)
    }

    pub fn id_by_key(&self, key: &str) -> Option<ItemId> {
        self.by_key.get(key).copied()
    }

    pub async fn load_from(dir: &str) -> Result<Self, String> {
        let mut catalog = Self::new();
        let mut files: Vec<String> = if cfg!(target_arch = "wasm32") {
            load_wasm_manifest_files(dir, &["chopbot_summon.json", "cropbot_summon.json"]).await
        } else {
            let root = std::path::PathBuf::from(data_path(dir));
            let mut found = Vec::new();
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
                        found.push(name.to_string());
                    }
                }
            }
            found
        };
        files.sort();

        for file in files {
            let path = format!("{}/{}", dir.trim_end_matches('/'), file);
            let raw_str = load_string(&data_path(&path))
                .await
                .map_err(|err| format!("failed to read item '{}': {err}", path))?;
            let raw: ItemFile = serde_json::from_str(&raw_str)
                .map_err(|err| format!("failed to parse item '{}': {err}", path))?;
            let icon = match raw.icon {
                ItemIconFile::Tile { tile } => ItemIcon::Tile(tile),
                ItemIconFile::Texture { path } => {
                    let tex = load_texture(&asset_path(&path))
                        .await
                        .map_err(|err| format!("failed to load item texture '{}': {err}", path))?;
                    tex.set_filter(FilterMode::Nearest);
                    ItemIcon::Texture(tex)
                }
            };
            catalog.register(ItemDefinition {
                key: raw.id,
                name: raw.name,
                short_name: raw.short_name,
                category: raw.category.into(),
                icon,
                accent: raw.accent.into_color(),
            });
        }

        Ok(catalog)
    }
}

pub fn draw_item_icon(def: &ItemDefinition, tileset: &TileSet, rect: Rect) {
    match &def.icon {
        ItemIcon::Tile(tile) => {
            if let Some(source) = tileset.tile_rect(*tile) {
                draw_texture_ex(
                    tileset.texture(),
                    rect.x,
                    rect.y,
                    WHITE,
                    DrawTextureParams {
                        source: Some(source),
                        dest_size: Some(vec2(rect.w, rect.h)),
                        ..Default::default()
                    },
                );
            }
        }
        ItemIcon::Texture(texture) => {
            draw_texture_ex(
                texture,
                rect.x,
                rect.y,
                def.accent,
                DrawTextureParams {
                    dest_size: Some(vec2(rect.w, rect.h)),
                    ..Default::default()
                },
            );
        }
    }
}

pub fn format_stack_count(amount: u32) -> String {
    match amount {
        0..=999 => amount.to_string(),
        1_000..=999_999 => format!("{:.1}k", amount as f32 / 1_000.0),
        _ => format!("{:.1}m", amount as f32 / 1_000_000.0),
    }
}

#[derive(Deserialize)]
struct ItemFile {
    id: String,
    name: String,
    short_name: String,
    category: ItemCategoryFile,
    icon: ItemIconFile,
    accent: ItemColorFile,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ItemCategoryFile {
    Resources,
    Materials,
    Tiles,
    Utility,
}

impl From<ItemCategoryFile> for ItemCategory {
    fn from(value: ItemCategoryFile) -> Self {
        match value {
            ItemCategoryFile::Resources => ItemCategory::Resources,
            ItemCategoryFile::Materials => ItemCategory::Materials,
            ItemCategoryFile::Tiles => ItemCategory::Tiles,
            ItemCategoryFile::Utility => ItemCategory::Utility,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ItemIconFile {
    Tile { tile: u8 },
    Texture { path: String },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ItemColorFile {
    Hex(String),
    Rgb([u8; 3]),
    Rgba([u8; 4]),
}

impl ItemColorFile {
    fn into_color(self) -> Color {
        match self {
            ItemColorFile::Hex(hex) => {
                let s = hex.trim_start_matches('#');
                if s.len() == 6 {
                    if let Ok(v) = u32::from_str_radix(s, 16) {
                        return Color::from_rgba(
                            ((v >> 16) & 0xFF) as u8,
                            ((v >> 8) & 0xFF) as u8,
                            (v & 0xFF) as u8,
                            255,
                        );
                    }
                }
                WHITE
            }
            ItemColorFile::Rgb([r, g, b]) => Color::from_rgba(r, g, b, 255),
            ItemColorFile::Rgba([r, g, b, a]) => Color::from_rgba(r, g, b, a),
        }
    }
}
