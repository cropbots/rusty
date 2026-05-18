use macroquad::prelude::*;

use crate::helpers::resolution_ui_scale;

const TILE_SIZE: f32 = 16.0;
const MAGNET_BLOCKS: f32 = 4.0;
const PICKUP_FLY_SPEED: f32 = 420.0;

#[derive(Clone)]
struct CogPickup {
    pos: Vec2,
    amount: u32,
    flying: bool,
}

pub struct CogWallet {
    balance: u32,
    start_balance: u32,
    pickups: Vec<CogPickup>,
    fly_targets: Vec<(usize, Vec2)>,
}

impl CogWallet {
    pub fn new(initial: u32) -> Self {
        Self {
            balance: initial,
            start_balance: initial,
            pickups: Vec::new(),
            fly_targets: Vec::new(),
        }
    }

    pub fn balance(&self) -> u32 {
        self.balance
    }

    pub fn reset_to_start(&mut self) {
        self.balance = self.start_balance;
        self.pickups.clear();
        self.fly_targets.clear();
    }

    pub fn snapshot_start(&mut self) {
        self.start_balance = self.balance;
    }

    pub fn spawn_pickup(&mut self, pos: Vec2, amount: u32) {
        if amount == 0 {
            return;
        }
        self.pickups.push(CogPickup {
            pos,
            amount,
            flying: false,
        });
    }

    pub fn update(&mut self, player_pos: Vec2, dt: f32) -> Option<(u32, Vec2)> {
        let magnet = MAGNET_BLOCKS * TILE_SIZE;
        let hud = cog_hud_anchor();
        let mut collected = None;

        for pickup in &mut self.pickups {
            if pickup.flying {
                continue;
            }
            if pickup.pos.distance(player_pos) <= magnet {
                pickup.flying = true;
            }
        }

        self.fly_targets.clear();
        for (idx, pickup) in self.pickups.iter_mut().enumerate() {
            if !pickup.flying {
                continue;
            }
            let dir = hud - pickup.pos;
            let dist = dir.length();
            if dist < 10.0 {
                self.fly_targets.push((idx, pickup.pos));
                continue;
            }
            pickup.pos += dir.normalize() * PICKUP_FLY_SPEED * dt;
        }

        if let Some((idx, pos)) = self.fly_targets.first().copied() {
            let amount = self.pickups.get(idx).map(|p| p.amount).unwrap_or(0);
            self.pickups.remove(idx);
            self.balance = self.balance.saturating_add(amount);
            collected = Some((amount, pos));
        }

        collected
    }

    pub fn draw_world(&self) {
        for pickup in &self.pickups {
            if pickup.flying {
                continue;
            }
            let bob = (get_time() as f32 * 4.0).sin() * 2.0;
            draw_circle(pickup.pos.x, pickup.pos.y + bob, 5.0, Color::from_hex(0xF0C040));
            draw_circle_lines(pickup.pos.x, pickup.pos.y + bob, 5.0, 1.5, Color::from_hex(0xFFE890));
            let label = format!("{}", pickup.amount);
            let m = measure_text(&label, None, 12, 1.0);
            draw_text(
                &label,
                pickup.pos.x - m.width * 0.5,
                pickup.pos.y + bob - 14.0,
                12.0,
                Color::from_hex(0xFFF6D0),
            );
        }
        for pickup in &self.pickups {
            if !pickup.flying {
                continue;
            }
            draw_circle(pickup.pos.x, pickup.pos.y, 4.0, Color::from_hex(0xFFE070));
        }
    }

    pub fn draw_hud(&self) {
        let anchor = cog_hud_anchor();
        let scale = resolution_ui_scale();
        let label = format!("Cogs: {}", self.balance);
        let size = (22.0 * scale).max(16.0);
        let metrics = measure_text(&label, None, size as u16, 1.0);
        let pad = 10.0 * scale;
        draw_rectangle(
            anchor.x - metrics.width - pad * 2.0,
            anchor.y - metrics.height - pad,
            metrics.width + pad * 2.0,
            metrics.height + pad * 1.4,
            Color::new(0.04, 0.05, 0.08, 0.82),
        );
        draw_rectangle_lines(
            anchor.x - metrics.width - pad * 2.0,
            anchor.y - metrics.height - pad,
            metrics.width + pad * 2.0,
            metrics.height + pad * 1.4,
            2.0,
            Color::from_hex(0xE6C781),
        );
        draw_text(
            &label,
            anchor.x - metrics.width - pad,
            anchor.y,
            size,
            Color::from_hex(0xF7E4B2),
        );
    }
}

pub fn cog_hud_anchor() -> Vec2 {
    let scale = resolution_ui_scale();
    vec2(screen_width() - 18.0 * scale, screen_height() - 18.0 * scale)
}
