use serde::{Deserialize, Serialize};
use macroquad::prelude::*;
use crate::helpers::asset_path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TileInfo {
    pub id: u16,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Tileset {
    pub image: String,
    pub tile_width: u16,
    pub tile_height: u16,
    pub columns: u16,
    pub rows: u16,
    pub tile_count: u16,
    pub tiles: Vec<TileInfo>,
    #[serde(skip, default)]
    tiles_by_id: Vec<Option<Rect>>,
}

impl Tileset {
    pub async fn load(tileset_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let json_path = asset_path(tileset_path);
        let json_content = load_string(&json_path).await?;
        let mut tileset: Tileset = serde_json::from_str(&json_content)?;
        tileset.rebuild_lookup();
        Ok(tileset)
    }
    
    pub fn get_tile_rect(&self, tile_id: u16) -> Option<Rect> {
        self.tiles_by_id
            .get(tile_id as usize)
            .and_then(|rect| *rect)
    }

    fn rebuild_lookup(&mut self) {
        let mut max_id = 0usize;
        for tile in &self.tiles {
            max_id = max_id.max(tile.id as usize);
        }
        let count = self.tile_count.max((max_id + 1) as u16) as usize;
        self.tiles_by_id = vec![None; count];
        for tile in &self.tiles {
            let idx = tile.id as usize;
            if idx >= self.tiles_by_id.len() {
                self.tiles_by_id.resize(idx + 1, None);
            }
            self.tiles_by_id[idx] = Some(Rect::new(
                tile.x as f32,
                tile.y as f32,
                tile.width as f32,
                tile.height as f32,
            ));
        }
    }
}

#[derive(Debug, Clone)]
pub struct Tilemap {
    pub tileset: Tileset,
    pub texture: Texture2D,
    pub map_data: Vec<Vec<u16>>,
    pub tile_width: f32,
    pub tile_height: f32,
    pub width: usize,
    pub height: usize,
}

impl Tilemap {
    pub async fn new(tileset_path: &str, texture_path: &str, map_width: usize, map_height: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let tileset = Tileset::load(tileset_path).await?;
        let tile_width = tileset.tile_width as f32;
        let tile_height = tileset.tile_height as f32;
        let texture_path = asset_path(texture_path);
        let texture = load_texture(&texture_path).await?;
        
        Ok(Tilemap {
            tileset,
            texture,
            map_data: vec![vec![0; map_width]; map_height],
            tile_width,
            tile_height,
            width: map_width,
            height: map_height,
        })
    }
    
    pub fn set_tile(&mut self, x: usize, y: usize, tile_id: u16) {
        if x < self.width && y < self.height {
            self.map_data[y][x] = tile_id;
        }
    }
    
    pub fn get_tile(&self, x: usize, y: usize) -> u16 {
        if x < self.width && y < self.height {
            self.map_data[y][x]
        } else {
            0
        }
    }
    
    pub fn draw(&self, start_x: f32, start_y: f32) {
        for (y, row) in self.map_data.iter().enumerate() {
            for (x, &tile_id) in row.iter().enumerate() {
                if tile_id == 0 { continue; } // Skip empty tiles
                
                if let Some(source_rect) = self.tileset.get_tile_rect(tile_id) {
                    let dest_rect = Rect::new(
                        start_x + x as f32 * self.tile_width,
                        start_y + y as f32 * self.tile_height,
                        self.tile_width,
                        self.tile_height
                    );
                    
                    draw_texture_ex(
                        &self.texture,
                        dest_rect.x,
                        dest_rect.y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(source_rect),
                            dest_size: Some(vec2(dest_rect.w, dest_rect.h)),
                            ..Default::default()
                        }
                    );
                }
            }
        }
    }
}
