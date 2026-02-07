use macroquad::prelude::*;

pub async fn load_single_texture(dir: &str, name: &str) -> Option<Texture2D> {
    let tile_path = format!("{}/{}.png", dir, name);
    load_texture(&tile_path).await.ok()
}

pub async fn draw_hitbox(hitbox: Rect, pos: Vec2) {
    draw_rectangle(
        hitbox.x + pos.x,
        hitbox.y + pos.y,
        hitbox.w,
        hitbox.h,
        Color::from_hex(0xFF0000),
    );
}

pub struct Entity {
    position: Vec2,
    hitbox: Rect,
}