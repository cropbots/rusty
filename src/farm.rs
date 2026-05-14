use crate::map::{GridIndex, LayerKind, TileMap};
use std::collections::HashMap;

pub const FARM_TILES: [u8; 8] = [24, 25, 26, 27, 28, 29, 30, 31];
pub const EMPTY_FARM_TILES: [u8; 4] = [24, 25, 26, 27];

pub fn farm_stage_table() -> HashMap<u8, u8> {
    let mut out = HashMap::new();
    // Random placeholder stage mapping; user can replace with real values.
    out.insert(24, 0);
    out.insert(25, 1);
    out.insert(26, 2);
    out.insert(27, 3);
    out.insert(28, 0);
    out.insert(29, 1);
    out.insert(30, 2);
    out.insert(31, 3);
    out
}

pub fn nearest_farm_grid(map: &TileMap, from: GridIndex, require_empty: bool) -> Option<GridIndex> {
    let max_r = 22i32;
    for r in 0..=max_r {
        for y in (from.y - r)..=(from.y + r) {
            for x in (from.x - r)..=(from.x + r) {
                if (x - from.x).abs().max((y - from.y).abs()) != r {
                    continue;
                }
                if x < 0 || y < 0 || x as usize >= map.width() || y as usize >= map.height() {
                    continue;
                }
                let tile = map.tile_at(LayerKind::Background, x as usize, y as usize);
                if require_empty {
                    if EMPTY_FARM_TILES.contains(&tile) {
                        return Some(GridIndex { x, y });
                    }
                } else if FARM_TILES.contains(&tile) {
                    return Some(GridIndex { x, y });
                }
            }
        }
    }
    None
}

pub fn lowest_stage_farm_grid(map: &TileMap, from: GridIndex) -> Option<GridIndex> {
    let stages = farm_stage_table();
    let mut best: Option<(u8, i32, GridIndex)> = None;
    let max_r = 30i32;
    for y in (from.y - max_r)..=(from.y + max_r) {
        for x in (from.x - max_r)..=(from.x + max_r) {
            if x < 0 || y < 0 || x as usize >= map.width() || y as usize >= map.height() {
                continue;
            }
            let tile = map.tile_at(LayerKind::Background, x as usize, y as usize);
            let Some(&stage) = stages.get(&tile) else {
                continue;
            };
            let d = (x - from.x).abs() + (y - from.y).abs();
            match best {
                Some((best_stage, best_d, _))
                    if stage > best_stage || (stage == best_stage && d >= best_d) => {}
                _ => best = Some((stage, d, GridIndex { x, y })),
            }
        }
    }
    best.map(|(_, _, g)| g)
}

pub fn advance_stage(tile: u8) -> u8 {
    match tile {
        24 => 25,
        25 => 26,
        26 => 27,
        28 => 29,
        29 => 30,
        30 => 31,
        _ => tile,
    }
}
