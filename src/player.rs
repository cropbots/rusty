use macroquad::prelude::*;

pub struct Player {
    pos: Vec2,
    vel: Vec2,
    hitbox: Rect,
    radius: f32,
    texture: Texture2D,
}

impl Player {
    pub fn new(pos: Vec2, texture: Texture2D, hitbox: Rect) -> Self {
        Self {
            pos,
            vel: Vec2::ZERO,
            hitbox,
            radius: 5.0,
            texture,
        }
    }

    pub fn update(&mut self) {
        let dt = get_frame_time();

        let mut input = vec2(0.0, 0.0);
        if is_key_down(KeyCode::D) {
            input.x += 1.0;
        }
        if is_key_down(KeyCode::A) {
            input.x -= 1.0;
        }
        if is_key_down(KeyCode::W) {
            input.y -= 1.0;
        }
        if is_key_down(KeyCode::S) {
            input.y += 1.0;
        }

        if input.length_squared() > 0.0 {
            input = input.normalize();
        }

        let accel = 1800.0;
        let max_speed = 640.0;
        let damping = 8.0;

        self.vel += input * accel * dt;

        let speed = self.vel.length();
        if speed > max_speed {
            self.vel = self.vel / speed * max_speed;
        }

        let decay = (1.0 - damping * dt).clamp(0.0, 1.0);
        self.vel *= decay;

        self.pos += self.vel * dt;
    }


    pub fn draw(&self) {
        // Draw the hitbox
        draw_rectangle(
            self.hitbox.x + self.pos.x,
            self.hitbox.y + self.pos.y,
            self.hitbox.w,
            self.hitbox.h,
            Color::from_hex(0xFF0000),
        );

        let scale = 0.5;
        let center_x = self.texture.width() as f32 * scale / 2.0;
        let center_y = self.texture.height() as f32 * scale / 2.0;
        draw_texture_ex(
            &self.texture,
            self.pos.x - center_x / 2.0,
            self.pos.y - center_y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(Vec2::new(self.texture.width() / 2 as f32 * scale, self.texture.height() / 2 as f32 * scale)),
                flip_y: false,
                ..Default::default()
            },
        );
    }

    pub fn position(&self) -> Vec2 {
        self.pos
    }
}
