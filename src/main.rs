use image::imageops::FilterType;
use macroquad::prelude::*;
use miniquad::conf::{Icon, Platform};
use std::collections::HashMap;
use std::future::poll_fn;
use std::task::Poll;

mod building;
mod cogs;
mod entity;
mod farm;
mod helpers;
mod interact;
mod inventory;
mod item;
mod map;
mod notebook;
mod particle;
mod player;
mod puzzle;
mod loot;
mod scene;
mod sound;
mod tilemap;
mod r#trait;

use building::BuildingMode;
use cogs::CogWallet;
use helpers::resolution_ui_scale;
use notebook::ProgrammingNotebook;

use entity::{
    DamageEvent, Entity, EntityContext, EntityDatabase, MovementRegistry, PlayerTarget, Target,
};
use map::{TileMap, TileSet, load_structures_from_dir};
use player::Player;

use interact::{InteractAction, InteractContext, InteractRegistry};
use inventory::StackInventory;
use item::{InventoryCatalog, ItemId, draw_item_icon};
use loot::LootTables;
use puzzle::PuzzleCatalog;
use particle::ParticleSystem;
use scene::SceneKind;
use sound::SoundSystem;

const CAMERA_DRAG: f32 = 5.0;
const TILE_SIZE: f32 = 16.0;
const MOVE_DEADZONE: f32 = 16.0;
const FOOTSTEP_INTERVAL: f32 = 0.2;
const CAMERA_FOV: f32 = 300.0;
const ENTITY_CULL_FADE_PAD: f32 = 96.0;
const LOADING_SPIN_SPEED: f32 = 3.0;
const CHUNK_ALLOC_PER_FRAME: usize = 6;
const CHUNK_REBUILD_PER_FRAME: usize = 8;
const SCENE_WARM_BUDGET_S: f32 = 0.006;

#[derive(Default)]
struct BlockRuntimeState {
    spawners: HashMap<String, SpawnerRuntime>,
    loot_chests: HashMap<String, StackInventory>,
    open_loot_key: Option<String>,
    active_lock_area: Option<Rect>,
    active_spawner_room_lock: Option<Rect>,
    virabird_bullet_cd: HashMap<u64, f32>,
    virabird_bullet_ttl: HashMap<u64, f32>,
}

#[derive(Clone, Copy)]
struct SpawnerRuntime {
    remaining: u32,
    cooldown: f32,
}

struct LootUiState {
    loot: StackInventory,
}

struct RunState {
    timer: f32,
    enemies_defeated: u32,
    start_inventory: StackInventory,
    completed: Option<bool>, // Some(true)=victory, Some(false)=corrupted
}

fn window_conf() -> Conf {
    let icon = load_window_icon(&helpers::asset_path("src/assets/favicon.png"));
    Conf {
        window_title: "cropbots".to_owned(),
        icon,
        sample_count: 1,
        platform: Platform {
            linux_wm_class: "cropbots",
            webgl_version: miniquad::conf::WebGLVersion::WebGL2,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn load_window_icon(path: &str) -> Option<Icon> {
    if cfg!(target_arch = "wasm32") {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.to_rgba8();

    fn resize_rgba(image: &image::RgbaImage, size: u32) -> Option<Vec<u8>> {
        let resized = image::imageops::resize(image, size, size, FilterType::Nearest);
        let raw = resized.into_raw();
        if raw.len() != (size as usize * size as usize * 4) {
            return None;
        }
        Some(raw)
    }

    let small: [u8; 16 * 16 * 4] = resize_rgba(&image, 16)?.try_into().ok()?;
    let medium: [u8; 32 * 32 * 4] = resize_rgba(&image, 32)?.try_into().ok()?;
    let big: [u8; 64 * 64 * 4] = resize_rgba(&image, 64)?.try_into().ok()?;

    Some(Icon { small, medium, big })
}

async fn show_loading(loading: &Texture2D, label: &str, progress: f32, spin: f32) {
    let pct = (progress.clamp(0.0, 1.0) * 100.0).round();
    let size = loading.size();
    let scale = (screen_height() * 0.075).max(32.0) / size.y.max(1.0);
    let draw_w = size.x * scale;
    let draw_h = size.y * scale;
    let pos = vec2(
        (screen_width() - draw_w) * 0.5,
        (screen_height() - draw_h) * 0.5,
    );

    set_default_camera();
    clear_background(BLACK);
    draw_texture_ex(
        loading,
        pos.x,
        pos.y,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(draw_w, draw_h)),
            rotation: spin,
            pivot: Some(vec2(pos.x + draw_w * 0.5, pos.y + draw_h * 0.5)),
            ..Default::default()
        },
    );
    draw_text(&format!("{label} {pct:.0}%"), 20.0, 40.0, 30.0, WHITE);
    next_frame().await;
}

async fn await_with_loading<F, T>(
    future: F,
    loading: &Texture2D,
    label: &str,
    progress: f32,
    spin: &mut f32,
) -> T
where
    F: std::future::Future<Output = T>,
{
    let mut future = std::pin::pin!(future);
    loop {
        let polled = poll_fn(|cx| Poll::Ready(future.as_mut().poll(cx))).await;
        match polled {
            Poll::Ready(value) => return value,
            Poll::Pending => {
                *spin += LOADING_SPIN_SPEED * get_frame_time();
                show_loading(loading, label, progress, *spin).await;
            }
        }
    }
}

async fn warm_scene_chunks_loading(
    map: &mut TileMap,
    tileset: &TileSet,
    loading: &Texture2D,
    label: &str,
    loading_spin: &mut f32,
) {
    loop {
        let done = map.warm_all_chunks_step(tileset, SCENE_WARM_BUDGET_S);
        let progress = map.warm_all_chunks_progress();
        *loading_spin += LOADING_SPIN_SPEED * get_frame_time();
        show_loading(loading, label, progress, *loading_spin).await;
        if done {
            break;
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let loading = load_texture(&helpers::asset_path("src/assets/loading.png"))
        .await
        .unwrap_or_else(|_| Texture2D::empty());
    loading.set_filter(FilterMode::Nearest);
    let mut loading_spin = 0.0f32;
    loading_spin += LOADING_SPIN_SPEED * get_frame_time();
    show_loading(&loading, "Loading", 0.0, loading_spin).await;

    // Load the tileset atlas (tileset.json + tileset.png)
    let tileset = await_with_loading(
        TileSet::load("src/assets/tileset.json", "src/assets/tileset.png"),
        &loading,
        "Loading",
        0.15,
        &mut loading_spin,
    )
    .await
    .unwrap_or_else(|err| {
        eprintln!("tileset load failed: {err}");
        eprintln!("Please ensure src/assets/tileset.json and src/assets/tileset.png exist");
        panic!("Tileset loading failed");
    });
    let grass: u8 = if tileset.count() > 24 { 24 } else { 0 };
    loading_spin += LOADING_SPIN_SPEED * get_frame_time();
    show_loading(&loading, "Loading", 0.22, loading_spin).await;
    loading_spin += LOADING_SPIN_SPEED * get_frame_time();
    show_loading(&loading, "Loading", 0.35, loading_spin).await;

    // Load structures from JSON and apply them with a fixed seed.
    let structures = await_with_loading(
        load_structures_from_dir("src/structure"),
        &loading,
        "Loading",
        0.45,
        &mut loading_spin,
    )
    .await
    .unwrap_or_else(|err| {
        eprintln!("structure load failed: {err}");
        Vec::new()
    });
    loading_spin += LOADING_SPIN_SPEED * get_frame_time();
    show_loading(&loading, "Loading", 0.55, loading_spin).await;

    // Player
    let player_texture = await_with_loading(
        helpers::load_single_texture("src/assets/objects", "player08"),
        &loading,
        "Loading",
        0.6,
        &mut loading_spin,
    )
    .await
    .unwrap_or_else(Texture2D::empty);
    loading_spin += LOADING_SPIN_SPEED * get_frame_time();
    show_loading(&loading, "Loading", 0.65, loading_spin).await;
    let mut player = Player::new(
        vec2(200.0, 300.0 + 16.0 / 2.0),
        player_texture,
        Rect::new(-6.5 / 2.0, -8.0, 6.5, 8.0),
    );
    loading_spin += LOADING_SPIN_SPEED * get_frame_time();
    show_loading(&loading, "Loading", 0.68, loading_spin).await;

    let heart_full = load_texture(&helpers::asset_path("src/assets/ui/heart.png"))
        .await
        .unwrap_or_else(|_| Texture2D::empty());
    let heart_empty = load_texture(&helpers::asset_path("src/assets/ui/heart-empty.png"))
        .await
        .unwrap_or_else(|_| Texture2D::empty());
    let hotbar_slot = load_texture(&helpers::asset_path("src/assets/ui/hotbar-slot.png"))
        .await
        .unwrap_or_else(|_| Texture2D::empty());
    heart_full.set_filter(FilterMode::Nearest);
    heart_empty.set_filter(FilterMode::Nearest);
    hotbar_slot.set_filter(FilterMode::Nearest);
    let inventory_catalog = await_with_loading(
        InventoryCatalog::load_from("src/items"),
        &loading,
        "Loading items",
        0.74,
        &mut loading_spin,
    )
    .await
    .unwrap_or_else(|err| {
        eprintln!("item load failed: {err}");
        InventoryCatalog::new()
    });
    let mut inventory = StackInventory::seed_demo(&inventory_catalog);
    let loot_tables = await_with_loading(
        LootTables::load_from("src/loot"),
        &loading,
        "Loading loot",
        0.76,
        &mut loading_spin,
    )
    .await
    .unwrap_or_else(|err| {
        eprintln!("loot table load failed: {err}");
        LootTables::empty()
    });
    let puzzle_catalog = await_with_loading(
        PuzzleCatalog::load_from("src/puzzle"),
        &loading,
        "Loading puzzles",
        0.78,
        &mut loading_spin,
    )
    .await
    .unwrap_or_else(|err| {
        eprintln!("puzzle load failed: {err}");
        PuzzleCatalog::empty()
    });
    let chopbot_summon_item = inventory_catalog.id_by_key("chopbot_summon");
    let cropbot_summon_item = inventory_catalog.id_by_key("cropbot_summon");
    let virat_summon_item = inventory_catalog.id_by_key("virat_summon");
    let virabird_summon_item = inventory_catalog.id_by_key("virabird_summon");

    // Camera
    let mut camera = Camera2D {
        target: player.position(),
        zoom: vec2(1.0, 1.0),
        ..Default::default()
    };

    let mut cogs = CogWallet::new(0);
    let mut building = BuildingMode::from_structures(&structures);

    let use_render_target = false;
    let render_scale = 0.5;
    let mut scene_target = create_scene_target(render_scale, screen_width(), screen_height());
    let mut last_screen_width = screen_width();
    let mut last_screen_height = screen_height();
    camera.zoom = camera_zoom_for_fov(CAMERA_FOV, use_render_target);
    camera.render_target = if use_render_target {
        Some(scene_target.clone())
    } else {
        None
    };

    // Entity registry
    let registry = MovementRegistry::new();
    let db = await_with_loading(
        EntityDatabase::load_from("src/entity"),
        &loading,
        "Loading",
        0.7,
        &mut loading_spin,
    )
    .await
    .unwrap_or_else(|err| {
        eprintln!("entity load failed: {err}");
        EntityDatabase::empty()
    });
    loading_spin += LOADING_SPIN_SPEED * get_frame_time();
    show_loading(&loading, "Loading", 0.75, loading_spin).await;
    let mut maps = TileMap::new_deferred(1, 1, TILE_SIZE, Vec2::new(TILE_SIZE, TILE_SIZE), 0.0);
    let mut entities = Vec::<Entity>::new();
    scene::scene_expedition(
        &mut maps,
        &mut entities,
        &db,
        &registry,
        &structures,
        grass,
        TILE_SIZE,
        CHUNK_ALLOC_PER_FRAME,
        CHUNK_REBUILD_PER_FRAME,
    );
    player.set_position(scene::expedition_spawn_point());
    player.clamp_to_map(&maps);
    let mut current_scene = SceneKind::Expedition;
    let mut run_state = Some(RunState {
        timer: 300.0,
        enemies_defeated: 0,
        start_inventory: inventory.clone(),
        completed: None,
    });

    let mut draw_order: Vec<usize> = Vec::new();

    // Particle system
    let mut particles = await_with_loading(
        ParticleSystem::load_from("src/particle"),
        &loading,
        "Loading",
        0.8,
        &mut loading_spin,
    )
    .await
    .unwrap_or_else(|err| {
        eprintln!("particle load failed: {err}");
        ParticleSystem::empty()
    });
    loading_spin += LOADING_SPIN_SPEED * get_frame_time();
    show_loading(&loading, "Loading", 0.85, loading_spin).await;
    let mut walk_trail = particles.emitter("dust_trail", player.position());
    let mut dash_trail = particles.emitter("dash_afterimage", player.position());

    // Load sounds
    let sounds = await_with_loading(
        SoundSystem::load_from("src/sound"),
        &loading,
        "Loading sounds",
        0.9,
        &mut loading_spin,
    )
    .await
    .unwrap_or_else(|err| {
        eprintln!("sound load failed: {err}");
        SoundSystem::empty()
    });
    loading_spin += LOADING_SPIN_SPEED * get_frame_time();
    show_loading(&loading, "Loading", 0.98, loading_spin).await;

    let mut footstep_timer = 0.0f32;
    let mut damage_events: Vec<DamageEvent> = Vec::new();
    let mut entity_target_cache: HashMap<(u64, u8), Option<entity::EntityTarget>> = HashMap::new();
    let mut player_dead = false;
    let interact_registry = InteractRegistry::new();
    let mut block_state = BlockRuntimeState::default();
    let mut notebook = ProgrammingNotebook::new();
    let mut selected_cropbot_uid: Option<u64> = None;

    loop {
        let dt = get_frame_time();

        let mut do_victory_warp = false;
        if let Some(state) = &run_state {
            if state.completed.is_some() {
                let bounds = loot_ui_bounds();
                let btn_text = "Return to Farm";
                let btn_size = measure_text(btn_text, None, 30, 1.0);
                let btn_rect = Rect::new(bounds.x + (bounds.w - btn_size.width - 40.0) * 0.5, bounds.y + bounds.h - 80.0, btn_size.width + 40.0, 50.0);
                let mouse = vec2(mouse_position().0, mouse_position().1);
                if btn_rect.contains(mouse) && is_mouse_button_pressed(MouseButton::Left) {
                    do_victory_warp = true;
                }
            }
        }
        if do_victory_warp {
            current_scene = apply_scene_warp(
                SceneKind::Farm,
                current_scene,
                &mut maps,
                &mut entities,
                &db,
                &registry,
                &structures,
                grass,
                &tileset,
                &mut player,
                &loading,
                &mut loading_spin,
            ).await;
            run_state = None;
            continue;
        }

        // Check for resolution changes and recreate render target if needed
        if use_render_target {
            let current_width = screen_width();
            let current_height = screen_height();
            if current_width != last_screen_width || current_height != last_screen_height {
                scene_target = create_scene_target(render_scale, current_width, current_height);
                last_screen_width = current_width;
                last_screen_height = current_height;
            }
        }

        if is_key_pressed(KeyCode::F1) && current_scene != SceneKind::Expedition {
            if current_scene == SceneKind::Farm {
                let _ = scene::save_farm_scene(&maps);
            }
            loading_spin += LOADING_SPIN_SPEED * get_frame_time();
            show_loading(&loading, "Loading Expedition", 0.1, loading_spin).await;
            scene::scene_expedition(
                &mut maps,
                &mut entities,
                &db,
                &registry,
                &structures,
                grass,
                TILE_SIZE,
                CHUNK_ALLOC_PER_FRAME,
                CHUNK_REBUILD_PER_FRAME,
            );
            player.set_position(scene::expedition_spawn_point());
            player.clamp_to_map(&maps);
            camera.target = player.position();
            entity_target_cache.clear();
            damage_events.clear();
            current_scene = SceneKind::Expedition;
            run_state = Some(RunState {
                timer: 300.0,
                enemies_defeated: 0,
                start_inventory: inventory.clone(),
                completed: None,
            });
            cogs.snapshot_start();
            loading_spin += LOADING_SPIN_SPEED * get_frame_time();
            show_loading(&loading, "Loading Expedition", 1.0, loading_spin).await;
        }

        if is_key_pressed(KeyCode::F2) && current_scene != SceneKind::Farm {
            loading_spin += LOADING_SPIN_SPEED * get_frame_time();
            show_loading(&loading, "Loading Farm", 0.08, loading_spin).await;
            scene::scene_farm(
                &mut maps,
                &mut entities,
                &structures,
                grass,
                TILE_SIZE,
                CHUNK_ALLOC_PER_FRAME,
                CHUNK_REBUILD_PER_FRAME,
            );
            player.set_position(scene::farm_spawn_point(&maps));
            player.clamp_to_map(&maps);
            camera.target = player.position();
            entity_target_cache.clear();
            damage_events.clear();
            current_scene = SceneKind::Farm;
            run_state = None;
            warm_scene_chunks_loading(
                &mut maps,
                &tileset,
                &loading,
                "Loading Farm",
                &mut loading_spin,
            )
            .await;
        }

        if is_quit_requested() {
            if current_scene == SceneKind::Farm {
                let _ = scene::save_farm_scene(&maps);
            }
            break;
        }

        let run_completed = run_state.as_ref().and_then(|s| s.completed).is_some();
        if !player_dead && !run_completed {
            player.update(&maps);
        }

        let particle_budget = particle_budget_scale(
            screen_width(),
            screen_height(),
            if use_render_target { render_scale } else { 1.0 },
        );
        particles.set_budget_scale(particle_budget);

        camera.zoom = camera_zoom_for_fov(CAMERA_FOV, use_render_target);
        let follow = 1.0 - (-CAMERA_DRAG * get_frame_time()).exp();
        camera.target += (player.position() - camera.target) * follow;
        camera.render_target = if use_render_target {
            Some(scene_target.clone())
        } else {
            None
        };
        maps.begin_frame_chunk_work();
        maps.prewarm_visible_chunks(camera.target, camera.zoom);

        let view_rect = camera_view_rect_logic(camera.target, CAMERA_FOV);
        let mouse_screen = mouse_position();
        let mouse_world = camera.screen_to_world(vec2(mouse_screen.0, mouse_screen.1));
        let mouse_screen_vec = vec2(mouse_screen.0, mouse_screen.1);
        notebook.handle_input();
        let ui_captures_pointer = notebook.captures_pointer()
            || inventory.captures_pointer(mouse_screen_vec, &inventory_catalog)
            || block_state
                .open_loot_key
                .as_ref()
                .is_some_and(|_| loot_ui_bounds().contains(mouse_screen_vec))
            || run_completed;
        if !notebook.open && !run_completed {
            inventory.handle_input(&inventory_catalog);
        }
        let player_pos = player.position();
        let hovered_interactor = maps
            .structure_interactors()
            .iter()
            .find(|interactor| {
                point_in_rect(mouse_world, interactor.rect)
                    && interactor_in_range(
                        player_pos,
                        interactor.group_rect,
                        interactor.interact_range_world,
                    )
            })
            .cloned();

        if !run_completed {
            block_state.active_spawner_room_lock = run_spawner_blocks(
                &maps,
                player.position(),
                &mut entities,
                &db,
                &registry,
                &mut block_state.spawners,
                dt,
            );
            if let Some(lock_rect) = block_state.active_spawner_room_lock {
                player.clamp_to_rect(lock_rect);
            }
        } else {
            block_state.active_spawner_room_lock = None;
        }

        if current_scene == SceneKind::Expedition {
            if let Some(state) = &mut run_state {
                if state.completed.is_none() {
                    state.timer -= dt;
                    if state.timer <= 0.0 {
                        state.timer = 0.0;
                        state.completed = Some(false);
                        inventory = state.start_inventory.clone();
                        cogs.reset_to_start();
                        notebook.program_text.clear();
                        notebook.puzzle_text.clear();
                    }
                }
            }
        }

        if !run_completed {
            if let Some((amount, pos)) = cogs.update(player.position(), dt) {
                sounds.play("hurt2");
                let _ = amount;
                let _ = pos;
            }
        }

        let mut summon_preview: Option<(Rect, bool, &'static str)> = None;
        let mut build_preview: Option<(Rect, bool)> = None;
        let mut hovered_cropbot_uid: Option<u64> = None;
        for ent in &entities {
            if db.entities[ent.instance.def].id == "cropbot" {
                let hb = ent.hitbox(&db);
                if hb.contains(mouse_world) {
                    hovered_cropbot_uid = Some(ent.instance.uid);
                    break;
                }
            }
        }

        let selected_item = inventory.selected_item();
        let summon_kind: Option<&'static str> = if selected_item == chopbot_summon_item {
            Some("chopbot")
        } else if selected_item == cropbot_summon_item {
            Some("cropbot")
        } else if selected_item == virat_summon_item {
            Some("virat")
        } else if selected_item == virabird_summon_item {
            Some("virabird")
        } else {
            None
        };

        if (current_scene == SceneKind::Expedition || current_scene == SceneKind::Farm)
            && summon_kind.is_some()
            && !notebook.open
        {
            if let Some(grid) = maps.grid_index(mouse_world) {
                let in_bounds = grid.x >= 0
                    && grid.y >= 0
                    && (grid.x as usize) < maps.width()
                    && (grid.y as usize) < maps.height();
                if in_bounds {
                    let gx = grid.x as usize;
                    let gy = grid.y as usize;
                    let tile_rect = maps.tile_bounds(gx, gy);
                    let bg_tile = maps.tile_at(map::LayerKind::Background, gx, gy);
                    let floor_ok = match current_scene {
                        SceneKind::Expedition => bg_tile == scene::EXPEDITION_FLOOR_TILE,
                        SceneKind::Farm => bg_tile == grass,
                    };
                    let solid_block = maps.is_solid(gx, gy);
                    let center = vec2(
                        tile_rect.x + tile_rect.w * 0.5,
                        tile_rect.y + tile_rect.h * 0.5,
                    );
                    let occupied_by_entity = entities.iter().any(|ent| {
                        let hb = ent.hitbox(&db);
                        hb.contains(center) || hb.overlaps(&tile_rect)
                    });
                    let valid = floor_ok && !solid_block && !occupied_by_entity;
                    summon_preview = Some((tile_rect, valid, summon_kind.unwrap_or("chopbot")));
                }
            }
        }

        if is_key_pressed(KeyCode::B) && current_scene == SceneKind::Farm && !run_completed {
            building.toggle();
        }
        if building.active && !run_completed && current_scene == SceneKind::Farm {
            if is_key_pressed(KeyCode::Q) {
                building.cycle(-1);
            }
            if is_key_pressed(KeyCode::E) {
                building.cycle(1);
            }
            if is_key_pressed(KeyCode::Enter) {
                for (structure_id, origin) in building.placements.drain(..) {
                    if let Some(def) = structures.iter().find(|s| s.id == structure_id) {
                        let gx = (origin.x / TILE_SIZE).floor().max(0.0) as usize;
                        let gy = (origin.y / TILE_SIZE).floor().max(0.0) as usize;
                        maps.place_structure_def(def, gx, gy);
                    }
                }
                building.active = false;
            }
        }

        if building.active && current_scene == SceneKind::Farm {
            let preview_rect = building.preview_rect(mouse_world, TILE_SIZE);
            let valid = building
                .selected_def(&structures)
                .map(|def| {
                    let w = def.structure.width().max(1) as f32 * TILE_SIZE;
                    let h = def.structure.height().max(1) as f32 * TILE_SIZE;
                    let area = Rect::new(preview_rect.x, preview_rect.y, w, h);
                    building.can_place(&maps, area, grass)
                })
                .unwrap_or(false);
            build_preview = Some((preview_rect, valid));
        }

        if is_mouse_button_pressed(MouseButton::Left) && !ui_captures_pointer && !run_completed {
            if building.active && current_scene == SceneKind::Farm {
                if let Some((rect, valid)) = build_preview {
                    if valid {
                        if let Some(def) = building.selected_def(&structures) {
                            building.commit_placement(&def.id, vec2(rect.x, rect.y));
                        }
                    }
                }
            } else if let Some(interactor) = hovered_interactor.as_ref() {
                let mut interact_actions = Vec::new();
                let mut ctx = InteractContext {
                    structure_id: &interactor.structure_id,
                    area: interactor.group_rect,
                    player: &mut player,
                    map: &mut maps,
                    actions: &mut interact_actions,
                };
                interact_registry.execute(&interactor.on_interact, &mut ctx);
                for action in interact_actions {
                    match action {
                        InteractAction::Warp { target } => {
                            if let Some(scene_kind) = parse_warp_target(&target) {
                                if current_scene == SceneKind::Expedition && scene_kind == SceneKind::Farm {
                                    if let Some(state) = &mut run_state {
                                        state.completed = Some(true);
                                    }
                                } else {
                                    current_scene = apply_scene_warp(
                                        scene_kind,
                                        current_scene,
                                        &mut maps,
                                        &mut entities,
                                        &db,
                                        &registry,
                                        &structures,
                                        grass,
                                        &tileset,
                                        &mut player,
                                        &loading,
                                        &mut loading_spin,
                                    )
                                    .await;
                                    if current_scene == SceneKind::Farm {
                                        run_state = None;
                                    } else {
                                        run_state = Some(RunState {
                                            timer: 300.0,
                                            enemies_defeated: 0,
                                            start_inventory: inventory.clone(),
                                            completed: None,
                                        });
                                    }
                                }
                            }
                        }
                        InteractAction::OpenLoot { table_id } => {
                            let key = loot_chest_key(&table_id, &interactor.group_rect);
                            block_state.loot_chests.entry(key.clone()).or_insert_with(|| {
                                build_loot_inventory(&table_id, &loot_tables, &inventory_catalog)
                                    .map(|state| state.loot)
                                    .unwrap_or_else(StackInventory::new)
                            });
                            block_state.open_loot_key = Some(key);
                        }
InteractAction::OpenLockPuzzle { puzzle_id, area } => {
                             if let Some(puzzle) = puzzle_catalog.get(&puzzle_id) {
                                 notebook.activate_puzzle(
                                     &puzzle.prompt,
                                     &puzzle.starter_code,
                                     &puzzle.validator_contains,
                                     puzzle.viz_config.as_ref(),
                                 );
                                 block_state.active_lock_area = Some(area);
                             }
                         }
                    }
                }
            } else if let Some((rect, valid, summon_entity_id)) = summon_preview {
                if valid && inventory.consume_selected_one().is_some() {
                    let spawn_pos = vec2(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5);
                    if let Some(mut entity) = Entity::spawn(&db, summon_entity_id, spawn_pos, &registry) {
                        // If we are summoning an "enemy" type, make it friendly to the player
                        // and target actual enemies instead.
                        if summon_entity_id == "virat" || summon_entity_id == "virabird" {
                            entity.instance.flags &= !entity::DEF_FLAG_TARGET_PLAYER;
                            entity.instance.flags |= entity::DEF_FLAG_TARGET_NEAREST_ENEMY;
                        }
                        entity.clamp_to_map(&maps, &db);
                        entities.push(entity);
                    }
                }
            } else if let Some(uid) = hovered_cropbot_uid {
                selected_cropbot_uid = Some(uid);
            }
        }

        if let Some(key) = block_state.open_loot_key.clone() {
            if let Some(loot) = block_state.loot_chests.get_mut(&key) {
                handle_loot_ui_input(loot, &mut inventory, &inventory_catalog);
            }
            if is_key_pressed(KeyCode::Escape) {
                block_state.open_loot_key = None;
            }
        }
        if notebook.take_puzzle_completed() {
            if let Some(area) = block_state.active_lock_area.take() {
                unlock_lock_block(&mut maps, area);
            }
        }

        if !run_completed {
            run_virabird_ranged_attacks(
                &mut entities,
                &db,
                &registry,
                if player_dead { None } else { Some(player.world_hitbox()) },
                &mut block_state.virabird_bullet_cd,
                dt,
            );
        }

        let mut entity_targets = Vec::with_capacity(entities.len());
        for ent in &entities {
            let def = &db.entities[ent.instance.def];
            entity_targets.push(entity::EntityTarget {
                id: ent.instance.uid,
                def: ent.instance.def,
                kind: def.kind,
                pos: ent.instance.pos,
                hitbox: ent.hitbox(&db),
                alive: ent.instance.hp > 0.0,
            });
        }

        damage_events.clear();
        let mut ctx = EntityContext {
            player: if player_dead || player.hp() <= 0.0 {
                None
            } else {
                Some(PlayerTarget {
                    pos: player.position(),
                    hitbox: player.world_hitbox(),
                })
            },
            target: None,
            entities: entity_targets,
            target_cache: std::mem::take(&mut entity_target_cache),
            view_height: CAMERA_FOV,
            damage_events: Vec::new(),
            db: &db,
        };

        if !run_completed {
            let mut ent_idx = 0usize;
            while ent_idx < entities.len() {
                entities[ent_idx].update(dt, &db, &mut ctx, &maps, &registry);
                entities[ent_idx].clamp_to_map(&maps, &db);
                ent_idx += 1;
            }
            resolve_entity_overlaps(&mut entities, &db, &maps);
            for ent in entities.iter_mut() {
                ent.clamp_to_map(&maps, &db);
            }
        }
        damage_events.extend(ctx.damage_events.drain(..));
        entity_target_cache = std::mem::take(&mut ctx.target_cache);

        for ent in entities.iter_mut() {
            let def = &db.entities[ent.instance.def];
            let render_origin = ent.instance.pos + def.texture.draw.offset;
            let size = def
                .texture
                .draw
                .dest_size
                .unwrap_or_else(|| def.texture.texture.size());
            let pos = render_origin + size * 0.5;
            if ent.instance.is_dashing() {
                if ent.instance.dash_trail.is_none() {
                    ent.instance.dash_trail = particles.emitter("dash_afterimage", pos);
                }
                if let Some(emitter) = ent.instance.dash_trail.as_mut() {
                    particles.update_emitter_with_texture(
                        emitter,
                        pos,
                        dt,
                        Some(&def.texture.texture),
                        Some(size),
                    );
                }
            } else if let Some(emitter) = ent.instance.dash_trail.as_mut() {
                particles.track_emitter(emitter, pos);
            }
        }

        let mut entity_index_by_uid = HashMap::with_capacity(entities.len());
        for (idx, ent) in entities.iter().enumerate() {
            entity_index_by_uid.insert(ent.instance.uid, idx);
        }

        for event in &damage_events {
            match event.target {
                Target::Player(_) => {
                    if event.amount > 0.0 {
                        sounds.play("hurt2");
                    }
                    player.apply_damage(event.amount);
                }
                Target::Entity(target) => {
                    if let Some(&ent_idx) = entity_index_by_uid.get(&target.id) {
                        let ent = &mut entities[ent_idx];
                        if event.amount > 0.0 {
                            sounds.play("hurt");
                        }
                        let old_hp = ent.instance.hp;
                        ent.instance.apply_damage(event.amount);
                        if old_hp > 0.0 && ent.instance.hp <= 0.0 {
                            if let Some(state) = &mut run_state {
                                state.enemies_defeated += 1;
                            }
                            let def_id = &db.entities[ent.instance.def];
                            if def_id.kind == entity::EntityKind::Enemy {
                                let hb = ent.hitbox(&db);
                                let drop_pos = vec2(hb.x + hb.w * 0.5, hb.y + hb.h * 0.5);
                                cogs.spawn_pickup(drop_pos, macroquad::rand::gen_range(2, 9));
                            }
                        }
                    }
                }
                Target::Position(_) => {}
            }
        }
        for ent in entities.iter_mut() {
            if db.entities[ent.instance.def].id == "virabirdBullet" {
                if ent.instance.dealt_damage_last_tick {
                    ent.instance.hp = 0.0;
                } else {
                    // Apply TTL: bullets expire after 4 seconds
                    let ttl = block_state.virabird_bullet_ttl
                        .entry(ent.instance.uid)
                        .or_insert(4.0);
                    *ttl -= dt;
                    if *ttl <= 0.0 {
                        ent.instance.hp = 0.0;
                    }
                }
            }
        }
        block_state.virabird_bullet_ttl
            .retain(|uid, _| entities.iter().any(|e| e.instance.uid == *uid));
        entities.retain(|ent| ent.instance.hp > 0.0);
        if let Some(uid) = selected_cropbot_uid {
            if !entities.iter().any(|ent| ent.instance.uid == uid) {
                selected_cropbot_uid = None;
            }
        }
        if !player_dead && player.hp() <= 0.0 {
            player_dead = true;
        }

        let dashing = !player_dead && player.is_dashing();
        let moving = !player_dead && player.is_moving(MOVE_DEADZONE) && !dashing;
        if let Some(emitter) = walk_trail.as_mut() {
            if moving {
                particles.update_emitter(emitter, player.position(), dt);
            } else {
                particles.track_emitter(emitter, player.position());
            }
        }

        if let Some(emitter) = dash_trail.as_mut() {
            if dashing {
                particles.update_emitter_with_texture(
                    emitter,
                    player.position() - Vec2::new(0.0, player.texture.size().y / 8.0),
                    dt,
                    Some(&player.texture),
                    Some(player.texture.size() * 0.25),
                );
            } else {
                particles.track_emitter(
                    emitter,
                    player.position() - Vec2::new(0.0, player.texture.size().y / 8.0),
                );
            }
        }

        particles.update(dt);

        if moving {
            footstep_timer -= dt;
            if footstep_timer <= 0.0 {
                sounds.play("footstep");
                footstep_timer = FOOTSTEP_INTERVAL;
            }
        } else {
            footstep_timer = 0.0;
        }

        set_camera(&camera);
        clear_background(BLACK);

        maps.draw_background(
            &tileset,
            camera.target,
            camera.zoom,
            screen_width(),
            screen_height(),
        );
        maps.draw_foreground(
            &tileset,
            camera.target,
            camera.zoom,
            screen_width(),
            screen_height(),
        );

        let cull_rect = expand_rect(view_rect, ENTITY_CULL_FADE_PAD);

        particles.draw_in_rect(cull_rect);

        if !player_dead {
            player.draw();
        }
        if !entities.is_empty() {
            draw_order.clear();
            for (idx, ent) in entities.iter().enumerate() {
                let hb = ent.hitbox(&db);
                if offscreen_fade_alpha(hb, view_rect, ENTITY_CULL_FADE_PAD) > 0.0 {
                    draw_order.push(idx);
                }
            }
            if draw_order.len() > 1 {
                draw_order.sort_unstable_by_key(|&idx| entities[idx].instance.def);
            }
            for &idx in &draw_order {
                let alpha = offscreen_fade_alpha(
                    entities[idx].hitbox(&db),
                    view_rect,
                    ENTITY_CULL_FADE_PAD,
                );
                entities[idx].draw_with_alpha(&db, alpha);
            }
        }

        if let Some(uid) = hovered_cropbot_uid {
            if let Some(ent) = entities.iter().find(|e| e.instance.uid == uid) {
                let hb = ent.hitbox(&db);
                draw_rectangle_lines(hb.x, hb.y, hb.w, hb.h, 2.0, YELLOW);
            }
        }

        maps.draw_overlay(
            &tileset,
            camera.target,
            camera.zoom,
            screen_width(),
            screen_height(),
        );

        if let Some(interactor) = hovered_interactor.as_ref() {
            draw_rectangle(
                interactor.group_rect.x,
                interactor.group_rect.y,
                interactor.group_rect.w,
                interactor.group_rect.h,
                Color::new(1.0, 0.95, 0.2, 0.2),
            );
            draw_rectangle_lines(
                interactor.group_rect.x,
                interactor.group_rect.y,
                interactor.group_rect.w,
                interactor.group_rect.h,
                1.0,
                Color::new(1.0, 0.95, 0.2, 0.95),
            );
        }

        if let Some((rect, valid)) = build_preview {
            building.draw_preview(rect, valid);
        }

        if let Some((rect, valid, _)) = summon_preview {
            let color = if valid {
                Color::new(1.0, 0.95, 0.2, 0.30)
            } else {
                Color::new(1.0, 0.2, 0.2, 0.30)
            };
            let line = if valid {
                Color::new(1.0, 0.95, 0.2, 0.95)
            } else {
                Color::new(1.0, 0.3, 0.3, 0.95)
            };
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, color);
            draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.5, line);
        }

        set_default_camera();
        if use_render_target {
            draw_texture_ex(
                &scene_target.texture,
                0.0,
                0.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(screen_width(), screen_height())),
                    flip_y: true,
                    ..Default::default()
                },
            );
        }

        draw_player_health(
            player.hp(),
            player.max_hp(),
            CAMERA_FOV,
            &heart_full,
            &heart_empty,
        );
        inventory.draw(&inventory_catalog, &tileset, &hotbar_slot);
        if let Some(key) = &block_state.open_loot_key {
            if let Some(loot) = block_state.loot_chests.get(key) {
                draw_loot_ui(loot, &inventory, &inventory_catalog, &tileset, &hotbar_slot);
            }
        }
        if let Some(uid) = selected_cropbot_uid {
            if let Some(ent) = entities.iter().find(|e| e.instance.uid == uid) {
                draw_cropbot_panel(ent, &db, &inventory_catalog, &tileset);
            }
        }
        notebook.draw();
        egui_macroquad::draw();

        if let Some(state) = &run_state {
            if state.completed.is_none() {
                let mins = (state.timer / 60.0) as u32;
                let secs = (state.timer % 60.0) as u32;
                let time_str = format!("{:02}:{:02}", mins, secs);
                let color = if state.timer <= 60.0 { RED } else { WHITE };
                let ui_scale = resolution_ui_scale();
                let font_size = (34.0 * ui_scale).max(22.0);
                let size = measure_text(&time_str, None, font_size as u16, 1.0);
                let y = screen_height() - font_size - 18.0 * ui_scale;
                draw_text(
                    &time_str,
                    (screen_width() - size.width) * 0.5,
                    y,
                    font_size,
                    color,
                );

                if state.timer <= 60.0 {
                    let alpha = ((60.0 - state.timer) / 60.0 * 0.5).clamp(0.0, 0.5);
                    draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(1.0, 0.0, 0.0, alpha));
                    draw_rectangle_lines(0.0, 0.0, screen_width(), screen_height(), 40.0, Color::new(1.0, 0.0, 0.0, alpha * 1.5));
                }
            } else if let Some(victory) = state.completed {
                let title = if victory { "Victory!" } else { "Corrupted" };
                let color = if victory { Color::from_hex(0xE6C781) } else { RED };
                let bounds = loot_ui_bounds();
                draw_rectangle(bounds.x, bounds.y, bounds.w, bounds.h, Color::new(0.03, 0.03, 0.05, 0.95));
                draw_rectangle_lines(bounds.x, bounds.y, bounds.w, bounds.h, 3.0, color);
                
                let title_size = measure_text(title, None, 60, 1.0);
                draw_text(title, bounds.x + (bounds.w - title_size.width) * 0.5, bounds.y + 80.0, 60.0, color);

                let time_taken = 300.0 - state.timer;
                let mins = (time_taken / 60.0) as u32;
                let secs = (time_taken % 60.0) as u32;
                let time_str = format!("Time Taken: {:02}:{:02}", mins, secs);
                draw_text(&time_str, bounds.x + 40.0, bounds.y + 160.0, 30.0, WHITE);

                let enemies_str = format!("Enemies Defeated: {}", state.enemies_defeated);
                draw_text(&enemies_str, bounds.x + 40.0, bounds.y + 200.0, 30.0, WHITE);

                draw_text("Loot Obtained:", bounds.x + 40.0, bounds.y + 240.0, 30.0, WHITE);
                let mut y = bounds.y + 280.0;
                let mut got_any = false;
                for (item, amount) in inventory.items() {
                    let start_amount = state.start_inventory.amount(item);
                    if amount > start_amount {
                        let diff = amount - start_amount;
                        let def = inventory_catalog.get(item);
                        let text = format!("{} x{}", def.name, diff);
                        draw_text(&text, bounds.x + 60.0, y, 24.0, WHITE);
                        y += 30.0;
                        got_any = true;
                    }
                }
                if !got_any {
                    draw_text("None", bounds.x + 60.0, y, 24.0, Color::new(0.6, 0.6, 0.6, 1.0));
                }

                let btn_text = "Return to Farm";
                let btn_size = measure_text(btn_text, None, 30, 1.0);
                let btn_rect = Rect::new(bounds.x + (bounds.w - btn_size.width - 40.0) * 0.5, bounds.y + bounds.h - 80.0, btn_size.width + 40.0, 50.0);
                let mouse = vec2(mouse_position().0, mouse_position().1);
                let hover = btn_rect.contains(mouse);
                draw_rectangle(btn_rect.x, btn_rect.y, btn_rect.w, btn_rect.h, if hover { Color::new(0.2, 0.2, 0.2, 1.0) } else { Color::new(0.1, 0.1, 0.1, 1.0) });
                draw_rectangle_lines(btn_rect.x, btn_rect.y, btn_rect.w, btn_rect.h, 2.0, WHITE);
                draw_text(btn_text, btn_rect.x + 20.0, btn_rect.y + 35.0, 30.0, WHITE);
            }
        }

        cogs.draw_world();
        cogs.draw_hud();
        building.draw_hud();

        next_frame().await;
    }
}

fn loot_chest_key(table_id: &str, area: &Rect) -> String {
    format!("loot:{table_id}:{:.0}:{:.0}", area.x, area.y)
}

fn loot_ui_bounds() -> Rect {
    let w = (screen_width() * 0.84).clamp(680.0, 1100.0);
    let h = (screen_height() * 0.74).clamp(420.0, 760.0);
    Rect::new((screen_width() - w) * 0.5, (screen_height() - h) * 0.5, w, h)
}

fn draw_loot_ui(
    loot: &StackInventory,
    player_inv: &StackInventory,
    catalog: &InventoryCatalog,
    tileset: &TileSet,
    hotbar_slot: &Texture2D,
) {
    const OUTER_RADIUS: f32 = 132.0;
    let bounds = loot_ui_bounds();
    draw_rectangle(bounds.x, bounds.y, bounds.w, bounds.h, Color::new(0.03, 0.03, 0.05, 0.92));
    draw_rectangle_lines(bounds.x, bounds.y, bounds.w, bounds.h, 3.0, Color::from_hex(0xE6C781));
    let left_center = vec2(bounds.x + bounds.w * 0.29, bounds.y + bounds.h * 0.56);
    let right_center = vec2(bounds.x + bounds.w * 0.71, bounds.y + bounds.h * 0.56);
    draw_loot_wheel(player_inv, catalog, tileset, left_center, true, hotbar_slot);
    draw_loot_wheel(loot, catalog, tileset, right_center, false, hotbar_slot);
    // Labels below each wheel with a small background
    let label_y = left_center.y + OUTER_RADIUS + 18.0;
    for (center, label) in [(left_center, "Player"), (right_center, "Loot")] {
        let metrics = measure_text(label, None, 22, 1.0);
        let pad_x = 14.0;
        let pad_y = 6.0;
        let bg_w = metrics.width + pad_x * 2.0;
        let bg_h = metrics.height + pad_y * 2.0;
        draw_rectangle(
            center.x - bg_w * 0.5,
            label_y - metrics.height - pad_y,
            bg_w,
            bg_h,
            Color::new(0.05, 0.04, 0.08, 0.88),
        );
        draw_rectangle_lines(
            center.x - bg_w * 0.5,
            label_y - metrics.height - pad_y,
            bg_w,
            bg_h,
            1.5,
            Color::from_hex(0xE6C781),
        );
        draw_text_ex(
            label,
            center.x - metrics.width * 0.5,
            label_y,
            TextParams {
                font_size: 22,
                color: Color::from_hex(0xF7E4B2),
                ..Default::default()
            },
        );
    }
}

fn draw_loot_wheel(
    inv: &StackInventory,
    catalog: &InventoryCatalog,
    tileset: &TileSet,
    center: Vec2,
    show_center: bool,
    slot_tex: &Texture2D,
) {
    use inventory::HOTBAR_SIZE;

    const OUTER_RADIUS: f32 = 132.0;
    const RING_THICKNESS: f32 = 72.0;
    const SLOT_SIZE: f32 = 48.0;
    const SPIN_SPEED: f32 = 0.28;

    let spin = get_time() as f32 * SPIN_SPEED;
    let slot_radius = OUTER_RADIUS - RING_THICKNESS * 0.5;
    let items: Vec<(ItemId, u32)> = inv.items();
    let count = HOTBAR_SIZE.max(1) as f32;

    draw_circle_lines(center.x, center.y, OUTER_RADIUS, 4.0, BLACK);
    draw_circle_lines(center.x, center.y, OUTER_RADIUS - RING_THICKNESS, 4.0, BLACK);

    for idx in 0..HOTBAR_SIZE {
        let angle =
            spin + idx as f32 / count * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let p = center + vec2(angle.cos(), angle.sin()) * slot_radius;
        let rect = Rect::new(p.x - SLOT_SIZE * 0.5, p.y - SLOT_SIZE * 0.5, SLOT_SIZE, SLOT_SIZE);
        draw_texture_ex(
            slot_tex,
            rect.x,
            rect.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(rect.w, rect.h)),
                ..Default::default()
            },
        );
        if let Some((item, amount)) = items.get(idx) {
            let def = catalog.get(*item);
            draw_item_icon(
                def,
                tileset,
                Rect::new(rect.x + 4.0, rect.y + 4.0, rect.w - 8.0, rect.h - 8.0),
            );
            let amount_str = format!("{}", amount);
            let metrics = measure_text(&amount_str, None, 12, 1.0);
            draw_rectangle(
                rect.x + rect.w - metrics.width - 8.0,
                rect.y + rect.h - 14.0,
                metrics.width + 5.0,
                12.0,
                Color::new(0.04, 0.025, 0.01, 0.80),
            );
            draw_text_ex(
                &amount_str,
                rect.x + rect.w - metrics.width - 5.5,
                rect.y + rect.h - 4.0,
                TextParams {
                    font_size: 12,
                    color: Color::from_hex(0xFFF0C7),
                    ..Default::default()
                },
            );
        }
    }

    if show_center {
        draw_circle(center.x, center.y, 36.0, Color::new(0.0, 0.0, 0.0, 0.46));
        draw_circle_lines(center.x, center.y, 36.0, 2.0, Color::new(0.35, 0.30, 0.18, 0.70));
        if let Some(item) = inv.selected_item() {
            let def = catalog.get(item);
            draw_item_icon(
                def,
                tileset,
                Rect::new(center.x - 20.0, center.y - 20.0, 40.0, 40.0),
            );
        }
    }
}

fn run_virabird_ranged_attacks(
    entities: &mut Vec<Entity>,
    db: &EntityDatabase,
    registry: &MovementRegistry,
    player_hitbox: Option<Rect>,
    cooldowns: &mut HashMap<u64, f32>,
    dt: f32,
) {
    let mut to_spawn: Vec<(Vec2, Target)> = Vec::new();
    for ent in entities.iter() {
        if db.entities[ent.instance.def].id != "virabird" {
            continue;
        }
        let cd = cooldowns
            .entry(ent.instance.uid)
            .or_insert_with(|| macroquad::rand::gen_range(0.0f32, 2.0f32));
        *cd -= dt;
        if *cd > 0.0 {
            continue;
        }
        *cd = 2.0;
        // Prefer nearest in-range friend, else player
        let mut best_friend_uid: Option<(f32, u64)> = None;
        for candidate in entities.iter() {
            if candidate.instance.uid == ent.instance.uid {
                continue;
            }
            if db.entities[candidate.instance.def].kind != entity::EntityKind::Friend {
                continue;
            }
            let d2 = ent.instance.pos.distance_squared(candidate.instance.pos);
            if d2 > 260.0 * 260.0 {
                continue;
            }
            match best_friend_uid {
                Some((bd2, _)) if d2 >= bd2 => {}
                _ => best_friend_uid = Some((d2, candidate.instance.uid)),
            }
        }
        let bullet_entry = if let Some((_, uid)) = best_friend_uid {
            entities.iter().find(|c| c.instance.uid == uid).map(|c| {
                let cdef = &db.entities[c.instance.def];
                let dir = (c.instance.pos - ent.instance.pos).normalize_or_zero();
                let spawn = ent.instance.pos + dir * 10.0;
                let t = Target::Entity(entity::EntityTarget {
                    id: c.instance.uid,
                    def: c.instance.def,
                    kind: cdef.kind,
                    pos: c.instance.pos,
                    hitbox: cdef.world_hitbox(c.instance.pos),
                    alive: true,
                });
                (spawn, t)
            })
        } else if let Some(hb) = player_hitbox {
            let p = vec2(hb.x + hb.w * 0.5, hb.y + hb.h * 0.5);
            if ent.instance.pos.distance_squared(p) <= 260.0 * 260.0 {
                let dir = (p - ent.instance.pos).normalize_or_zero();
                let spawn = ent.instance.pos + dir * 10.0;
                Some((spawn, Target::Player(PlayerTarget { pos: p, hitbox: hb })))
            } else {
                None
            }
        } else {
            None
        };
        if let Some(pair) = bullet_entry {
            to_spawn.push(pair);
        }
    }

    for (spawn_pos, bullet_target) in to_spawn {
        if let Some(mut bullet) = Entity::spawn(db, "virabirdBullet", spawn_pos, registry) {
            bullet.instance.current_target = Some(bullet_target);
            entities.push(bullet);
        }
    }
    cooldowns.retain(|uid, _| entities.iter().any(|e| e.instance.uid == *uid));
}

fn handle_loot_ui_input(
    loot: &mut StackInventory,
    player_inv: &mut StackInventory,
    _catalog: &InventoryCatalog,
) {
    if !is_mouse_button_pressed(MouseButton::Left) {
        return;
    }
    let mouse = vec2(mouse_position().0, mouse_position().1);
    let bounds = loot_ui_bounds();
    let left_center = vec2(bounds.x + bounds.w * 0.29, bounds.y + bounds.h * 0.56);
    let right_center = vec2(bounds.x + bounds.w * 0.71, bounds.y + bounds.h * 0.56);
    if let Some(item) = wheel_item_at(player_inv, left_center, mouse) {
        let _ = player_inv.transfer_one_to(loot, item);
    } else if let Some(item) = wheel_item_at(loot, right_center, mouse) {
        let _ = loot.transfer_one_to(player_inv, item);
    }
}

fn wheel_item_at(inv: &StackInventory, center: Vec2, mouse: Vec2) -> Option<ItemId> {
    use inventory::HOTBAR_SIZE;

    const OUTER_RADIUS: f32 = 132.0;
    const RING_THICKNESS: f32 = 72.0;
    const SLOT_SIZE: f32 = 48.0;
    const SPIN_SPEED: f32 = 0.28;

    let spin = get_time() as f32 * SPIN_SPEED;
    let d = mouse.distance(center);
    if d < OUTER_RADIUS - RING_THICKNESS - SLOT_SIZE * 0.3 || d > OUTER_RADIUS + SLOT_SIZE * 0.7 {
        return None;
    }
    let items = inv.items();
    let count = HOTBAR_SIZE.max(1) as f32;
    for idx in 0..HOTBAR_SIZE {
        let angle = spin + idx as f32 / count * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let p = center + vec2(angle.cos(), angle.sin()) * (OUTER_RADIUS - RING_THICKNESS * 0.5);
        let rect = Rect::new(p.x - SLOT_SIZE * 0.5, p.y - SLOT_SIZE * 0.5, SLOT_SIZE, SLOT_SIZE);
        if rect.contains(mouse) {
            return items.get(idx).map(|(item, _)| *item);
        }
    }
    None
}

fn unlock_lock_block(map: &mut TileMap, area: Rect) {
    let tile = map.tile_size().max(1.0);
    let start_gx = (area.x / tile).floor().max(0.0) as usize;
    let start_gy = (area.y / tile).floor().max(0.0) as usize;
    let end_gx = ((area.x + area.w) / tile).ceil().max(0.0) as usize;
    let end_gy = ((area.y + area.h) / tile).ceil().max(0.0) as usize;

    for gy in start_gy..end_gy {
        for gx in start_gx..end_gx {
            if gx < map.width() && gy < map.height() {
                map.set_collision(gx, gy, false);
                map.set_tile(map::LayerKind::Overlay, gx, gy, 0);
            }
        }
    }
}

fn camera_zoom_for_fov(view_height: f32, render_target: bool) -> Vec2 {
    let view_h = view_height.max(1.0);
    let aspect = screen_width().max(1.0) / screen_height().max(1.0);
    let view_w = view_h * aspect;
    let y_sign = if render_target { -1.0 } else { 1.0 };
    vec2(2.0 / view_w, y_sign * 2.0 / view_h)
}

fn camera_view_rect_logic(target: Vec2, view_height: f32) -> Rect {
    let view_h = view_height.max(1.0);
    Rect::new(
        target.x - view_h * 0.5,
        target.y - view_h * 0.5,
        view_h,
        view_h,
    )
}

fn expand_rect(rect: Rect, pad: f32) -> Rect {
    Rect::new(
        rect.x - pad,
        rect.y - pad,
        rect.w + pad * 2.0,
        rect.h + pad * 2.0,
    )
}

fn scale_rect(rect: Rect, factor: f32) -> Rect {
    let f = factor.max(0.0);
    let cx = rect.x + rect.w * 0.5;
    let cy = rect.y + rect.h * 0.5;
    let w = rect.w * f;
    let h = rect.h * f;
    Rect::new(cx - w * 0.5, cy - h * 0.5, w, h)
}

fn create_scene_target(scale: f32, screen_w: f32, screen_h: f32) -> RenderTarget {
    let target_w = (screen_w * scale).round().max(1.0) as u32;
    let target_h = (screen_h * scale).round().max(1.0) as u32;
    let target = render_target(target_w, target_h);
    target.texture.set_filter(FilterMode::Nearest);
    target
}

fn particle_budget_scale(screen_w: f32, screen_h: f32, render_scale: f32) -> f32 {
    let base_area = 500.0 * 500.0;
    let area = (screen_w * screen_h * render_scale * render_scale).max(1.0);
    (base_area / area).clamp(0.35, 1.0)
}

fn offscreen_fade_alpha(hitbox: Rect, view_rect: Rect, fade_pad: f32) -> f32 {
    if hitbox.overlaps(&view_rect) {
        return 1.0;
    }
    let expanded = expand_rect(view_rect, fade_pad.max(1.0));
    if !hitbox.overlaps(&expanded) {
        return 0.0;
    }

    let cx = hitbox.x + hitbox.w * 0.5;
    let cy = hitbox.y + hitbox.h * 0.5;
    let nearest_x = cx.clamp(view_rect.x, view_rect.x + view_rect.w);
    let nearest_y = cy.clamp(view_rect.y, view_rect.y + view_rect.h);
    let distance = vec2(cx - nearest_x, cy - nearest_y).length();
    (1.0 - distance / fade_pad.max(1.0)).clamp(0.0, 1.0)
}

fn point_in_rect(point: Vec2, rect: Rect) -> bool {
    point.x >= rect.x
        && point.y >= rect.y
        && point.x <= rect.x + rect.w
        && point.y <= rect.y + rect.h
}

fn interactor_in_range(player_pos: Vec2, area: Rect, range_world: f32) -> bool {
    if range_world <= 0.0 {
        return true;
    }
    let nearest = vec2(
        player_pos.x.clamp(area.x, area.x + area.w),
        player_pos.y.clamp(area.y, area.y + area.h),
    );
    player_pos.distance(nearest) <= range_world
}

fn parse_warp_target(target: &str) -> Option<SceneKind> {
    match target {
        "farm" => Some(SceneKind::Farm),
        "expedition" => Some(SceneKind::Expedition),
        _ => None,
    }
}

async fn apply_scene_warp(
    target: SceneKind,
    current_scene: SceneKind,
    maps: &mut TileMap,
    entities: &mut Vec<Entity>,
    db: &EntityDatabase,
    registry: &MovementRegistry,
    structures: &[map::StructureDef],
    grass: u8,
    tileset: &TileSet,
    player: &mut Player,
    loading: &Texture2D,
    loading_spin: &mut f32,
) -> SceneKind {
    if target == current_scene {
        return current_scene;
    }
    match target {
        SceneKind::Expedition => {
            scene::scene_expedition(
                maps,
                entities,
                db,
                registry,
                structures,
                grass,
                TILE_SIZE,
                CHUNK_ALLOC_PER_FRAME,
                CHUNK_REBUILD_PER_FRAME,
            );
            player.set_position(scene::expedition_spawn_point());
            player.clamp_to_map(maps);
            SceneKind::Expedition
        }
        SceneKind::Farm => {
            scene::scene_farm(
                maps,
                entities,
                structures,
                grass,
                TILE_SIZE,
                CHUNK_ALLOC_PER_FRAME,
                CHUNK_REBUILD_PER_FRAME,
            );
            player.set_position(scene::farm_spawn_point(maps));
            player.clamp_to_map(maps);
            warm_scene_chunks_loading(
                maps,
                tileset,
                loading,
                "Loading Farm",
                loading_spin,
            )
            .await;
            SceneKind::Farm
        }
    }
}

fn build_loot_inventory(
    table_id: &str,
    tables: &LootTables,
    catalog: &InventoryCatalog,
) -> Option<LootUiState> {
    let mut inv = StackInventory::new();
    let entries = tables.get(table_id)?;
    for entry in entries.iter().take(15) {
        if let Some(item_id) = catalog.id_by_key(&entry.item) {
            inv.add(item_id, entry.amount.max(1));
        }
    }
    Some(LootUiState { loot: inv })
}

fn run_spawner_blocks(
    map: &TileMap,
    player_pos: Vec2,
    entities: &mut Vec<Entity>,
    db: &EntityDatabase,
    registry: &MovementRegistry,
    states: &mut HashMap<String, SpawnerRuntime>,
    dt: f32,
) -> Option<Rect> {
    let mut active_lock = None;
    for spawner in map.placed_spawners() {
        let key = format!("spawner:{:.0}:{:.0}", spawner.rect.x, spawner.rect.y);
        let state = states.entry(key).or_insert(SpawnerRuntime {
            remaining: spawner.def.total,
            cooldown: spawner.def.interval,
        });
        if state.remaining == 0 {
            continue;
        }
        let room_rect = spawner_room_bounds(map, spawner.rect).unwrap_or(spawner.rect);
        let player_in_room = room_rect.contains(player_pos);
        if player_in_room {
            active_lock = Some(room_rect);
        } else {
            continue;
        }
        state.cooldown -= dt;
        if state.cooldown > 0.0 {
            continue;
        }
        state.cooldown = spawner.def.interval;
        let candidates = &spawner.def.entities;
        if candidates.is_empty() {
            continue;
        }
        let pick_idx = macroquad::rand::gen_range(0usize, candidates.len());
        let entity_id = &candidates[pick_idx];
        let spawn_pos = vec2(
            spawner.rect.x + spawner.rect.w * 0.5,
            spawner.rect.y + spawner.rect.h * 0.5,
        );
        if let Some(grid) = map.grid_index(spawn_pos) {
            let gx = grid.x.max(0) as usize;
            let gy = grid.y.max(0) as usize;
            if gx < map.width()
                && gy < map.height()
                && !map.is_solid(gx, gy)
                && map.tile_at(map::LayerKind::Background, gx, gy) == scene::EXPEDITION_FLOOR_TILE
            {
                if let Some(mut entity) = Entity::spawn(db, entity_id, spawn_pos, registry) {
                    entity.clamp_to_map(map, db);
                    entities.push(entity);
                    state.remaining -= 1;
                }
            }
        }
    }
    active_lock
}

fn spawner_room_bounds(map: &TileMap, area: Rect) -> Option<Rect> {
    use std::collections::VecDeque;

    let tile = map.tile_size().max(1.0);
    let center = vec2(area.x + area.w * 0.5, area.y + area.h * 0.5);
    let grid = map.grid_index(center)?;
    let start_x = grid.x.max(0) as usize;
    let start_y = grid.y.max(0) as usize;
    if start_x >= map.width() || start_y >= map.height() || map.is_solid(start_x, start_y) {
        return None;
    }

    let max_tiles = 900usize;
    let mut visited = vec![false; map.width() * map.height()];
    let mut queue = VecDeque::new();
    let start_idx = start_y * map.width() + start_x;
    visited[start_idx] = true;
    queue.push_back((start_x, start_y));
    let mut min_x = start_x;
    let mut max_x = start_x;
    let mut min_y = start_y;
    let mut max_y = start_y;
    let mut count = 0usize;

    while let Some((x, y)) = queue.pop_front() {
        count += 1;
        if count > max_tiles {
            break;
        }
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
        for (nx, ny) in [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ] {
            if nx >= map.width() || ny >= map.height() {
                continue;
            }
            if map.is_solid(nx, ny) {
                continue;
            }
            let idx = ny * map.width() + nx;
            if visited[idx] {
                continue;
            }
            visited[idx] = true;
            queue.push_back((nx, ny));
        }
    }

    Some(Rect::new(
        min_x as f32 * tile,
        min_y as f32 * tile,
        (max_x - min_x + 1) as f32 * tile,
        (max_y - min_y + 1) as f32 * tile,
    ))
}

fn resolve_entity_overlaps(entities: &mut [Entity], db: &EntityDatabase, map: &TileMap) {
    if entities.len() < 2 {
        return;
    }

    const EPSILON: f32 = 0.0005;
    const CELL_SIZE: f32 = 32.0;
    const SOLVER_ITERS: usize = 4;

    #[inline]
    fn pair_sign(i: usize, j: usize, salt: u64) -> f32 {
        let mut h = (i as u64).wrapping_mul(0x9E37_79B1_85EB_CA87);
        h ^= (j as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
        h ^= salt;
        if (h & 1) == 0 { -1.0 } else { 1.0 }
    }

    let mut overlap_marks = vec![0u32; entities.len()];
    let mut overlap_stamp = 1u32;
    let mut corrections = vec![Vec2::ZERO; entities.len()];
    let mut collide_cache: HashMap<(usize, usize), bool> = HashMap::new();

    for _ in 0..SOLVER_ITERS {
        let mut any = false;
        let mut hitboxes = Vec::with_capacity(entities.len());
        let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::with_capacity(entities.len() * 2);

        for (idx, ent) in entities.iter().enumerate() {
            let hb = db.entities[ent.instance.def].world_hitbox(ent.instance.pos);
            hitboxes.push(hb);
            let (min_cx, max_cx, min_cy, max_cy) = rect_cell_range(hb, CELL_SIZE);
            for cy in min_cy..=max_cy {
                for cx in min_cx..=max_cx {
                    grid.entry((cx, cy)).or_default().push(idx);
                }
            }
        }

        corrections.fill(Vec2::ZERO);

        for i in 0..entities.len() {
            overlap_stamp = overlap_stamp.wrapping_add(1);
            if overlap_stamp == 0 {
                overlap_marks.fill(0);
                overlap_stamp = 1;
            }

            let a_hb = hitboxes[i];
            let a_center = vec2(a_hb.x + a_hb.w * 0.5, a_hb.y + a_hb.h * 0.5);
            let (min_cx, max_cx, min_cy, max_cy) = rect_cell_range(a_hb, CELL_SIZE);
            for cy in min_cy..=max_cy {
                for cx in min_cx..=max_cx {
                    let Some(bucket) = grid.get(&(cx, cy)) else {
                        continue;
                    };
                    for &j in bucket {
                        if j <= i {
                            continue;
                        }
                        if overlap_marks[j] == overlap_stamp {
                            continue;
                        }
                        overlap_marks[j] = overlap_stamp;

                        let a_def_idx = entities[i].instance.def;
                        let b_def_idx = entities[j].instance.def;
                        let pair = if a_def_idx <= b_def_idx {
                            (a_def_idx, b_def_idx)
                        } else {
                            (b_def_idx, a_def_idx)
                        };
                        let can_collide = *collide_cache
                            .entry(pair)
                            .or_insert_with(|| entities_should_collide(db, a_def_idx, b_def_idx));
                        if !can_collide {
                            continue;
                        }

                        let b_hb = hitboxes[j];
                        let overlap_x = (a_hb.x + a_hb.w).min(b_hb.x + b_hb.w) - a_hb.x.max(b_hb.x);
                        let overlap_y = (a_hb.y + a_hb.h).min(b_hb.y + b_hb.h) - a_hb.y.max(b_hb.y);
                        if overlap_x <= 0.0 || overlap_y <= 0.0 {
                            continue;
                        }

                        any = true;
                        let b_center = vec2(b_hb.x + b_hb.w * 0.5, b_hb.y + b_hb.h * 0.5);
                        let delta = b_center - a_center;

                        let choose_x = if (overlap_x - overlap_y).abs() <= 0.0001 {
                            if delta.x.abs() > delta.y.abs() {
                                true
                            } else if delta.y.abs() > delta.x.abs() {
                                false
                            } else {
                                pair_sign(i, j, 0xA53C_7E19) > 0.0
                            }
                        } else {
                            overlap_x < overlap_y
                        };

                        let pair_extent = a_hb.w.min(a_hb.h).min(b_hb.w.min(b_hb.h)).max(1.0);
                        let max_pair_push = pair_extent * 0.35;

                        if choose_x {
                            let dir = if delta.x.abs() > 0.0001 {
                                delta.x.signum()
                            } else {
                                pair_sign(i, j, 0x5F4D_CC3B)
                            };
                            let push = ((overlap_x + EPSILON) * 0.5).min(max_pair_push);
                            corrections[i].x -= dir * push;
                            corrections[j].x += dir * push;
                        } else {
                            let dir = if delta.y.abs() > 0.0001 {
                                delta.y.signum()
                            } else {
                                pair_sign(i, j, 0x73D2_A11F)
                            };
                            let push = ((overlap_y + EPSILON) * 0.5).min(max_pair_push);
                            corrections[i].y -= dir * push;
                            corrections[j].y += dir * push;
                        }
                    }
                }
            }
        }

        if !any {
            break;
        }

        for i in 0..entities.len() {
            let mut correction = corrections[i];
            if correction.length_squared() <= 0.0 {
                continue;
            }

            let hb = hitboxes[i];
            let max_total_push = hb.w.max(hb.h).max(1.0) * 0.45;
            let len_sq = correction.length_squared();
            if len_sq > max_total_push * max_total_push {
                correction *= max_total_push / len_sq.sqrt();
            }

            entities[i].instance.pos += correction;
            entities[i].clamp_to_map(map, db);
        }
    }
}

fn draw_cropbot_panel(
    ent: &Entity,
    db: &EntityDatabase,
    catalog: &InventoryCatalog,
    tileset: &TileSet,
) {
    let panel_w = 420.0;
    let panel_h = 220.0;
    let x = (screen_width() - panel_w) * 0.5;
    let y = screen_height() - panel_h - 24.0;
    draw_rectangle(x, y, panel_w, panel_h, Color::new(0.08, 0.08, 0.1, 0.92));
    draw_rectangle_lines(x, y, panel_w, panel_h, 2.0, Color::new(0.9, 0.9, 0.9, 0.9));
    draw_text("Cropbot", x + 14.0, y + 24.0, 24.0, YELLOW);

    let crop_def = &db.entities[ent.instance.def];
    let icon_rect = Rect::new(x + 24.0, y + 38.0, 56.0, 56.0);
    draw_texture_ex(
        &crop_def.texture.texture,
        icon_rect.x,
        icon_rect.y,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(icon_rect.w, icon_rect.h)),
            ..Default::default()
        },
    );

    let slot_y = y + panel_h - 74.0;
    for i in 0..3 {
        let sx = x + 18.0 + i as f32 * 64.0;
        draw_rectangle(sx, slot_y, 52.0, 52.0, Color::new(0.2, 0.2, 0.24, 1.0));
        draw_rectangle_lines(
            sx,
            slot_y,
            52.0,
            52.0,
            2.0,
            Color::new(0.95, 0.95, 0.8, 0.9),
        );
        let slot_item = match i {
            0 => ent.instance.cropbot_slots.seed,
            1 => ent.instance.cropbot_slots.bonemeal,
            _ => ent.instance.cropbot_slots.tool,
        };
        if let Some(raw) = slot_item {
            let idx = ItemId(raw as usize % catalog.iter_ids().count().max(1));
            let def = catalog.get(idx);
            draw_item_icon(def, tileset, Rect::new(sx + 6.0, slot_y + 6.0, 40.0, 40.0));
        }
    }

    let right_x = x + panel_w * 0.52;
    draw_text(
        "Player Inventory",
        right_x,
        y + 24.0,
        22.0,
        Color::from_hex(0xCDE9FF),
    );
    draw_text(
        "Use wheel / hotbar on right side of HUD",
        right_x,
        y + 52.0,
        18.0,
        Color::from_hex(0xAFC6D9),
    );
}

fn rect_cell_range(rect: Rect, cell_size: f32) -> (i32, i32, i32, i32) {
    let cell = cell_size.max(1.0);
    let min_cx = (rect.x / cell).floor() as i32;
    let max_cx = ((rect.x + rect.w) / cell).floor() as i32;
    let min_cy = (rect.y / cell).floor() as i32;
    let max_cy = ((rect.y + rect.h) / cell).floor() as i32;
    (min_cx, max_cx, min_cy, max_cy)
}

fn entities_should_collide(db: &EntityDatabase, a_def_idx: usize, b_def_idx: usize) -> bool {
    let a_flags = db.entities[a_def_idx].flags;
    let b_flags = db.entities[b_def_idx].flags;
    if (a_flags & entity::DEF_FLAG_NO_ENTITY_COLLISION) != 0
        || (b_flags & entity::DEF_FLAG_NO_ENTITY_COLLISION) != 0
    {
        return false;
    }

    let a_kind = db.entities[a_def_idx].kind;
    let b_kind = db.entities[b_def_idx].kind;
    !blocks_kind(db, a_def_idx, b_kind) && !blocks_kind(db, b_def_idx, a_kind)
}

fn blocks_kind(db: &EntityDatabase, def_idx: usize, kind: entity::EntityKind) -> bool {
    let flags = db.entities[def_idx].flags;
    match kind {
        entity::EntityKind::Enemy => (flags & entity::DEF_FLAG_NO_ENEMY_COLLISION) != 0,
        entity::EntityKind::Friend => (flags & entity::DEF_FLAG_NO_FRIEND_COLLISION) != 0,
        entity::EntityKind::Misc => (flags & entity::DEF_FLAG_NO_MISC_COLLISION) != 0,
    }
}

fn draw_player_health(
    hp: f32,
    max_hp: f32,
    view_height: f32,
    heart_full: &Texture2D,
    heart_empty: &Texture2D,
) {
    if max_hp <= 0.0 {
        return;
    }
    let hp_per_heart = 1.0;
    let ui_scale = resolution_ui_scale();
    let padding = 8.0 * ui_scale;
    let base_fov = 300.0;
    let fov_scale = (base_fov / view_height.max(1.0)).clamp(0.7, 1.35);
    let scale = fov_scale * 0.75 * ui_scale;

    let heart_w = heart_full.width() * scale;
    let heart_h = heart_full.height() * scale;
    if heart_w <= 0.0 || heart_h <= 0.0 {
        return;
    }
    // Terraria-style overlap: sprite has padding, so compress spacing hard.
    let step_x = (heart_w * 0.4).max(1.0);
    let step_y = (heart_h * 0.4).max(1.0);

    let total_hearts = (max_hp / hp_per_heart).ceil().max(1.0) as i32;
    let full_hearts = (hp / hp_per_heart).floor().max(0.0) as i32;
    let hearts_per_row = 10;
    let rows = ((total_hearts + hearts_per_row - 1) / hearts_per_row) as i32;

    for row in 0..rows {
        let row_start = row * hearts_per_row;
        let row_count = (total_hearts - row_start).min(hearts_per_row);
        let row_width = heart_w + (row_count as f32 - 1.0) * step_x;
        let start_x = screen_width() - padding - row_width;
        let y = padding + row as f32 * step_y;

        for i in 0..row_count {
            let idx = row_start + i;
            let tex = if idx < full_hearts {
                heart_full
            } else {
                heart_empty
            };
            let x = start_x + i as f32 * step_x;
            draw_texture_ex(
                tex,
                x,
                y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(heart_w, heart_h)),
                    ..Default::default()
                },
            );
        }
    }
}
