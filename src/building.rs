use macroquad::prelude::*;

use crate::helpers::resolution_ui_scale;
use crate::map::{StructureDef, TileMap};

#[derive(Clone)]
pub struct BuildableStructure {
    pub id: String,
    pub label: String,
}

pub struct BuildingMode {
    pub active: bool,
    pub catalog: Vec<BuildableStructure>,
    pub selected: usize,
    pub placements: Vec<(String, Vec2)>,
}

impl BuildingMode {
    pub fn from_structures(structures: &[StructureDef]) -> Self {
        let catalog = structures
            .iter()
            .filter(|s| s.player_buildable)
            .map(|s| BuildableStructure {
                id: s.id.clone(),
                label: s.id.replace('_', " "),
            })
            .collect();
        Self {
            active: false,
            catalog,
            selected: 0,
            placements: Vec::new(),
        }
    }

    pub fn toggle(&mut self) {
        if self.catalog.is_empty() {
            return;
        }
        self.active = !self.active;
    }

    pub fn selected_def<'a>(&self, structures: &'a [StructureDef]) -> Option<&'a StructureDef> {
        let id = self.catalog.get(self.selected)?.id.as_str();
        structures.iter().find(|s| s.id == id)
    }

    pub fn cycle(&mut self, delta: i32) {
        if self.catalog.is_empty() {
            return;
        }
        let len = self.catalog.len() as i32;
        self.selected = (self.selected as i32 + delta).rem_euclid(len) as usize;
    }

    pub fn preview_rect(&self, mouse_world: Vec2, tile_size: f32) -> Rect {
        let gx = (mouse_world.x / tile_size).floor();
        let gy = (mouse_world.y / tile_size).floor();
        Rect::new(gx * tile_size, gy * tile_size, tile_size, tile_size)
    }

    pub fn can_place(&self, map: &TileMap, rect: Rect, grass_tile: u8) -> bool {
        let tile = map.tile_size().max(1.0);
        let gx = (rect.x / tile).floor().max(0.0) as usize;
        let gy = (rect.y / tile).floor().max(0.0) as usize;
        if gx >= map.width() || gy >= map.height() {
            return false;
        }
        if map.is_solid(gx, gy) {
            return false;
        }
        map.tile_at(crate::map::LayerKind::Background, gx, gy) == grass_tile
    }

    pub fn commit_placement(&mut self, structure_id: &str, origin: Vec2) {
        self.placements.push((structure_id.to_string(), origin));
    }

    pub fn draw_hud(&self) {
        if !self.active || self.catalog.is_empty() {
            return;
        }
        let scale = resolution_ui_scale();
        let sel = &self.catalog[self.selected];
        let title = format!("Build: {}", sel.label);
        let hint = "LMB place  |  Q/E scroll  |  B close  |  Enter finish";
        let y = 72.0 * scale;
        draw_text(&title, 18.0 * scale, y, (24.0 * scale).max(16.0), Color::from_hex(0xF7E4B2));
        draw_text(
            hint,
            18.0 * scale,
            y + 28.0 * scale,
            (16.0 * scale).max(12.0),
            Color::from_hex(0xB8C8E8),
        );
        if !self.placements.is_empty() {
            let pending = format!("Pending: {}", self.placements.len());
            draw_text(
                &pending,
                18.0 * scale,
                y + 52.0 * scale,
                (16.0 * scale).max(12.0),
                Color::from_hex(0x90E0A8),
            );
        }
    }

    pub fn draw_preview(&self, rect: Rect, valid: bool) {
        let fill = if valid {
            Color::new(0.3, 0.9, 0.5, 0.28)
        } else {
            Color::new(0.95, 0.25, 0.25, 0.28)
        };
        let line = if valid {
            Color::new(0.4, 1.0, 0.6, 0.95)
        } else {
            Color::new(1.0, 0.35, 0.35, 0.95)
        };
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill);
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, line);
    }
}
