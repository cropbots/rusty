use macroquad::prelude::*;
use miniquad::conf::{Icon, Platform};
use image::imageops::FilterType;

mod map;
mod player;
mod helpers;
mod entity;
mod r#trait;
mod particle;
mod tilemap;
mod sound;

use map::{TileMap, TileSet, load_structures_from_dir};
use player::Player;
use entity::{Entity, EntityContext, EntityDatabase, MovementRegistry};

use sound::SoundSystem;
use particle::ParticleSystem;

const CAMERA_DRAG: f32 = 5.0;
const TILE_SIZE: f32 = 16.0;
const MOVE_DEADZONE: f32 = 16.0;
const FOOTSTEP_INTERVAL: f32 = 0.2;
const CAMERA_FOV: f32 = 200.0;

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

#[macroquad::main(window_conf)]
async fn main() {
    // Load the tileset atlas (tileset.json + tileset.png)
    let tileset = TileSet::load("src/assets/tileset.json", "src/assets/tileset.png")
        .await
        .unwrap_or_else(|err| {
            eprintln!("tileset load failed: {err}");
            eprintln!("Please ensure src/assets/tileset.json and src/assets/tileset.png exist");
            panic!("Tileset loading failed");
        });
    let mut maps = TileMap::demo(512, 512, TILE_SIZE, tileset.count(), 0.0);

    // Load structures from JSON and apply them with a fixed seed.
    let structures = load_structures_from_dir("src/structure").await.unwrap_or_else(|err| {
        eprintln!("structure load failed: {err}");
        Vec::new()
    });
    if !structures.is_empty() {
        maps.apply_structures(&structures, 1337);
    }

    // Player
    let player_texture = helpers::load_single_texture("src/assets/objects", "player08")
        .await
        .unwrap_or_else(Texture2D::empty);
    let mut player = Player::new(
        vec2(200.0, 300.0 + 16.0 / 2.0),
        player_texture,
        Rect::new(-6.5 / 2.0, -8.0, 6.5, 8.0),
    );

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
    let db = EntityDatabase::load_from("src/entity")
        .await
        .unwrap_or_else(|err| {
            eprintln!("entity load failed: {err}");
            EntityDatabase::empty()
        });

    let mut entities = Vec::<Entity>::new();
    if let Some(virat) = Entity::spawn(&db, "virat", vec2(100.0, 100.0), &registry) {
        entities.push(virat);
    }
    let mut draw_order: Vec<usize> = Vec::new();

    // Particle system
    let mut particles = ParticleSystem::load_from("src/particle")
        .await
        .unwrap_or_else(|err| {
            eprintln!("particle load failed: {err}");
            ParticleSystem::empty()
        });
    let mut walk_trail = particles.emitter("dust_trail", player.position());
    let mut dash_trail = particles.emitter("dash_afterimage", player.position());

    // Load sounds
    let sounds = SoundSystem::load_from("src/sound")
        .await
        .unwrap_or_else(|err| {
            eprintln!("sound load failed: {err}");
            SoundSystem::empty()
        });

    let mut footstep_timer = 0.0f32;
    
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
        
        player.update(&maps);
        
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

        let view_rect = camera_view_rect(camera.target, CAMERA_FOV);
        let sim_rect = scale_rect(view_rect, 2.0);
        let delete_rect = scale_rect(view_rect, 4.0);

        let mut ent_idx = 0usize;
        while ent_idx < entities.len() {
            let hb = entities[ent_idx].hitbox(&db);
            if !hb.overlaps(&delete_rect) {
                entities.swap_remove(ent_idx);
                continue;
            }

            if hb.overlaps(&sim_rect) {
                entities[ent_idx].update(dt, &db, &EntityContext { target: Some(player.position()) }, &maps);
                entities[ent_idx].clamp_to_map(&maps, &db);
            }
            ent_idx += 1;
        }

        let dashing = player.is_dashing();
        let moving = player.is_moving(MOVE_DEADZONE) && !dashing;
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

        let cull_rect = expand_rect(view_rect, 64.0);

        particles.draw_in_rect(cull_rect);

        player.draw();
        if !entities.is_empty() {
            draw_order.clear();
            draw_order.extend(0..entities.len());
            draw_order.sort_unstable_by_key(|&idx| entities[idx].instance.def);
            for &idx in &draw_order {
                let ent = &entities[idx];
                let hb = ent.hitbox(&db);
                if hb.overlaps(&cull_rect) {
                    ent.draw(&db);
                }
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
    let view_w = view_h * screen_width().max(1.0) / screen_height().max(1.0);
    let y_sign = if render_target { -1.0 } else { 1.0 };
    vec2(2.0 / view_w, y_sign * 2.0 / view_h)
}

fn camera_view_rect(target: Vec2, view_height: f32) -> Rect {
    let view_h = view_height.max(1.0);
    let view_w = view_h * screen_width().max(1.0) / screen_height().max(1.0);
    Rect::new(
        target.x - view_w * 0.5,
        target.y - view_h * 0.5,
        view_w,
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
    let base_area = 1920.0 * 1080.0;
    let area = (screen_w * screen_h * render_scale * render_scale).max(1.0);
    (base_area / area).clamp(0.35, 1.0)
}
