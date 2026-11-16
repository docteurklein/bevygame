use bevy::prelude::*;
use bevy::picking::backend::ray::RayMap;
use bevy::picking::events::{PointerPress, PointerRelease};
use bevy::{
    camera_controller::free_camera::{
        FreeCamera, FreeCameraPlugin,
    },
};
use bevytiles::{height::HeightGrid, prelude::*, store::Tile, TerrainSet};
// use bevy_hanabi::prelude::*;
use bevy_rapier3d::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // .add_plugins(HanabiPlugin)
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugins(RapierPickingPlugin)
        // .add_plugins(RapierDebugRenderPlugin::default())
        .insert_resource(WorldConfig::from_lat_lon(46.02317177011591, 3.6486635395039237))
        .add_plugins(TerrainPlugin)
        .add_plugins(FreeCameraPlugin)
        .add_systems(Startup, |mut c: Commands, w: Res<WorldConfig>| {
            c.spawn((
                Transform::from_translation(w.initial_position(5000.0)),
                Camera3d::default(),
                TerrainCamera,
                FreeCamera {
                    sensitivity: 0.2,
                    friction: 25.0,
                    walk_speed: 900.0,
                    run_speed: 9000.0,
                    ..default()
                },
            ));
        })
        .add_systems(Startup, setup_ball_visuals)
        // .add_systems(Update, |
        //     mut cams: Query<&mut Transform, With<TerrainCamera>>,
        //     freecam: Query<&Transform, With<FreeCameraState>>
        // | {
        //     let Ok(mut tf) = cams.single_mut() else {
        //         return;
        //     };
        //     let Ok(fc) = freecam.single() else {
        //         return;
        //     };
        //     // tf.translation = fc.translation;
        //     // tf.rotation = fc.rotation;
        //     // tf.scale = fc.scale;
        // })
        // .add_systems(Startup, setup)
        .init_resource::<DragState>()
        .add_systems(Update, fire)
        .add_systems(Update, move_drag_anchor)
        .add_systems(Update, spawn_terrain_colliders.after(TerrainSet::Promote))
        .run()
    ;
}

// fn setup(mut c: Commands, mut effects: ResMut<Assets<EffectAsset>>, w: Res<WorldConfig>) {
//   // Define a color gradient from red to transparent black
//   let mut gradient = bevy_hanabi::prelude::Gradient::new();
//   gradient.add_key(0.0, Vec4::new(1., 0., 0., 1.));
//   gradient.add_key(1.0, Vec4::splat(0.));

//   // Create a new expression module
//   let mut module = Module::default();

//   // On spawn, randomly initialize the position of the particle
//   // to be over the surface of a sphere of radius 2 units.
//   let init_pos = SetPositionSphereModifier {
//       center: module.lit(Vec3::ZERO),
//       radius: module.lit(2.),
//       dimension: ShapeDimension::Surface,
//   };

//   // Also initialize a radial initial velocity to 6 units/sec
//   // away from the (same) sphere center.
//   let init_vel = SetVelocitySphereModifier {
//       center: module.lit(Vec3::ZERO),
//       speed: module.lit(6.),
//   };

//   // Initialize the total lifetime of the particle, that is
//   // the time for which it's simulated and rendered. This modifier
//   // is almost always required, otherwise the particles won't show.
//   let lifetime = module.lit(10.); // literal value "10.0"
//   let init_lifetime = SetAttributeModifier::new(
//       Attribute::LIFETIME, lifetime);

//   // Every frame, add a gravity-like acceleration downward
//   let accel = module.lit(Vec3::new(0., -3., 0.));
//   let update_accel = AccelModifier::new(accel);

//   // Create the effect asset
//   let effect = EffectAsset::new(
//     // Maximum number of particles alive at a time
//     1024,
//     // Spawn at a rate of 5 particles per second
//     SpawnerSettings::rate(5.0.into()),
//     // Move the expression module into the asset
//     module
//   )
//   .with_name("MyEffect")
//   .init(init_pos)
//   .init(init_vel)
//   .init(init_lifetime)
//   .update(update_accel)
//   // Render the particles with a color gradient over their
//   // lifetime. This maps the gradient key 0 to the particle spawn
//   // time, and the gradient key 1 to the particle death (10s).
//   .render(ColorOverLifetimeModifier { gradient, ..default() });

//   // Insert into the asset system
//   let effect_handle = effects.add(effect);

//     c.spawn((
//         ParticleEffect::new(effect_handle),
//         Transform::from_translation(w.initial_position(5000.0))
//     ));
// }
/// Speed in m/s at which fired balls are launched from the camera.
const BALL_LAUNCH_SPEED: f32 = 40.0;

/// Shared sphere mesh used to render fired balls (matches `Collider::ball(0.5)`).
#[derive(Resource)]
struct BallVisuals {
    mesh: Handle<Mesh>,
}

fn setup_ball_visuals(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(BallVisuals {
        mesh: meshes.add(Sphere::new(0.5).mesh().ico(5).unwrap()),
    });
}

/// Deterministic pseudo-random, vivid color for the next ball.
fn random_ball_color(seed: u32) -> Color {
    let mut x = seed.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    Color::hsl((x % 360) as f32, 0.8, 0.55)
}

#[derive(Component)]
struct DragAnchor;

#[derive(Resource, Default)]
struct DragState {
    anchor: Option<Entity>,
    joint: Option<Entity>,
    drag_depth: f32,
}

fn on_drag_press(
    trigger: On<PointerPress>,
    mut commands: Commands,
    mut drag: ResMut<DragState>,
    bodies: Query<&RigidBody>,
    cameras: Query<&GlobalTransform, With<Camera>>,
) {
    if drag.joint.is_some() { return; }
    let target = trigger.entity;
    if bodies.get(target) != Ok(&RigidBody::Dynamic) { return; }
    let Some(hit_point) = trigger.hit.position else { return; };
    let anchor = commands.spawn((
        DragAnchor,
        Transform::from_translation(hit_point),
        RigidBody::KinematicPositionBased,
        Collider::ball(0.01),
        CollisionGroups::new(Group::NONE, Group::NONE),
    )).id();
    let spring = SpringJointBuilder::new(0.0, 500.0, 20.0)
        .local_anchor2(Vec3::ZERO)
        .contacts_enabled(false)
        .build();
    let joint = commands.spawn(ImpulseJoint::new(anchor, spring)).id();
    commands.entity(target).add_child(joint);
    if let Ok(cam_tf) = cameras.single() {
        drag.drag_depth = cam_tf.translation().distance(hit_point);
    }
    drag.anchor = Some(anchor);
    drag.joint = Some(joint);
}

fn on_drag_release(
    _trigger: On<PointerRelease>,
    mut commands: Commands,
    mut drag: ResMut<DragState>,
) {
    if let Some(j) = drag.joint.take() { commands.entity(j).despawn(); }
    if let Some(a) = drag.anchor.take() { commands.entity(a).despawn(); }
}

fn move_drag_anchor(
    drag: Res<DragState>,
    ray_map: Res<RayMap>,
    mut anchors: Query<&mut Transform, With<DragAnchor>>,
) {
    let Some(anchor) = drag.anchor else { return; };
    let Ok(mut tf) = anchors.get_mut(anchor) else { return; };
    // Use the primary pointer's latest ray.
    for (_, ray) in ray_map.map.iter() {
        tf.translation = ray.origin + *ray.direction * drag.drag_depth;
        break;
    }
}

fn fire(
    mut commands: Commands,
    buttons: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    visuals: Res<BallVisuals>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ball_count: Local<u32>,
) {
    if !buttons.pressed(KeyCode::Space) {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let Ok((camera, cam_transform)) = cameras.single() else { return };
    // Convert the click to a world-space ray and spawn along it.
    let Ok(ray) = camera.viewport_to_world(cam_transform, cursor) else { return };
    let cam_pos = cam_transform.translation();
    let spawn_pos = ray.origin + ray.direction * 10.0;
    // Launch the ball away from the free camera, toward the cursor.
    let launch_dir = (spawn_pos - cam_pos).normalize_or_zero();
    let material = materials.add(StandardMaterial {
        base_color: random_ball_color(*ball_count),
        perceptual_roughness: 0.4,
        ..default()
    });
    *ball_count = ball_count.wrapping_add(1);
    commands
        .spawn((RigidBody::Dynamic, RapierPickable, Visibility::default()))
        .insert(Collider::ball(0.5))
        .insert(Restitution::coefficient(0.7))
        .insert(Mesh3d(visuals.mesh.clone()))
        .insert(MeshMaterial3d(material))
        .insert(Transform::from_translation(spawn_pos))
        .insert(Velocity::linear(launch_dir * BALL_LAUNCH_SPEED))
        .observe(on_drag_press)
        .observe(on_drag_release)
    ;
}
/// Target spacing between collider vertices (meters). Bigger = coarser/faster.
const COLLIDER_SPACING_M: f32 = 100.0;
/// Hard cap per axis, so a 256² tile doesn't produce ~130k triangles.
const MAX_COLLIDER_SAMPLES: usize = 64;

/// Number of height samples to use per axis for a tile's collider.
fn collider_dims(grid: &HeightGrid, tile_size: f32) -> (usize, usize) {
    // Aim for a constant world-space resolution across zoom levels.
    let target = (tile_size / COLLIDER_SPACING_M).ceil() as usize + 1;

    let ncols = target.min(MAX_COLLIDER_SAMPLES).min(grid.w as usize).max(2);
    let nrows = target.min(MAX_COLLIDER_SAMPLES).min(grid.h as usize).max(2);
    (ncols, nrows) // (X samples, Z samples)
}

/// Spawn one decimated heightfield collider per newly promoted terrain tile.
///
/// Attached as a child of the tile entity, so the plugin's rebasing moves it
/// with the tile and eviction (recursive despawn) removes it automatically.
fn spawn_terrain_colliders(
    mut commands: Commands,
    grids: Res<HeightGrids>,
    added: Query<(Entity, &Tile), Added<Tile>>,
    rendering: Res<RenderingConfig>,
) {
    for (tile_entity, tile) in &added {
        let Some(grid) = grids.0.get(&tile.key) else {
            continue;
        };
        if grid.w < 2 || grid.h < 2 {
            continue;
        }

        let (ncols, nrows) = collider_dims(grid, tile.size);

        // Column-major: data[z + x * nrows]. Sampling with the same UV
        // convention as the vertex shader keeps vertices on the surface.
        let mut heights = Vec::with_capacity(ncols * nrows);
        for x in 0..ncols {
            let u = x as f32 / (ncols - 1) as f32;
            for z in 0..nrows {
                let v = z as f32 / (nrows - 1) as f32;
                heights.push(grid.sample(u, v)); // already minus 32768, in meters
            }
        }

        commands.entity(tile_entity).with_child((
            // Identity local transform: lets transform propagation give the
            // collider the tile's GlobalTransform (rebase-friendly).
            Transform::default(),
            Collider::heightfield(
                heights,
                nrows, // num_rows -> Z
                ncols, // num_cols -> X
                Vec3::new(
                    tile.size,              // full X extent in meters
                    rendering.height_scale, // same exaggeration as the shader
                    tile.size,              // full Z extent in meters
                ),
            ),
        ));
    }
}
