use std::collections::HashMap;

use macroquad::prelude::*;

use crate::{map::TileMap, player::Player};

#[derive(Clone)]
pub enum InteractAction {
    Warp { target: String },
    OpenLoot { table_id: String },
    OpenLockPuzzle { puzzle_id: String, area: Rect },
}

pub struct InteractContext<'a> {
    pub structure_id: &'a str,
    pub area: Rect,
    pub player: &'a mut Player,
    pub map: &'a mut TileMap,
    pub actions: &'a mut Vec<InteractAction>,
}

pub type InteractFn = fn(&mut InteractContext<'_>);

pub struct InteractRegistry {
    funcs: HashMap<String, InteractFn>,
}

impl InteractRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            funcs: HashMap::new(),
        };
        registry.register("log_interact", interact_log);
        registry.register("heal_player_small", interact_heal_player_small);
        registry.register("damage_player_small", interact_damage_player_small);
        registry
    }

    pub fn register(&mut self, name: &str, func: InteractFn) {
        self.funcs.insert(name.to_string(), func);
    }

    pub fn execute(&self, names: &[String], ctx: &mut InteractContext<'_>) {
        for name in names {
            if let Some(func) = self.funcs.get(name).copied() {
                func(ctx);
            } else if handle_block_action(name, ctx) {
                continue;
            } else {
                eprintln!(
                    "unknown structure interact function '{}' on '{}'",
                    name, ctx.structure_id
                );
            }
        }
    }
}

fn handle_block_action(name: &str, ctx: &mut InteractContext<'_>) -> bool {
    if let Some(target) = name.strip_prefix("warp:") {
        ctx.actions.push(InteractAction::Warp {
            target: target.to_string(),
        });
        return true;
    }
    if let Some(table_id) = name.strip_prefix("loot:") {
        ctx.actions.push(InteractAction::OpenLoot {
            table_id: table_id.to_string(),
        });
        return true;
    }
    if let Some(puzzle_id) = name.strip_prefix("lock_puzzle:") {
        ctx.actions.push(InteractAction::OpenLockPuzzle {
            puzzle_id: puzzle_id.to_string(),
            area: ctx.area,
        });
        return true;
    }
    false
}

fn interact_log(ctx: &mut InteractContext<'_>) {
    let _ = ctx.map.tile_size();
    eprintln!(
        "interacted with '{}' at ({:.1}, {:.1})",
        ctx.structure_id, ctx.area.x, ctx.area.y
    );
}

fn interact_heal_player_small(ctx: &mut InteractContext<'_>) {
    ctx.player.heal(25.0);
}

fn interact_damage_player_small(ctx: &mut InteractContext<'_>) {
    ctx.player.apply_damage(25.0);
}
