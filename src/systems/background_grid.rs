//! The dotted canvas background - a single big quad shaded by a WGSL
//! fragment shader (`background_grid.wgsl`) that computes the dot pattern
//! per-pixel from world position, rather than tiling a small dot texture
//! across the canvas.
//!
//! The old approach (`ImageScaleMode::Tiled` over a 40x40px dot texture)
//! built one mesh "slice" per tile - at the original 40-unit spacing across
//! a 4000x4000 world area that was already 10000 slices (`bevy_sprite`
//! warns about exactly this: "One of your tiled textures has generated
//! 10000 slices"). Shrinking the spacing to look denser would have made
//! this quadratically worse (10-unit spacing = 160000 slices) for no real
//! reason - the pattern is the same handful of instructions repeated per
//! pixel regardless of how many dots are on screen, which is exactly what a
//! fragment shader is for. This mirrors `radial_blur.rs`'s reasoning for
//! embedding its own WGSL via `load_internal_asset!` rather than a runtime
//! asset path (no wasm asset-path to resolve).

use bevy::asset::load_internal_asset;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::sprite::{Material2d, Material2dPlugin, MaterialMesh2dBundle};

/// Spacing between grid dots, in world units. Also reused by `drag.rs` as the node
/// snap-to-grid interval, so dragged nodes land exactly on the dots drawn here.
pub const GRID_SPACING: f32 = 10.0;
const DOT_RADIUS: f32 = 1.0;
const GRID_SIZE: f32 = 4000.0;
/// Straight 0..1 floats (not sRGB `u8`s like the old texture-based version
/// used) since this now goes straight into a shader uniform.
const DOT_COLOR: Vec4 = Vec4::new(130.0 / 255.0, 130.0 / 255.0, 130.0 / 255.0, 150.0 / 255.0);

const SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(0x5b1e7a2c9f0d4468_u128);

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct GridMaterial {
    #[uniform(0)]
    color: Vec4,
    /// x: spacing, y: dot radius (both world units) - packed alongside
    /// `color` at the same binding rather than as their own `#[uniform]`
    /// fields purely to keep the generated bind group to one buffer.
    #[uniform(0)]
    grid_params: Vec4,
}

impl Material2d for GridMaterial {
    fn fragment_shader() -> bevy::render::render_resource::ShaderRef {
        SHADER_HANDLE.into()
    }
    // No alpha_mode override needed - unlike bevy_pbr's 3D materials,
    // Material2d always renders through the Transparent2d phase (see
    // Material2dPlugin::build's `add_render_command::<Transparent2d, ..>`),
    // so the dots already blend over whatever's behind them (the
    // discard'd pixels in between included) with no extra opt-in.
}

pub struct BackgroundGridPlugin;

impl Plugin for BackgroundGridPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            SHADER_HANDLE,
            "background_grid.wgsl",
            Shader::from_wgsl
        );
        app.add_plugins(Material2dPlugin::<GridMaterial>::default());
    }
}

pub fn setup_grid(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GridMaterial>>,
) {
    commands.spawn(MaterialMesh2dBundle {
        mesh: meshes.add(Rectangle::new(GRID_SIZE, GRID_SIZE)).into(),
        material: materials.add(GridMaterial {
            color: DOT_COLOR,
            grid_params: Vec4::new(GRID_SPACING, DOT_RADIUS, 0.0, 0.0),
        }),
        transform: Transform::from_translation(Vec3::new(0.0, 0.0, -1.0)),
        ..default()
    });
}
