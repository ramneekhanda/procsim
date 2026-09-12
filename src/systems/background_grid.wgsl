// Procedural dot grid, sampled per-pixel in world space instead of tiling a
// small dot texture across thousands of mesh slices (the old approach - see
// background_grid.rs's doc comment for why that stopped being viable once
// the dot spacing shrank).
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct GridMaterial {
    color: vec4<f32>,
    // x: spacing between dots (world units), y: dot radius (world units).
    grid_params: vec4<f32>,
};

@group(2) @binding(0) var<uniform> material: GridMaterial;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let spacing = material.grid_params.x;
    let dot_radius = material.grid_params.y;

    // Position within the current grid cell, centered on the cell's own
    // center (so a dot at world origin lands exactly on a cell center, not
    // a corner) - independent of how many cells actually exist, so the
    // per-pixel cost never scales with grid size or how fine the spacing is.
    let cell_pos = (fract(mesh.world_position.xy / spacing) - 0.5) * spacing;
    let dist = length(cell_pos);

    if dist > dot_radius {
        discard;
    }
    return material.color;
}
