use macroquad::prelude::*;

use crate::entity::{Entity, EntityDatabase, MovementRegistry};
use crate::map::{LayerKind, StructureDef, TileMap, TileMapSnapshot};

pub const EXPEDITION_WIDTH: usize = 256;
pub const EXPEDITION_HEIGHT: usize = 256;
pub const FARM_WIDTH: usize = 100;
pub const FARM_HEIGHT: usize = 50;

const FARM_OUTER_MARGIN: usize = 128;
const FARM_MAP_WIDTH: usize = FARM_WIDTH + FARM_OUTER_MARGIN * 2;
const FARM_MAP_HEIGHT: usize = FARM_HEIGHT + FARM_OUTER_MARGIN * 2;

const FARM_DECOR_SEED: u32 = 0xA531_2D91;
const DECOR_STRUCTURE_IDS: [&str; 2] = ["tree_plains", "bush_plains"];
const SCENE_DECOR_DENSITY_SCALE: f32 = 0.75;
const SCENE_DECOR_MAX_PER_DEF: usize = 1200;
const EXPEDITION_DUNGEON_SEED: u32 = 0xD06E_0B07;
const EXPEDITION_WALL_TILE: u8 = 225;
pub const EXPEDITION_FLOOR_TILE: u8 = 226;
const EXPEDITION_DUNGEON_ROOM_TARGET: usize = 140;
const EXPEDITION_DUNGEON_MARGIN: usize = 8;
const EXPEDITION_HALL_LENGTH: usize = 3;
const EXPEDITION_ROOM_SIZES: [(usize, usize); 12] = [
    (5, 4),
    (4, 5),
    (5, 5),
    (5, 6),
    (6, 5),
    (6, 6),
    (6, 7),
    (7, 6),
    (7, 7),
    (7, 8),
    (8, 7),
    (8, 8),
];

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

#[derive(Clone, Copy)]
enum Direction {
    North,
    East,
    South,
    West,
}

#[derive(Clone, Copy)]
struct PendingRoom {
    room: TileRect,
    hall: TileRect,
}

struct DungeonLayout {
    rooms: Vec<TileRect>,
    edges: Vec<(usize, usize)>,
    halls: Vec<TileRect>,
}

#[derive(Clone, Copy)]
struct SpawnArea {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

impl SpawnArea {
    fn from_room(room: TileRect) -> Self {
        Self {
            x0: room.x,
            y0: room.y,
            x1: room.max_x(),
            y1: room.max_y(),
        }
    }
}

pub struct SpawnRule<'a> {
    pub entity_id: &'a str,
    pub count: usize,
    pub min_spacing_tiles: f32,
}

struct DungeonRng {
    state: u32,
}

impl DungeonRng {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        self.state
    }

    fn usize(&mut self, upper: usize) -> usize {
        if upper == 0 {
            0
        } else {
            (self.next() as usize) % upper
        }
    }
}

pub fn clear_scenes(map: &mut TileMap, entities: &mut Vec<Entity>) {
    entities.clear();
    map.clear_all_tiles();
}

pub fn expedition_spawn_point() -> Vec2 {
    vec2(
        (EXPEDITION_WIDTH as f32 * 0.5 + 0.5) * 16.0,
        (EXPEDITION_HEIGHT as f32 * 0.5 + 0.5) * 16.0,
    )
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
    _ground_tile: u8,
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
    next.fill_layer(LayerKind::Background, u8::MAX);
    next.fill_layer(LayerKind::Foreground, u8::MAX);
    next.fill_collision(false);
    next.set_custom_border_hitbox(None);
    let layout = generate_expedition_dungeon(&mut next);
    place_expedition_dungeon_walls(&mut next);
    apply_dungeon_hpa_topology(&mut next, &layout);
    place_expedition_blocks(&mut next, structures, &layout.rooms, &layout.halls);
    *map = next;

    entities.clear();
    spawn_expedition_entities(map, entities, db, registry, &layout.rooms, tile_size);
}

fn place_expedition_blocks(
    map: &mut TileMap,
    structures: &[StructureDef],
    rooms: &[TileRect],
    halls: &[TileRect],
) {
    let mut rng = DungeonRng::new(EXPEDITION_DUNGEON_SEED ^ 0xE8A4_31D2);

    // --- Loot blocks: place generously in rooms ---
    let loot_candidates = ["loot_block", "spawner_block", "warp_block"];
    let loot_defs: Vec<&StructureDef> = loot_candidates
        .iter()
        .filter_map(|id| find_structure(structures, id))
        .collect();

    // Shuffle room order
    let mut room_order: Vec<usize> = (0..rooms.len()).collect();
    for i in 0..room_order.len() {
        let j = rng.usize(room_order.len());
        room_order.swap(i, j);
    }

    // Target: roughly 40% of rooms get a block, minimum 6
    let room_target = ((rooms.len() as f32 * 0.40).round() as usize).max(6);
    let mut loot_placed = 0usize;
    if !loot_defs.is_empty() {
        for &room_idx in &room_order {
            if loot_placed >= room_target {
                break;
            }
            let room = rooms[room_idx];
            if room.w < 4 || room.h < 4 {
                continue;
            }
            // 70% chance to place in any eligible room
            if rng.usize(100) >= 70 {
                continue;
            }
            let Some(interior) = room_interior(room, 1) else {
                continue;
            };
            let def = loot_defs[rng.usize(loot_defs.len())];
            let sw = def.structure.width();
            let sh = def.structure.height();
            if sw == 0 || sh == 0 || sw > interior.w || sh > interior.h {
                continue;
            }
            let max_x = interior.x + interior.w - sw;
            let max_y = interior.y + interior.h - sh;
            for _ in 0..12 {
                let x = interior.x + rng.usize((max_x - interior.x).max(1));
                let y = interior.y + rng.usize((max_y - interior.y).max(1));
                let rect = TileRect { x, y, w: sw, h: sh };
                if structure_footprint_blocked(map, rect) {
                    continue;
                }
                map.place_structure_def(def, x, y);
                loot_placed += 1;
                break;
            }
        }
        // Guarantee at least one loot block
        if loot_placed == 0 {
            'guarantee: for room in rooms.iter().copied() {
                let Some(interior) = room_interior(room, 1) else { continue };
                for def in &loot_defs {
                    let sw = def.structure.width();
                    let sh = def.structure.height();
                    if sw == 0 || sh == 0 || sw > interior.w || sh > interior.h { continue; }
                    let x = interior.x + (interior.w - sw) / 2;
                    let y = interior.y + (interior.h - sh) / 2;
                    let rect = TileRect { x, y, w: sw, h: sh };
                    if structure_footprint_blocked(map, rect) { continue; }
                    map.place_structure_def(def, x, y);
                    break 'guarantee;
                }
            }
        }
    }

    // --- Lock blocks: place in hallways as gating ---
    // Hallways are 2 wide. Use lock_block_h (2x1) for wide horizontal halls
    // and lock_block_v (1x2) for wide vertical halls.
    let lock_h = find_structure(structures, "lock_block_h");
    let lock_v = find_structure(structures, "lock_block_v");
    let lock_1x1 = find_structure(structures, "lock_block");

    // Target: about 25% of hallways get a lock block, minimum 3
    let lock_target = ((halls.len() as f32 * 0.25).round() as usize).max(3);
    let mut lock_placed = 0usize;
    let mut hall_order: Vec<usize> = (0..halls.len()).collect();
    for i in 0..hall_order.len() {
        let j = rng.usize(hall_order.len());
        hall_order.swap(i, j);
    }

    for &hi in &hall_order {
        if lock_placed >= lock_target {
            break;
        }
        let hall = halls[hi];
        if rng.usize(100) >= 60 {
            continue;
        }
        // Determine orientation: horizontal hall (w >= h) vs vertical (h > w)
        let (def, pw, ph) = if hall.w >= hall.h {
            // Horizontal hallway (passage is E-W): use 1x2 vertical block to block it
            if let Some(d) = lock_v {
                (d, 1usize, 2usize)
            } else if let Some(d) = lock_1x1 {
                (d, 1usize, 1usize)
            } else {
                continue;
            }
        } else {
            // Vertical hallway (passage is N-S): use 2x1 horizontal block to block it
            if let Some(d) = lock_h {
                (d, 2usize, 1usize)
            } else if let Some(d) = lock_1x1 {
                (d, 1usize, 1usize)
            } else {
                continue;
            }
        };
        if pw > hall.w || ph > hall.h {
            continue;
        }
        // Centre the block in the hallway
        let x = hall.x + (hall.w - pw) / 2;
        let y = hall.y + (hall.h - ph) / 2;
        let rect = TileRect { x, y, w: pw, h: ph };
        if structure_footprint_blocked(map, rect) {
            continue;
        }
        map.place_structure_def(def, x, y);
        lock_placed += 1;
    }
}

fn room_interior(room: TileRect, inset: usize) -> Option<TileRect> {
    if room.w <= inset * 2 || room.h <= inset * 2 {
        return None;
    }
    Some(TileRect {
        x: room.x + inset,
        y: room.y + inset,
        w: room.w - inset * 2,
        h: room.h - inset * 2,
    })
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

fn generate_expedition_dungeon(map: &mut TileMap) -> DungeonLayout {
    let mut rng = DungeonRng::new(EXPEDITION_DUNGEON_SEED);
    let mut rooms = Vec::with_capacity(EXPEDITION_DUNGEON_ROOM_TARGET);
    let mut edges = Vec::with_capacity(EXPEDITION_DUNGEON_ROOM_TARGET * 2);
    let mut halls = Vec::with_capacity(EXPEDITION_DUNGEON_ROOM_TARGET * 2);
    let start = TileRect {
        x: EXPEDITION_WIDTH / 2 - 2,
        y: EXPEDITION_HEIGHT / 2 - 2,
        w: 5,
        h: 5,
    };
    carve_floor_rect(map, start);
    rooms.push(start);

    let max_attempts = EXPEDITION_DUNGEON_ROOM_TARGET * 400;
    for _ in 0..max_attempts {
        if rooms.len() >= EXPEDITION_DUNGEON_ROOM_TARGET {
            break;
        }

        let base_idx = rng.usize(rooms.len());
        let base = rooms[base_idx];
        let direction = match rng.usize(4) {
            0 => Direction::North,
            1 => Direction::East,
            2 => Direction::South,
            _ => Direction::West,
        };
        let (w, h) = EXPEDITION_ROOM_SIZES[rng.usize(EXPEDITION_ROOM_SIZES.len())];
        let Some(candidate) = pending_room_from(base, direction, w, h) else {
            continue;
        };
        if !room_candidate_fits(map, candidate, &rooms) {
            continue;
        }

        carve_floor_rect(map, candidate.hall);
        carve_floor_rect(map, candidate.room);
        let next_idx = rooms.len();
        rooms.push(candidate.room);
        halls.push(candidate.hall);
        edges.push((base_idx, next_idx));
    }

    DungeonLayout { rooms, edges, halls }
}

fn apply_dungeon_hpa_topology(map: &mut TileMap, layout: &DungeonLayout) {
    let _ = layout;
    const NODE_SIZE: usize = 5;
    let w = map.width();
    let h = map.height();
    let len = w * h;
    let mut room_ids = vec![-1i16; len];
    let mut centers = Vec::new();
    let mut node_lookup: Vec<Option<usize>> = vec![None; len];

    for y0 in (0..h).step_by(NODE_SIZE) {
        for x0 in (0..w).step_by(NODE_SIZE) {
            let x1 = (x0 + NODE_SIZE).min(w);
            let y1 = (y0 + NODE_SIZE).min(h);
            let mut tiles = Vec::new();
            for y in y0..y1 {
                for x in x0..x1 {
                    if map.tile_at(LayerKind::Background, x, y) == EXPEDITION_FLOOR_TILE
                        && !map.is_solid(x, y)
                    {
                        tiles.push((x, y));
                    }
                }
            }
            if tiles.is_empty() {
                continue;
            }

            let room_idx = centers.len();
            let (cx, cy) = tiles[tiles.len() / 2];
            centers.push(crate::map::GridIndex {
                x: cx as i32,
                y: cy as i32,
            });
            for (x, y) in tiles {
                let idx = y * w + x;
                room_ids[idx] = room_idx as i16;
                node_lookup[idx] = Some(room_idx);
            }
        }
    }

    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); centers.len()];
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let Some(a) = node_lookup[idx] else {
                continue;
            };
            if x + 1 < w {
                if let Some(b) = node_lookup[y * w + (x + 1)] {
                    if a != b {
                        adjacency[a].push(b);
                        adjacency[b].push(a);
                    }
                }
            }
            if y + 1 < h {
                if let Some(b) = node_lookup[(y + 1) * w + x] {
                    if a != b {
                        adjacency[a].push(b);
                        adjacency[b].push(a);
                    }
                }
            }
        }
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable();
        neighbors.dedup();
    }
    map.set_hpa_topology(room_ids, centers, adjacency);
}

fn pending_room_from(
    base: TileRect,
    direction: Direction,
    room_w: usize,
    room_h: usize,
) -> Option<PendingRoom> {
    let hall_width = match direction {
        Direction::North | Direction::South => hallway_width_for_side(base.w),
        Direction::East | Direction::West => hallway_width_for_side(base.h),
    };

    match direction {
        Direction::North => {
            if base.y < EXPEDITION_HALL_LENGTH + room_h {
                return None;
            }
            let hall_x = centered_span_start(base.x, base.w, hall_width)?;
            let room_x = centered_room_start(hall_x, hall_width, room_w)?;
            Some(PendingRoom {
                room: TileRect {
                    x: room_x,
                    y: base.y - EXPEDITION_HALL_LENGTH - room_h,
                    w: room_w,
                    h: room_h,
                },
                hall: TileRect {
                    x: hall_x,
                    y: base.y - EXPEDITION_HALL_LENGTH,
                    w: hall_width,
                    h: EXPEDITION_HALL_LENGTH,
                },
            })
        }
        Direction::East => {
            let hall_y = centered_span_start(base.y, base.h, hall_width)?;
            let room_y = centered_room_start(hall_y, hall_width, room_h)?;
            Some(PendingRoom {
                room: TileRect {
                    x: base.max_x() + EXPEDITION_HALL_LENGTH,
                    y: room_y,
                    w: room_w,
                    h: room_h,
                },
                hall: TileRect {
                    x: base.max_x(),
                    y: hall_y,
                    w: EXPEDITION_HALL_LENGTH,
                    h: hall_width,
                },
            })
        }
        Direction::South => {
            let hall_x = centered_span_start(base.x, base.w, hall_width)?;
            let room_x = centered_room_start(hall_x, hall_width, room_w)?;
            Some(PendingRoom {
                room: TileRect {
                    x: room_x,
                    y: base.max_y() + EXPEDITION_HALL_LENGTH,
                    w: room_w,
                    h: room_h,
                },
                hall: TileRect {
                    x: hall_x,
                    y: base.max_y(),
                    w: hall_width,
                    h: EXPEDITION_HALL_LENGTH,
                },
            })
        }
        Direction::West => {
            if base.x < EXPEDITION_HALL_LENGTH + room_w {
                return None;
            }
            let hall_y = centered_span_start(base.y, base.h, hall_width)?;
            let room_y = centered_room_start(hall_y, hall_width, room_h)?;
            Some(PendingRoom {
                room: TileRect {
                    x: base.x - EXPEDITION_HALL_LENGTH - room_w,
                    y: room_y,
                    w: room_w,
                    h: room_h,
                },
                hall: TileRect {
                    x: base.x - EXPEDITION_HALL_LENGTH,
                    y: hall_y,
                    w: EXPEDITION_HALL_LENGTH,
                    h: hall_width,
                },
            })
        }
    }
}

fn hallway_width_for_side(side_len: usize) -> usize {
    let _ = side_len;
    2
}

fn centered_span_start(outer_start: usize, outer_len: usize, inner_len: usize) -> Option<usize> {
    if inner_len > outer_len {
        None
    } else {
        Some(outer_start + (outer_len - inner_len) / 2)
    }
}

fn centered_room_start(door_start: usize, door_len: usize, room_len: usize) -> Option<usize> {
    let door_mid = door_start + door_len / 2;
    let room_offset = room_len / 2;
    door_mid.checked_sub(room_offset)
}

fn room_candidate_fits(map: &TileMap, candidate: PendingRoom, rooms: &[TileRect]) -> bool {
    rect_within_map(candidate.room, map.width(), map.height())
        && rect_within_map(candidate.hall, map.width(), map.height())
        && candidate.room.x >= EXPEDITION_DUNGEON_MARGIN
        && candidate.room.y >= EXPEDITION_DUNGEON_MARGIN
        && candidate.room.max_x() + EXPEDITION_DUNGEON_MARGIN < map.width()
        && candidate.room.max_y() + EXPEDITION_DUNGEON_MARGIN < map.height()
        && rooms.iter().all(|&room| {
            !tile_rect_intersects(expand_tile_rect(room, 1), candidate.room)
                && !tile_rect_intersects(room, candidate.hall)
        })
}

fn rect_within_map(rect: TileRect, map_w: usize, map_h: usize) -> bool {
    rect.w > 0 && rect.h > 0 && rect.max_x() <= map_w && rect.max_y() <= map_h
}

fn expand_tile_rect(rect: TileRect, amount: usize) -> TileRect {
    let x = rect.x.saturating_sub(amount);
    let y = rect.y.saturating_sub(amount);
    TileRect {
        x,
        y,
        w: rect.w + (rect.x - x) + amount,
        h: rect.h + (rect.y - y) + amount,
    }
}

fn carve_floor_rect(map: &mut TileMap, rect: TileRect) {
    for y in rect.y..rect.max_y() {
        for x in rect.x..rect.max_x() {
            map.set_tile(LayerKind::Background, x, y, EXPEDITION_FLOOR_TILE);
            map.set_tile(LayerKind::Foreground, x, y, u8::MAX);
            map.set_collision(x, y, false);
        }
    }
}

fn place_expedition_dungeon_walls(map: &mut TileMap) {
    let mut walls = Vec::new();
    for y in 0..map.height() {
        for x in 0..map.width() {
            if map.tile_at(LayerKind::Background, x, y) == EXPEDITION_FLOOR_TILE {
                continue;
            }
            if neighbors_expedition_floor(map, x, y) {
                walls.push((x, y));
            }
        }
    }

    for (x, y) in walls {
        map.set_tile(LayerKind::Foreground, x, y, EXPEDITION_WALL_TILE);
        map.set_dungeon_wall_collision(x, y, true);
    }
}

fn neighbors_expedition_floor(map: &TileMap, x: usize, y: usize) -> bool {
    let min_x = x.saturating_sub(1);
    let min_y = y.saturating_sub(1);
    let max_x = (x + 1).min(map.width().saturating_sub(1));
    let max_y = (y + 1).min(map.height().saturating_sub(1));

    for ny in min_y..=max_y {
        for nx in min_x..=max_x {
            if nx == x && ny == y {
                continue;
            }
            if map.tile_at(LayerKind::Background, nx, ny) == EXPEDITION_FLOOR_TILE {
                return true;
            }
        }
    }

    false
}

fn spawn_expedition_entities(
    map: &TileMap,
    entities: &mut Vec<Entity>,
    db: &EntityDatabase,
    registry: &MovementRegistry,
    rooms: &[TileRect],
    tile_size: f32,
) {
    let mut rng = DungeonRng::new(EXPEDITION_DUNGEON_SEED ^ 0x54A3_11C5);
    let areas: Vec<SpawnArea> = rooms.iter().copied().map(SpawnArea::from_room).collect();
    let rules = [
        SpawnRule {
            entity_id: "virabird",
            count: 5,
            min_spacing_tiles: 1.5,
        },
        SpawnRule {
            entity_id: "virat",
            count: 2,
            min_spacing_tiles: 2.0,
        },
        SpawnRule {
            entity_id: "chopbot",
            count: 2,
            min_spacing_tiles: 2.0,
        },
        SpawnRule {
            entity_id: "cropbot",
            count: 1,
            min_spacing_tiles: 3.0,
        },
    ];
    spawn_entities_with_rules(map, entities, db, registry, &areas, &rules, tile_size, &mut rng);
}

fn spawn_entities_with_rules(
    map: &TileMap,
    entities: &mut Vec<Entity>,
    db: &EntityDatabase,
    registry: &MovementRegistry,
    areas: &[SpawnArea],
    rules: &[SpawnRule<'_>],
    tile_size: f32,
    rng: &mut DungeonRng,
) {
    if areas.is_empty() || rules.is_empty() {
        return;
    }

    let mut taken_positions: Vec<Vec2> = Vec::with_capacity(rules.iter().map(|r| r.count).sum());
    for rule in rules {
        let min_sq = (rule.min_spacing_tiles.max(0.0) * tile_size).powi(2);
        let attempts = (rule.count * 24).max(32);
        let mut spawned = 0usize;
        for _ in 0..attempts {
            if spawned >= rule.count {
                break;
            }
            let area = areas[rng.usize(areas.len())];
            if area.x0 >= area.x1 || area.y0 >= area.y1 {
                continue;
            }
            let x = area.x0 + rng.usize(area.x1 - area.x0);
            let y = area.y0 + rng.usize(area.y1 - area.y0);
            if map.is_solid(x, y) {
                continue;
            }
            let pos = vec2((x as f32 + 0.5) * tile_size, (y as f32 + 0.5) * tile_size);
            if min_sq > 0.0
                && taken_positions
                    .iter()
                    .any(|other| other.distance_squared(pos) < min_sq)
            {
                continue;
            }
            if let Some(entity) = Entity::spawn(db, rule.entity_id, pos, registry) {
                entities.push(entity);
                taken_positions.push(pos);
                spawned += 1;
            }
        }
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
        let seed = FARM_DECOR_SEED ^ 0xBD1E_9955 ^ ((i as u32 + 1).wrapping_mul(0xA24B_4F6D));
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
    let mut v = x.wrapping_mul(0x9E37_79B1)
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
    Some(
        std::path::PathBuf::from(home)
            .join(".cropbots")
            .join("farm.json"),
    )
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
    wasm_storage_set_item(FARM_STORAGE_KEY, json)
}

#[cfg(target_arch = "wasm32")]
fn load_farm_snapshot_json() -> Option<String> {
    wasm_storage_get_item(FARM_STORAGE_KEY)
}

#[cfg(target_arch = "wasm32")]
fn wasm_storage_set_item(key: &str, value: &str) -> bool {
    let key_bytes = key.as_bytes();
    let value_bytes = value.as_bytes();
    unsafe {
        mq_storage_set_item(
            key_bytes.as_ptr(),
            key_bytes.len(),
            value_bytes.as_ptr(),
            value_bytes.len(),
        ) != 0
    }
}

#[cfg(target_arch = "wasm32")]
fn wasm_storage_get_item(key: &str) -> Option<String> {
    let key_bytes = key.as_bytes();
    let len = unsafe { mq_storage_get_item_len(key_bytes.as_ptr(), key_bytes.len()) };
    if len < 0 {
        return None;
    }

    let mut buf = vec![0u8; len as usize];
    let written = unsafe {
        mq_storage_get_item(
            key_bytes.as_ptr(),
            key_bytes.len(),
            buf.as_mut_ptr(),
            buf.len(),
        )
    };
    if written < 0 {
        return None;
    }
    let written = written as usize;
    if written > buf.len() {
        return None;
    }
    buf.truncate(written);
    String::from_utf8(buf).ok()
}

#[cfg(target_arch = "wasm32")]
unsafe extern "C" {
    fn mq_storage_set_item(
        key_ptr: *const u8,
        key_len: usize,
        value_ptr: *const u8,
        value_len: usize,
    ) -> i32;

    fn mq_storage_get_item_len(key_ptr: *const u8, key_len: usize) -> i32;

    fn mq_storage_get_item(
        key_ptr: *const u8,
        key_len: usize,
        out_ptr: *mut u8,
        out_len: usize,
    ) -> i32;
}
