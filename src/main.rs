use macroquad::prelude::*;
use miniquad::conf::{Icon, Platform};
use image::imageops::FilterType;
use std::collections::HashMap;
use std::future::poll_fn;
use std::task::Poll;

mod map;
mod player;
mod helpers;
mod entity;
mod r#trait;
mod particle;
mod tilemap;
mod sound;

use map::{LayerKind, TileMap, TileSet, load_structures_from_dir};
use player::Player;
use entity::{DamageEvent, Entity, EntityContext, EntityDatabase, MovementRegistry, PlayerTarget, Target};

use sound::SoundSystem;
use particle::ParticleSystem;

const CAMERA_DRAG: f32 = 5.0;
const TILE_SIZE: f32 = 16.0;
const MOVE_DEADZONE: f32 = 16.0;
const FOOTSTEP_INTERVAL: f32 = 0.2;
const CAMERA_FOV: f32 = 300.0;
const ENTITY_CULL_FADE_PAD: f32 = 96.0;
const LOADING_SPIN_SPEED: f32 = 3.0;
const STRUCTURE_APPLY_TIME_BUDGET_S: f32 = 0.005;
const CHUNK_ALLOC_PER_FRAME: usize = 6;
const CHUNK_REBUILD_PER_FRAME: usize = 8;

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
    draw_text(
        &format!("{label} {pct:.0}%"),
        20.0,
        40.0,
        30.0,
        WHITE,
    );
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
    loading_spin += LOADING_SPIN_SPEED * get_frame_time();
    show_loading(&loading, "Loading", 0.22, loading_spin).await;
    let mut maps = TileMap::new_deferred(512, 512, TILE_SIZE, Vec2::new(TILE_SIZE, TILE_SIZE), 0.0);
    maps.set_chunk_work_budget(CHUNK_ALLOC_PER_FRAME, CHUNK_REBUILD_PER_FRAME);
    let grass: u8 = if tileset.count() > 24 { 24 } else { 0 };
    maps.fill_layer(LayerKind::Background, grass);
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
    if !structures.is_empty() {
        maps.start_structure_apply(structures, 1337);
        while !maps.apply_structures_step(STRUCTURE_APPLY_TIME_BUDGET_S) {
            loading_spin += LOADING_SPIN_SPEED * get_frame_time();
            show_loading(&loading, "Placing structures", maps.structure_apply_progress() * 0.15 + 0.45, loading_spin).await;
        }
    }
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
    heart_full.set_filter(FilterMode::Nearest);
    heart_empty.set_filter(FilterMode::Nearest);

    // Camera
    let mut camera = Camera2D {
        target: player.position(),
        zoom: vec2(1.0, 1.0),
        ..Default::default()
    };

    let mut i: f32 = 0.0;
    let mut fps: i32 = 0;

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

    let mut entities = Vec::<Entity>::new();
    for _ in 0..2 {
        let pos = vec2(
            helpers::random_range(0.0, 100.0),
            helpers::random_range(0.0, 100.0),
        );
        if let Some(virat) = Entity::spawn(&db, "virabird", pos, &registry) {
            entities.push(virat);
        }
    }
    for _ in 0..3 {
        let pos = vec2(
            helpers::random_range(0.0, 100.0),
            helpers::random_range(0.0, 100.0),
        );
        if let Some(virat) = Entity::spawn(&db, "virat", pos, &registry) {
            entities.push(virat);
        }
    }
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
    let mut player_dead = false;
    
    loop {
        let dt = get_frame_time();
        
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
        
        if !player_dead {
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
        let sim_rect = scale_rect(view_rect, 2.0);

        let mut entity_targets = Vec::with_capacity(entities.len());
        for ent in &entities {
            let def = &db.entities[ent.instance.def];
            entity_targets.push(entity::EntityTarget {
                id: ent.instance.uid,
                def: ent.instance.def,
                kind: def.kind,
                pos: ent.instance.pos,
                hitbox: ent.hitbox(&db),
            });
        }

        damage_events.clear();
        let mut ctx = EntityContext {
            player: if player_dead {
                None
            } else {
                Some(PlayerTarget {
                    pos: player.position(),
                    hitbox: player.world_hitbox(),
                })
            },
            target: None,
            entities: entity_targets,
            target_cache: std::cell::RefCell::new(HashMap::new()),
            view_height: CAMERA_FOV,
            damage_events: Vec::new(),
        };

        let mut ent_idx = 0usize;
        while ent_idx < entities.len() {
            let hb = entities[ent_idx].hitbox(&db);
            if hb.overlaps(&sim_rect) {
                entities[ent_idx].update(dt, &db, &mut ctx, &maps, &registry);
                entities[ent_idx].clamp_to_map(&maps, &db);
            }
            ent_idx += 1;
        }
        damage_events.extend(ctx.damage_events.drain(..));

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
                        ent.instance.apply_damage(event.amount);
                    }
                }
                Target::Position(_) => {}
            }
        }
        entities.retain(|ent| ent.instance.hp > 0.0);
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

        maps.draw_overlay(
            &tileset,
            camera.target,
            camera.zoom,
            screen_width(),
            screen_height(),
        );

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

        i += get_frame_time();
        if i >= 1.0 {
            fps = get_fps();
            i = 0.0;
        } 
        draw_text(
            &format!("FPS: {:.0}", fps),
            20.0,
            40.0,
            30.0, // font size
            WHITE
        );

        next_frame().await;
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
    let padding = 8.0;
    let base_fov = 300.0;
    let fov_scale = (base_fov / view_height.max(1.0)).clamp(0.7, 1.35);
    let scale = fov_scale;

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
