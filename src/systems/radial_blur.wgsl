// Radial ("zoom") blur post-process: pulls samples progressively toward `center`,
// so the whole frame streaks inward - reads as focus pulling toward the narration
// bubble. See `systems::radial_blur` for how `settings.intensity` is driven.
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct RadialBlurSettings {
    intensity: f32,
    center: vec2<f32>,
}
@group(0) @binding(2) var<uniform> settings: RadialBlurSettings;

const SAMPLES: i32 = 10;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    if settings.intensity <= 0.001 {
        return textureSample(screen_texture, texture_sampler, in.uv);
    }

    let to_center = settings.center - in.uv;
    var color = vec4<f32>(0.0);
    for (var i = 0; i < SAMPLES; i++) {
        // t sweeps 0..~intensity*0.03, pulling each successive sample closer to
        // `center` - averaging them is what produces the inward streak. Kept
        // modest on purpose: this runs on the world camera only (see
        // `systems::radial_blur` / the second, unblurred bubble camera), but a
        // strong blur still fights the connectors/text it's applied to.
        let t = settings.intensity * 0.03 * (f32(i) / f32(SAMPLES - 1));
        let uv = in.uv + to_center * t;
        color += textureSample(screen_texture, texture_sampler, clamp(uv, vec2(0.0), vec2(1.0)));
    }
    return color / f32(SAMPLES);
}
