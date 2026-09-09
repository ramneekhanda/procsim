use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::ImageScaleMode;

/// Spacing between grid dots, in world units. Also reused by `drag.rs` as the node
/// snap-to-grid interval, so dragged nodes land exactly on the dots drawn here.
pub const GRID_SPACING: f32 = 40.0;
const DOT_RADIUS: f32 = 2.0;
const GRID_SIZE: f32 = 4000.0;
const DOT_COLOR: [u8; 4] = [130, 130, 130, 150];

fn build_dot_tile() -> Image {
    let tile_size = GRID_SPACING as u32;
    let mut data = vec![0u8; (tile_size * tile_size * 4) as usize];
    let center = tile_size as f32 / 2.0;

    for y in 0..tile_size {
        for x in 0..tile_size {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            if dx * dx + dy * dy <= DOT_RADIUS * DOT_RADIUS {
                let i = ((y * tile_size + x) * 4) as usize;
                data[i..i + 4].copy_from_slice(&DOT_COLOR);
            }
        }
    }

    Image::new(
        Extent3d {
            width: tile_size,
            height: tile_size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

pub fn setup_grid(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let texture = images.add(build_dot_tile());

    commands.spawn((
        SpriteBundle {
            texture,
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, -1.0)),
            sprite: Sprite {
                custom_size: Some(Vec2::new(GRID_SIZE, GRID_SIZE)),
                ..Default::default()
            },
            ..default()
        },
        ImageScaleMode::Tiled {
            tile_x: true,
            tile_y: true,
            stretch_value: 1.0,
        },
    ));
}
