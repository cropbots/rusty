use macroquad::prelude::*;

pub async fn load_single_texture(dir: &str, name: &str) -> Option<Texture2D> {
    let tile_path = format!("{}/{}.png", dir, name);
    load_texture(&tile_path).await.ok()
}