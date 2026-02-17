use macroquad::prelude::*;

use crate::entity::{Entity, EntityDatabase, MovementRegistry};
use crate::helpers::random_range;
use crate::map::{LayerKind, StructureDef, TileMap, TileMapSnapshot};

pub const EXPEDITION_WIDTH: usize = 1024;
pub const EXPEDITION_HEIGHT: usize = 1024;
pub const FARM_WIDTH: usize = 100;
pub const FARM_HEIGHT: usize = 50;

const FARM_OUTER_MARGIN: usize = 128;
const FARM_MAP_WIDTH: usize = FARM_WIDTH + FARM_OUTER_MARGIN * 2;
const FARM_MAP_HEIGHT: usize = FARM_HEIGHT + FARM_OUTER_MARGIN * 2;

const EXPEDITION_DECOR_SEED: u32 = 0x6D2B_79F5;
const FARM_DECOR_SEED: u32 = 0xA531_2D91;
const EXPEDITION_EDGE_BAND: usize = 96;
const DECOR_STRUCTURE_IDS: [&str; 2] = ["tree_plains", "bush_plains"];
const SCENE_DECOR_DENSITY_SCALE: f32 = 0.75;
const SCENE_DECOR_MAX_PER_DEF: usize = 1200;

#[cfg(target_arch = "wasm32")]
const FARM_STORAGE_KEY: &str = "cropbots:farm.json";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SceneKind {
    Expedition,
    Farm,
}

#[derive(Clone, Copy)]
struct TileRect {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

impl TileRect {
    fn max_x(self) -> usize {
        self.x + self.w
    }

    fn max_y(self) -> usize {
        self.y + self.h
    }
}

pub fn clear_scenes(map: &mut TileMap, entities: &mut Vec<Entity>) {
    entities.clear();
    map.clear_all_tiles();
}

pub fn expedition_spawn_point() -> Vec2 {
    vec2(200.0, 300.0 + 16.0 / 2.0)
}

pub fn farm_spawn_point(map: &TileMap) -> Vec2 {
    let area = inset_tile_rect(farm_core_rect(), 1);
    let ts = map.tile_size();
    vec2(
        (area.x as f32 + area.w as f32 * 0.5) * ts,
        (area.y as f32 + area.h as f32 * 0.5) * ts,
    )
}

pub fn place_structure_from_defs(
    map: &mut TileMap,
    structures: &[StructureDef],
    structure_id: &str,
    tile_x: usize,
    tile_y: usize,
) -> bool {
    let Some(def) = find_structure(structures, structure_id) else {
        return false;
    };
    map.place_structure_def(def, tile_x, tile_y);
    true
}

pub fn scene_expedition(
    map: &mut TileMap,
    entities: &mut Vec<Entity>,
    db: &EntityDatabase,
    registry: &MovementRegistry,
    structures: &[StructureDef],
    ground_tile: u8,
    tile_size: f32,
    chunk_alloc_per_frame: usize,
    chunk_rebuild_per_frame: usize,
) {
    clear_scenes(map, entities);

    let mut next = TileMap::new_deferred(
        EXPEDITION_WIDTH,
        EXPEDITION_HEIGHT,
        tile_size,
        Vec2::new(tile_size, tile_size),
        0.0,
    );
    next.set_chunk_work_budget(chunk_alloc_per_frame, chunk_rebuild_per_frame);
    next.fill_layer(LayerKind::Background, ground_tile);
    next.set_custom_border_hitbox(None);
    spawn_expedition_edge_decorations(&mut next, structures);
    *map = next;

    entities.clear();
    for _ in 0..200 {
        let pos = vec2(random_range(0.0, 500.0), random_range(0.0, 500.0));
        if let Some(virabird) = Entity::spawn(db, "virabird", pos, registry) {
            entities.push(virabird);
        }
    }
    for _ in 0..200 {
        let pos = vec2(random_range(0.0, 500.0), random_range(0.0, 500.0));
        if let Some(virat) = Entity::spawn(db, "virat", pos, registry) {
            entities.push(virat);
        }
    }
    for _ in 0..200 {
        let pos = vec2(random_range(0.0, 500.0), random_range(0.0, 500.0));
        if let Some(chopbot) = Entity::spawn(db, "chopbot", pos, registry) {
            entities.push(chopbot);
        }
    }
}

pub fn scene_farm(
    map: &mut TileMap,
    entities: &mut Vec<Entity>,
    structures: &[StructureDef],
    ground_tile: u8,
    tile_size: f32,
    chunk_alloc_per_frame: usize,
    chunk_rebuild_per_frame: usize,
) {
    clear_scenes(map, entities);

    let mut next = TileMap::new_deferred(
        FARM_MAP_WIDTH,
        FARM_MAP_HEIGHT,
        tile_size,
        Vec2::new(tile_size, tile_size),
        0.0,
    );
    next.set_chunk_work_budget(chunk_alloc_per_frame, chunk_rebuild_per_frame);
    next.fill_layer(LayerKind::Background, ground_tile);

    let farm_area = farm_core_rect();
    let farm_inner_area = inset_tile_rect(farm_area, 1);
    let loaded = load_farm_snapshot()
        .map(|snapshot| next.apply_snapshot(&snapshot).is_ok())
        .unwrap_or(false);

    if !loaded {
        spawn_farm_outer_decorations(&mut next, structures, farm_area);
        spawn_farm_inner_decorations(&mut next, structures, farm_inner_area);
    }

    place_farm_bush_border(&mut next, structures, farm_area);
    next.set_custom_border_hitbox(Some(tile_rect_to_world_rect(farm_inner_area, tile_size)));

    *map = next;
    entities.clear();
}

pub fn save_farm_scene(map: &TileMap) -> bool {
    let snapshot = map.snapshot();
    let json = match serde_json::to_string(&snapshot) {
        Ok(json) => json,
        Err(err) => {
            eprintln!("failed to serialize farm scene: {err}");
            return false;
        }
    };
    save_farm_snapshot_json(&json)
}

fn spawn_expedition_edge_decorations(map: &mut TileMap, structures: &[StructureDef]) {
    let band = EXPEDITION_EDGE_BAND
        .min(map.width() / 2)
        .min(map.height() / 2);
    let inner = TileRect {
        x: band,
        y: band,
        w: map.width().saturating_sub(band * 2),
        h: map.height().saturating_sub(band * 2),
    };
    let edge_area_tiles = map
        .width()
        .saturating_mul(map.height())
        .saturating_sub(inner.w.saturating_mul(inner.h));

    for (i, id) in DECOR_STRUCTURE_IDS.iter().enumerate() {
        let Some(def) = find_structure(structures, id) else {
            continue;
        };
        let seed = EXPEDITION_DECOR_SEED ^ ((i as u32 + 1).wrapping_mul(0x9E37_79B9));
        scatter_structure_where(map, def, seed, edge_area_tiles, |candidate| {
            inner.w == 0 || inner.h == 0 || !tile_rect_intersects(candidate, inner)
        });
    }
}

fn spawn_farm_outer_decorations(
    map: &mut TileMap,
    structures: &[StructureDef],
    farm_area: TileRect,
) {
    let outer_area_tiles = map
        .width()
        .saturating_mul(map.height())
        .saturating_sub(farm_area.w.saturating_mul(farm_area.h));

    for (i, id) in DECOR_STRUCTURE_IDS.iter().enumerate() {
        let Some(def) = find_structure(structures, id) else {
            continue;
        };
        let seed = FARM_DECOR_SEED ^ ((i as u32 + 1).wrapping_mul(0x7FEB_352D));
        scatter_structure_where(map, def, seed, outer_area_tiles, |candidate| {
            !tile_rect_intersects(candidate, farm_area)
        });
    }
}

fn spawn_farm_inner_decorations(
    map: &mut TileMap,
    structures: &[StructureDef],
    farm_area: TileRect,
) {
    let inner_area_tiles = farm_area.w.saturating_mul(farm_area.h);
    if inner_area_tiles == 0 {
        return;
    }

    for (i, id) in DECOR_STRUCTURE_IDS.iter().enumerate() {
        let Some(def) = find_structure(structures, id) else {
            continue;
        };
        let seed = FARM_DECOR_SEED
            ^ 0xBD1E_9955
            ^ ((i as u32 + 1).wrapping_mul(0xA24B_4F6D));
        scatter_structure_where(map, def, seed, inner_area_tiles, |candidate| {
            tile_rect_contains(farm_area, candidate)
        });
    }
}

fn place_farm_bush_border(map: &mut TileMap, structures: &[StructureDef], area: TileRect) {
    if area.w == 0 || area.h == 0 {
        return;
    }

    let x0 = area.x;
    let y0 = area.y;
    let x1 = area.max_x().saturating_sub(1);
    let y1 = area.max_y().saturating_sub(1);

    let has_bush = find_structure(structures, "bush_plains").is_some();
    if has_bush {
        for x in x0..=x1 {
            place_structure_from_defs(map, structures, "bush_plains", x, y0);
            place_structure_from_defs(map, structures, "bush_plains", x, y1);
        }
        for y in (y0 + 1)..y1 {
            place_structure_from_defs(map, structures, "bush_plains", x0, y);
            place_structure_from_defs(map, structures, "bush_plains", x1, y);
        }
        return;
    }

    for x in x0..=x1 {
        map.set_collision(x, y0, true);
        map.set_collision(x, y1, true);
    }
    for y in (y0 + 1)..y1 {
        map.set_collision(x0, y, true);
        map.set_collision(x1, y, true);
    }
}

fn scatter_structure_where<F>(
    map: &mut TileMap,
    def: &StructureDef,
    seed: u32,
    area_tiles: usize,
    mut allow: F,
) -> usize
where
    F: FnMut(TileRect) -> bool,
{
    let sw = def.structure.width();
    let sh = def.structure.height();
    if sw == 0 || sh == 0 || sw > map.width() || sh > map.height() {
        return 0;
    }

    let freq = def.frequency.clamp(0.0, 1.0);
    if freq <= 0.0 || def.max_per_map == 0 {
        return 0;
    }

    let target = ((area_tiles as f32 * freq * SCENE_DECOR_DENSITY_SCALE).round() as usize)
        .min(def.max_per_map)
        .min(SCENE_DECOR_MAX_PER_DEF);
    if target == 0 {
        return 0;
    }

    let max_x = map.width() - sw;
    let max_y = map.height() - sh;
    let attempts = (target * 18).max(64);
    let tile_size = map.tile_size();
    let min_distance = def.min_distance.max(0.0);
    let mut placed = 0usize;
    let mut placed_rects: Vec<Rect> = Vec::with_capacity(target.min(512));

    for i in 0..attempts {
        if placed >= target {
            break;
        }

        let x = (hash_u32(i as u32, seed, 11) as usize) % (max_x + 1);
        let y = (hash_u32(i as u32, seed, 37) as usize) % (max_y + 1);
        let rect = TileRect { x, y, w: sw, h: sh };
        if !allow(rect) {
            continue;
        }
        if structure_footprint_blocked(map, rect) {
            continue;
        }

        let world = tile_rect_to_world_rect(rect, tile_size);
        let padded = if min_distance > 0.0 {
            Rect::new(
                world.x - min_distance,
                world.y - min_distance,
                world.w + min_distance * 2.0,
                world.h + min_distance * 2.0,
            )
        } else {
            world
        };
        if placed_rects.iter().any(|other| other.overlaps(&padded)) {
            continue;
        }

        map.place_structure_def(def, x, y);
        placed_rects.push(padded);
        placed += 1;
    }

    placed
}

fn structure_footprint_blocked(map: &TileMap, rect: TileRect) -> bool {
    for y in rect.y..rect.max_y() {
        for x in rect.x..rect.max_x() {
            if map.is_solid(x, y)
                || map.tile_at(LayerKind::Foreground, x, y) != u8::MAX
                || map.tile_at(LayerKind::Overlay, x, y) != u8::MAX
            {
                return true;
            }
        }
    }
    false
}

fn farm_core_rect() -> TileRect {
    TileRect {
        x: FARM_OUTER_MARGIN,
        y: FARM_OUTER_MARGIN,
        w: FARM_WIDTH,
        h: FARM_HEIGHT,
    }
}

fn inset_tile_rect(rect: TileRect, inset: usize) -> TileRect {
    let doubled = inset.saturating_mul(2);
    let w = rect.w.saturating_sub(doubled);
    let h = rect.h.saturating_sub(doubled);
    if w == 0 || h == 0 {
        return rect;
    }
    TileRect {
        x: rect.x + inset,
        y: rect.y + inset,
        w,
        h,
    }
}

fn tile_rect_intersects(a: TileRect, b: TileRect) -> bool {
    a.x < b.max_x() && a.max_x() > b.x && a.y < b.max_y() && a.max_y() > b.y
}

fn tile_rect_contains(outer: TileRect, inner: TileRect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.max_x() <= outer.max_x()
        && inner.max_y() <= outer.max_y()
}

fn tile_rect_to_world_rect(rect: TileRect, tile_size: f32) -> Rect {
    Rect::new(
        rect.x as f32 * tile_size,
        rect.y as f32 * tile_size,
        rect.w as f32 * tile_size,
        rect.h as f32 * tile_size,
    )
}

fn find_structure<'a>(structures: &'a [StructureDef], id: &str) -> Option<&'a StructureDef> {
    structures.iter().find(|def| def.id == id)
}

fn hash_u32(x: u32, seed: u32, salt: u32) -> u32 {
    let mut v = x
        .wrapping_mul(0x9E37_79B1)
        ^ seed.wrapping_mul(0x85EB_CA6B)
        ^ salt.wrapping_mul(0xC2B2_AE35);
    v ^= v >> 16;
    v = v.wrapping_mul(0x7FEB_352D);
    v ^= v >> 15;
    v
}

fn load_farm_snapshot() -> Option<TileMapSnapshot> {
    let json = load_farm_snapshot_json()?;
    serde_json::from_str(&json).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn farm_save_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(std::path::PathBuf::from(home).join(".cropbots").join("farm.json"))
}

#[cfg(not(target_arch = "wasm32"))]
fn save_farm_snapshot_json(json: &str) -> bool {
    let Some(path) = farm_save_path() else {
        return false;
    };
    let Some(parent) = path.parent() else {
        return false;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return false;
    }
    std::fs::write(path, json.as_bytes()).is_ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn load_farm_snapshot_json() -> Option<String> {
    let path = farm_save_path()?;
    std::fs::read_to_string(path).ok()
}

#[cfg(target_arch = "wasm32")]
fn save_farm_snapshot_json(json: &str) -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return false;
    };
    storage.set_item(FARM_STORAGE_KEY, json).is_ok()
}

#[cfg(target_arch = "wasm32")]
fn load_farm_snapshot_json() -> Option<String> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok().flatten()?;
    storage.get_item(FARM_STORAGE_KEY).ok().flatten()
}
