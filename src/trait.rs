use crate::entity::{
    BehaviorRuntime,
    DEF_FLAG_ERRATIC,
    EntityContext,
    EntityKind,
    EntityInstance,
    MovementParams,
    StatBlock,
    TraitDef,
    Target,
};
use macroquad::prelude::*;

pub fn append_builtin_traits(traits: &mut Vec<TraitDef>) {
    let mut push_trait = |id: &str, flags: &[&str]| {
        if traits.iter().any(|t| t.id == id) {
            return;
        }
        traits.push(TraitDef {
            id: id.to_string(),
            stats: StatBlock::default(),
            flags: flags.iter().map(|s| s.to_string()).collect(),
            tags: Default::default(),
        });
    };

    push_trait("target_player", &["target_player"]);
    push_trait("target_nearest_entity", &["target_nearest_entity"]);
    push_trait("target_nearest_enemy", &["target_nearest_enemy"]);
    push_trait("target_nearest_friend", &["target_nearest_friend"]);
    push_trait("target_nearest_misc", &["target_nearest_misc"]);
    push_trait("dynamic_targeting", &["dynamic_targeting"]);
    push_trait("erratic", &["erratic"]);
    push_trait("no_map_collision", &["no_map_collision"]);
    push_trait("no_entity_collision", &["no_entity_collision"]);
    push_trait("no_enemy_collision", &["no_enemy_collision"]);
    push_trait("no_friend_collision", &["no_friend_collision"]);
    push_trait("no_misc_collision", &["no_misc_collision"]);
    push_trait("no_player_collision", &["no_player_collision"]);
}

fn cooldown_with_erratic(entity: &EntityInstance, base: f32) -> f32 {
    if base <= 0.0 {
        return 0.0;
    }
    if (entity.flags & DEF_FLAG_ERRATIC) == 0 {
        return base;
    }
    base * macroquad::rand::gen_range(0.5f32, 1.5f32)
}

fn rotate_towards_dir(current: Vec2, desired: Vec2, max_turn_radians: f32) -> Vec2 {
    if desired.length_squared() <= 0.0001 {
        return current.normalize_or_zero();
    }
    if current.length_squared() <= 0.0001 {
        return desired.normalize();
    }
    let current_n = current.normalize();
    let desired_n = desired.normalize();
    let current_angle = current_n.y.atan2(current_n.x);
    let desired_angle = desired_n.y.atan2(desired_n.x);
    let mut delta = desired_angle - current_angle;
    while delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    }
    while delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    let step = delta.clamp(-max_turn_radians, max_turn_radians);
    let next_angle = current_angle + step;
    vec2(next_angle.cos(), next_angle.sin())
}

fn resolve_speed(params: &MovementParams, specific_key: &str, fallback: f32) -> f32 {
    params
        .get(specific_key)
        .copied()
        .or_else(|| params.get("speed").copied())
        .unwrap_or(fallback)
}

fn nearest_entity_target(
    entity: &EntityInstance,
    ctx: &EntityContext,
    kind_filter: Option<EntityKind>,
) -> Option<Target> {
    let mut best: Option<(f32, Target)> = None;
    for candidate in &ctx.entities {
        if candidate.id == entity.uid || !candidate.alive {
            continue;
        }
        if let Some(kind) = kind_filter {
            if candidate.kind != kind {
                continue;
            }
        }
        let dist_sq = entity.pos.distance_squared(candidate.pos);
        match best {
            Some((best_dist, _)) if dist_sq >= best_dist => {}
            _ => best = Some((dist_sq, Target::Entity(*candidate))),
        }
    }
    best.map(|(_, target)| target)
}

fn seek_towards_target(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    speed_key: &str,
    target: Target,
) {
    entity.current_target = Some(target);
    let speed = resolve_speed(params, speed_key, entity.speed);
    let turn_rate = params
        .get("turn_rate")
        .copied()
        .or_else(|| params.get("accel").copied().map(|a| a * 0.35))
        .unwrap_or(7.0)
        .max(0.0);
    let dir = target.position() - entity.pos;
    if dir.length_squared() > 0.0001 {
        let desired_dir = dir.normalize();
        behavior.dir = rotate_towards_dir(behavior.dir, desired_dir, turn_rate * dt);
        if behavior.dir.length_squared() > 0.0001 {
            entity.vel = behavior.dir.normalize() * speed;
        }
    }
}

fn flee_from_target(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    speed_key: &str,
    target: Target,
) {
    entity.current_target = Some(target);
    let speed = resolve_speed(params, speed_key, entity.speed);
    let turn_rate = params
        .get("turn_rate")
        .copied()
        .or_else(|| params.get("accel").copied().map(|a| a * 0.35))
        .unwrap_or(7.0)
        .max(0.0);
    let dir = entity.pos - target.position();
    if dir.length_squared() > 0.0001 {
        let desired_dir = dir.normalize();
        behavior.dir = rotate_towards_dir(behavior.dir, desired_dir, turn_rate * dt);
        if behavior.dir.length_squared() > 0.0001 {
            entity.vel = behavior.dir.normalize() * speed;
        }
    }
}

pub fn movement_idle(
    entity: &mut EntityInstance,
    _behavior: &mut BehaviorRuntime,
    _dt: f32,
    _params: &MovementParams,
    _ctx: &EntityContext,
) {
    entity.vel = Vec2::ZERO;
}

pub fn movement_wander(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    _ctx: &EntityContext,
) {
    let speed = resolve_speed(params, "wander_speed", entity.speed);
    let interval = params.get("interval").copied().unwrap_or(3.0);
    let turn_rate = params.get("turn_rate").copied().unwrap_or(3.2).max(0.0);
    let steering_range = params
        .get("steering_range")
        .copied()
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);

    if behavior.dir.length_squared() <= 0.0001 {
        let angle = macroquad::rand::gen_range(0.0, std::f32::consts::TAU);
        behavior.dir = vec2(angle.cos(), angle.sin());
    }

    behavior.timer -= dt;
    if behavior.timer <= 0.0 {
        behavior.timer = interval.max(0.1);
        behavior.cooldown = macroquad::rand::gen_range(-steering_range, steering_range);
    }

    let current_angle = behavior.dir.y.atan2(behavior.dir.x);
    let next_angle = current_angle + behavior.cooldown * turn_rate * dt;
    behavior.dir = vec2(next_angle.cos(), next_angle.sin());
    entity.vel = behavior.dir * speed;
}

pub fn movement_seek(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    _ctx: &EntityContext,
) {
    let Some(target) = entity.current_target.as_ref().map(Target::position) else {
        return;
    };
    seek_towards_target(entity, behavior, dt, params, "seek_speed", Target::Position(target));
}

pub fn movement_flee(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    _ctx: &EntityContext,
) {
    let Some(target) = entity.current_target.as_ref().map(Target::position) else {
        return;
    };
    flee_from_target(entity, behavior, dt, params, "flee_speed", Target::Position(target));
}

pub fn movement_watch(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    _ctx: &EntityContext,
) {
    let Some(target) = entity.current_target else {
        return;
    };
    watch_target(entity, behavior, dt, params, target);
}

fn watch_target(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    target: Target,
) {
    entity.current_target = Some(target);
    let to_target = target.position() - entity.pos;
    let dist = to_target.length();
    if dist <= 0.0001 {
        return;
    }

    let seek_range = params.get("seek_range").copied().unwrap_or(300.0).max(0.0);
    let flee_range = params.get("flee_range").copied().unwrap_or(150.0).max(0.0);
    let seek_force = params.get("seek_force").copied().unwrap_or(900.0);
    let flee_force = params.get("flee_force").copied().unwrap_or(900.0);
    let band = (seek_range - flee_range).max(0.0);
    let range_blend = params
        .get("range_blend")
        .copied()
        .unwrap_or((band * 0.25).clamp(8.0, 80.0))
        .max(0.0001);
    let smoothing = params
        .get("watch_smoothing")
        .copied()
        .or_else(|| params.get("smoothing").copied())
        .unwrap_or(10.0)
        .max(0.0);
    let dir = to_target / dist;

    let smoothstep01 = |x: f32| {
        let t = x.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let seek_weight =
        smoothstep01((dist - (seek_range - range_blend)) / (2.0 * range_blend));
    let flee_weight =
        smoothstep01(((flee_range + range_blend) - dist) / (2.0 * range_blend));
    let desired_force = dir * (seek_force * seek_weight) - dir * (flee_force * flee_weight);

    // Smooth force changes so watch steering doesn't jitter/snap.
    let t = (smoothing * dt).clamp(0.0, 1.0);
    behavior.dir = behavior.dir.lerp(desired_force, t);

    entity.vel += behavior.dir;
}

pub fn movement_watch_nearest_entity(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    if let Some(target) = nearest_entity_target(entity, ctx, None) {
        watch_target(entity, behavior, dt, params, target);
    } else {
        entity.current_target = None;
    }
}

pub fn movement_watch_nearest_enemy(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    if let Some(target) = nearest_entity_target(entity, ctx, Some(EntityKind::Enemy)) {
        watch_target(entity, behavior, dt, params, target);
    } else {
        entity.current_target = None;
    }
}

pub fn movement_watch_nearest_friend(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    if let Some(target) = nearest_entity_target(entity, ctx, Some(EntityKind::Friend)) {
        watch_target(entity, behavior, dt, params, target);
    } else {
        entity.current_target = None;
    }
}

pub fn movement_watch_nearest_misc(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    if let Some(target) = nearest_entity_target(entity, ctx, Some(EntityKind::Misc)) {
        watch_target(entity, behavior, dt, params, target);
    } else {
        entity.current_target = None;
    }
}

pub fn movement_watch_player(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    if let Some(player) = ctx.player {
        watch_target(entity, behavior, dt, params, Target::Player(player));
    } else {
        entity.current_target = None;
    }
}

pub fn movement_seek_nearest_entity(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    if let Some(target) = nearest_entity_target(entity, ctx, None) {
        seek_towards_target(entity, behavior, dt, params, "seek_speed", target);
    } else {
        entity.current_target = None;
    }
}

pub fn movement_seek_nearest_enemy(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    if let Some(target) = nearest_entity_target(entity, ctx, Some(EntityKind::Enemy)) {
        seek_towards_target(entity, behavior, dt, params, "seek_speed", target);
    } else {
        entity.current_target = None;
    }
}

pub fn movement_seek_nearest_friend(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    if let Some(target) = nearest_entity_target(entity, ctx, Some(EntityKind::Friend)) {
        seek_towards_target(entity, behavior, dt, params, "seek_speed", target);
    } else {
        entity.current_target = None;
    }
}

pub fn movement_seek_nearest_misc(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    if let Some(target) = nearest_entity_target(entity, ctx, Some(EntityKind::Misc)) {
        seek_towards_target(entity, behavior, dt, params, "seek_speed", target);
    } else {
        entity.current_target = None;
    }
}

pub fn movement_seek_player(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    if let Some(player) = ctx.player {
        seek_towards_target(
            entity,
            behavior,
            dt,
            params,
            "seek_speed",
            Target::Player(player),
        );
    } else {
        entity.current_target = None;
    }
}

pub fn movement_flee_nearest_entity(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    if let Some(target) = nearest_entity_target(entity, ctx, None) {
        flee_from_target(entity, behavior, dt, params, "flee_speed", target);
    } else {
        entity.current_target = None;
    }
}

pub fn movement_flee_nearest_enemy(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    if let Some(target) = nearest_entity_target(entity, ctx, Some(EntityKind::Enemy)) {
        flee_from_target(entity, behavior, dt, params, "flee_speed", target);
    } else {
        entity.current_target = None;
    }
}

pub fn movement_flee_nearest_friend(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    if let Some(target) = nearest_entity_target(entity, ctx, Some(EntityKind::Friend)) {
        flee_from_target(entity, behavior, dt, params, "flee_speed", target);
    } else {
        entity.current_target = None;
    }
}

pub fn movement_flee_nearest_misc(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    if let Some(target) = nearest_entity_target(entity, ctx, Some(EntityKind::Misc)) {
        flee_from_target(entity, behavior, dt, params, "flee_speed", target);
    } else {
        entity.current_target = None;
    }
}

pub fn movement_flee_player(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    if let Some(player) = ctx.player {
        flee_from_target(
            entity,
            behavior,
            dt,
            params,
            "flee_speed",
            Target::Player(player),
        );
    } else {
        entity.current_target = None;
    }
}

pub fn movement_rebound(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    _ctx: &EntityContext,
) {
    let rebound_speed = params
        .get("rebound_speed")
        .copied()
        .or_else(|| params.get("speed").copied())
        .unwrap_or(900.0)
        .max(0.0);
    let rebound_duration = params
        .get("rebound_duration")
        .copied()
        .unwrap_or(0.14)
        .max(0.01);
    let rebound_cooldown = params
        .get("rebound_cooldown")
        .copied()
        .unwrap_or(0.0)
        .max(0.0);

    if behavior.cooldown > 0.0 {
        behavior.cooldown = (behavior.cooldown - dt).max(0.0);
    }
    if behavior.timer <= 0.0 && behavior.cooldown <= 0.0 {
        let mut dir = Vec2::ZERO;
        if let Some(target) = entity.current_target.as_ref().map(Target::position) {
            let away = entity.pos - target;
            if away.length_squared() > 0.0001 {
                dir = away.normalize();
            }
        }
        if dir.length_squared() <= 0.0001 && entity.vel.length_squared() > 0.0001 {
            dir = -entity.vel.normalize();
        }
        if dir.length_squared() > 0.0001 {
            behavior.dir = dir;
            behavior.timer = rebound_duration;
            behavior.cooldown = cooldown_with_erratic(entity, rebound_cooldown);
        }
    }

    if behavior.timer > 0.0 {
        behavior.timer = (behavior.timer - dt).max(0.0);
        let t = (behavior.timer / rebound_duration).clamp(0.0, 1.0);
        let strength = 0.5 + 0.5 * t;
        entity.vel += behavior.dir * rebound_speed * strength;
    }
}

pub fn movement_dash_at_target(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    _ctx: &EntityContext,
) {
    let dash_speed = params.get("dash_speed").copied().unwrap_or(500.0);
    let dash_duration = params.get("dash_duration").copied().unwrap_or(0.14);
    let dash_cooldown = params.get("dash_cooldown").copied().unwrap_or(0.1);

    if behavior.cooldown > 0.0 {
        behavior.cooldown = (behavior.cooldown - dt).max(0.0);
    }
    if behavior.timer > 0.0 {
        behavior.timer = (behavior.timer - dt).max(0.0);
    }

    if behavior.timer <= 0.0 && behavior.cooldown <= 0.0 {
        if let Some(target) = entity.current_target.as_ref().map(Target::position) {
            let dir = target - entity.pos;
            if dir.length_squared() > 0.0001 {
                behavior.dir = dir.normalize();
                behavior.timer = dash_duration;
                behavior.cooldown = cooldown_with_erratic(entity, dash_cooldown);
            }
        }
    }

    if behavior.timer > 0.0 {
        entity.vel += behavior.dir * dash_speed;
    }
}

pub fn movement_curve_dash_at_target(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    _ctx: &EntityContext,
) {
    let dash_speed = params.get("dash_speed").copied().unwrap_or(500.0);
    let dash_duration = params.get("dash_duration").copied().unwrap_or(0.18).max(0.01);
    let dash_cooldown = params.get("dash_cooldown").copied().unwrap_or(0.2).max(0.0);
    let arc_strength = params.get("arc_strength").copied().unwrap_or(0.75).clamp(0.0, 2.0);
    let curve_rate = params.get("curve_rate").copied().unwrap_or(14.0).max(0.0); // radians/sec

    if behavior.cooldown > 0.0 {
        behavior.cooldown = (behavior.cooldown - dt).max(0.0);
    }
    if behavior.timer > 0.0 {
        behavior.timer = (behavior.timer - dt).max(0.0);
    }

    if behavior.timer <= 0.0 && behavior.cooldown <= 0.0 {
        if let Some(target) = entity.current_target.as_ref().map(Target::position) {
            let to_target = target - entity.pos;
            if to_target.length_squared() > 0.0001 {
                let base = to_target.normalize();
                let sign = if macroquad::rand::gen_range(0i32, 2i32) == 0 {
                    -1.0
                } else {
                    1.0
                };
                // Angle-based start direction is more stable than vector blending.
                let start_angle = sign * arc_strength * std::f32::consts::FRAC_PI_2;
                let cos_a = start_angle.cos();
                let sin_a = start_angle.sin();
                let start_dir =
                    vec2(base.x * cos_a - base.y * sin_a, base.x * sin_a + base.y * cos_a)
                        .normalize_or_zero();
                if start_dir.length_squared() > 0.0001 {
                    behavior.dir = start_dir;
                    behavior.timer = dash_duration;
                    behavior.cooldown = cooldown_with_erratic(entity, dash_cooldown);
                }
            }
        }
    }

    if behavior.timer > 0.0 {
        if let Some(target) = entity.current_target.as_ref().map(Target::position) {
            let to_target = target - entity.pos;
            if to_target.length_squared() > 0.0001 {
                let desired = to_target.normalize();
                behavior.dir = rotate_towards_dir(behavior.dir, desired, curve_rate * dt);
            }
        }
        entity.vel += behavior.dir * dash_speed;
    }
}

pub fn movement_bird_ai(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    _ctx: &EntityContext,
) {
    // JS parity (gameNightly/modules/ai.js virabirdAi):
    // if dist <= 200 => sMoveTowards(_, player, -1000)
    // else if dist >= 300 => sMoveTowards(_, player, 1500)
    // then _.dash(random[-1|0|1], random[-1|0|1])
    let flee_range = params.get("flee_range").copied().unwrap_or(200.0);
    let seek_range = params.get("seek_range").copied().unwrap_or(300.0);
    let flee_force = params.get("flee_force").copied().unwrap_or(1000.0);
    let seek_force = params.get("seek_force").copied().unwrap_or(1500.0);
    // Pursuit force used while inside the flee/seek band.
    let mid_seek_force = params
        .get("mid_seek_force")
        .copied()
        .unwrap_or(seek_force * 0.65)
        .max(0.0);
    // Dash defaults follow JS dash() defaults closely:
    // dashSpeed=1200, dashDuration=0.2, dashMCd=1
    let dash_speed = params.get("dash_speed").copied().unwrap_or(1200.0).max(0.0);
    let dash_duration = params.get("dash_duration").copied().unwrap_or(0.2).max(0.0);
    let dash_cooldown = params.get("dash_cooldown").copied().unwrap_or(1.0).max(0.0);
    // If > 0, cap total dash displacement per dash regardless of speed/duration tuning.
    let dash_max_distance = params.get("dash_max_distance").copied().unwrap_or(0.0).max(0.0);
    // Scales discrete random input axis values: {-1,0,1} * dash_input_scale.
    let dash_input_scale = params
        .get("dash_input_scale")
        .copied()
        .unwrap_or(1.0)
        .max(0.0);
    // 0..1 bias of dash heading toward current target (higher = more chase-oriented dashes).
    let dash_target_bias = params
        .get("dash_target_bias")
        .copied()
        .unwrap_or(0.6)
        .clamp(0.0, 1.0);
    // If 0, reroll a few times to avoid a zero dash vector.
    let dash_allow_zero = params.get("dash_allow_zero").copied().unwrap_or(0.0) > 0.5;
    // Heading smoothing between dash bursts (0 = instant, 0.9 = very inertial).
    let dash_turn_smoothing = params
        .get("dash_turn_smoothing")
        .copied()
        .unwrap_or(0.6)
        .clamp(0.0, 0.95);
    // Max heading change allowed at each new dash (radians).
    let dash_max_turn = params
        .get("dash_max_turn")
        .copied()
        .unwrap_or(std::f32::consts::FRAC_PI_2)
        .max(0.0);
    // When 0, dash displacement is isolated (prevents seek/flee overshooting during dashes).
    let steer_while_dashing = params
        .get("steer_while_dashing")
        .copied()
        .unwrap_or(0.0)
        > 0.5;
    // Optional multiplier for seek/flee acceleration band.
    let move_force_scale = params.get("move_force_scale").copied().unwrap_or(1.0).max(0.0);
    let was_dashing = behavior.timer > 0.0;

    if !was_dashing || steer_while_dashing {
        if let Some(target) = entity.current_target.as_ref().map(Target::position) {
            let to_target = target - entity.pos;
            let dist = to_target.length();
            if dist > 0.0001 {
                let toward = to_target / dist;
                if dist <= flee_range {
                    entity.vel += -toward * flee_force * move_force_scale;
                } else if dist >= seek_range {
                    entity.vel += toward * seek_force * move_force_scale;
                } else {
                    entity.vel += toward * mid_seek_force * move_force_scale;
                }
            }
        }
    }

    if behavior.cooldown > 0.0 {
        behavior.cooldown = (behavior.cooldown - dt).max(0.0);
    }
    if behavior.timer > 0.0 {
        behavior.timer = (behavior.timer - dt).max(0.0);
    }

    if behavior.timer <= 0.0 && behavior.cooldown <= 0.0 {
        let mut dash_dir = Vec2::ZERO;
        for _ in 0..4 {
            let rx =
                macroquad::rand::gen_range(0i32, 2i32) - macroquad::rand::gen_range(0i32, 2i32);
            let ry =
                macroquad::rand::gen_range(0i32, 2i32) - macroquad::rand::gen_range(0i32, 2i32);
            dash_dir = vec2(rx as f32, ry as f32) * dash_input_scale;
            if dash_allow_zero || dash_dir.length_squared() > 0.0001 {
                break;
            }
        }
        if !dash_allow_zero && dash_dir.length_squared() <= 0.0001 {
            let angle = macroquad::rand::gen_range(0.0, std::f32::consts::TAU);
            dash_dir = vec2(angle.cos(), angle.sin()) * dash_input_scale.max(1.0);
        }
        if dash_target_bias > 0.0 {
            if let Some(target) = entity.current_target.as_ref().map(Target::position) {
                let toward = (target - entity.pos).normalize_or_zero();
                if toward.length_squared() > 0.0001 {
                    let random_n = dash_dir.normalize_or_zero();
                    let guided = (random_n * (1.0 - dash_target_bias) + toward * dash_target_bias)
                        .normalize_or_zero();
                    if guided.length_squared() > 0.0001 {
                        dash_dir = guided;
                    }
                }
            }
        }

        let mut dash_dir = dash_dir.normalize_or_zero();
        if behavior.dir.length_squared() > 0.0001 && dash_dir.length_squared() > 0.0001 {
            let blended = (behavior.dir * dash_turn_smoothing
                + dash_dir * (1.0 - dash_turn_smoothing))
                .normalize_or_zero();
            let target_dir = if blended.length_squared() > 0.0001 {
                blended
            } else {
                dash_dir
            };
            dash_dir = rotate_towards_dir(behavior.dir, target_dir, dash_max_turn);
        }
        behavior.dir = dash_dir;
        behavior.timer = dash_duration;
        behavior.cooldown = cooldown_with_erratic(entity, dash_cooldown);
    }

    if behavior.timer > 0.0 {
        // Match JS dash behavior: direct positional impulse during dash window.
        let effective_dash_speed = if dash_max_distance > 0.0 && dash_duration > 0.0 {
            dash_speed.min(dash_max_distance / dash_duration)
        } else {
            dash_speed
        };
        entity.pos += behavior.dir * effective_dash_speed * dt;
    }
}

pub fn movement_virabird_ai(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    movement_bird_ai(entity, behavior, dt, params, ctx);
}

pub fn movement_bird_orbit(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    _ctx: &EntityContext,
) {
    let orbit_speed = params.get("orbit_speed").copied().unwrap_or(1200.0);
    let orbit_radius = params.get("orbit_radius").copied().unwrap_or(80.0);
    let erratic_factor = params.get("erratic_factor").copied().unwrap_or(0.8);

    // Track orbit direction (alternates for more erratic behavior)
    if behavior.timer <= 0.0 {
        behavior.timer = 1.0;
        // Randomly flip orbit direction occasionally
        if macroquad::rand::gen_range(0, 10) < 3 {
            behavior.dir = -behavior.dir;
        }
    }
    behavior.timer -= dt;

    if let Some(target) = entity.current_target.as_ref().map(Target::position) {
        let to_target = target - entity.pos;
        let dist = to_target.length();

        if dist > 0.0001 {
            let toward = to_target / dist;
            
            // Calculate tangent (perpendicular to direction to target)
            // This creates the orbital motion
            let tangent = vec2(-toward.y, toward.x);
            
            // Main orbital velocity
            let orbit_vel = tangent * orbit_speed * behavior.dir;
            
            // Erratic random movement
            let erratic = vec2(
                macroquad::rand::gen_range(-1.0, 1.0),
                macroquad::rand::gen_range(-1.0, 1.0),
            ) * erratic_factor * orbit_speed;
            
            // Apply velocity
            entity.vel += orbit_vel + erratic;
            
            // Distance correction - maintain orbit radius
            let radius_diff = dist - orbit_radius;
            if radius_diff > 5.0 {
                // Too far - move closer
                entity.vel += toward * orbit_speed * 0.5;
            } else if radius_diff < -5.0 {
                // Too close - back away
                entity.vel += -toward * orbit_speed * 0.5;
            }
        }
    }
}
