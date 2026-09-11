//! A radial ("zoom") blur post-process, faded in while a narration bubble is
//! shown (see `explain_bubble`) - the dim backdrop alone read as a flat tint, not
//! as "focus pulling toward the dialog", so this adds real screen-space blur on
//! top of it. Colour-only (no depth reads), unlike depth-of-field - this app's
//! WebGL2 target already can't do DOF ("depth textures aren't supported
//! correctly" - see `resource_loader`'s startup log), so a depth-based effect
//! wasn't an option; a post-process that only samples the already-rendered
//! colour texture several times and averages doesn't have that dependency.
//!
//! Structure follows Bevy's own "custom post processing" pattern: a `ViewNode`
//! inserted into the `Core2d` render graph right after tonemapping, reading the
//! prior pass's output texture and writing a blurred version via a small WGSL
//! shader (`radial_blur.wgsl`, embedded into the binary with `load_internal_asset!`
//! rather than loaded as a runtime asset, so there's no wasm asset-path to get
//! wrong). `RadialBlurSettings.intensity` is a plain component on the camera,
//! animated by `animate_radial_blur` using real time for the same pause-safety
//! reason `explain_bubble`'s pop-in animation does.

use bevy::{
    asset::load_internal_asset,
    core_pipeline::{
        core_2d::graph::{Core2d, Node2d},
        fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    },
    ecs::query::QueryItem,
    prelude::*,
    render::{
        extract_component::{
            ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
            UniformComponentPlugin,
        },
        render_graph::{
            NodeRunError, RenderGraphApp, RenderGraphContext, RenderLabel, ViewNode, ViewNodeRunner,
        },
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer},
            BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, CachedRenderPipelineId,
            ColorTargetState, ColorWrites, FragmentState, MultisampleState, Operations,
            PipelineCache, PrimitiveState, RenderPassColorAttachment, RenderPassDescriptor,
            RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, Shader,
            ShaderStages, ShaderType, TextureFormat, TextureSampleType,
        },
        renderer::{RenderContext, RenderDevice},
        texture::BevyDefault,
        view::ViewTarget,
        RenderApp,
    },
};

use bevy_pancam::PanCam;

use crate::components::camera::BubbleCamera;
use crate::components::node::ExplainBubble;

const SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(0x8f2c9a1e5d3b4a7c_u128);

const FADE_SECS: f32 = 0.25;
const MAX_INTENSITY: f32 = 1.0;

/// Lives on the camera entity. `center` is in the render target's own UV space
/// (0..1, y-down) - viewport center, not tied to any particular node, since the
/// blur is meant to draw the eye inward generally rather than track one point.
#[derive(Component, Clone, Copy, ExtractComponent, ShaderType)]
pub struct RadialBlurSettings {
    pub intensity: f32,
    pub center: Vec2,
}

impl Default for RadialBlurSettings {
    fn default() -> Self {
        Self {
            intensity: 0.0,
            center: Vec2::splat(0.5),
        }
    }
}

pub struct RadialBlurPlugin;

impl Plugin for RadialBlurPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, SHADER_HANDLE, "radial_blur.wgsl", Shader::from_wgsl);

        app.add_plugins((
            ExtractComponentPlugin::<RadialBlurSettings>::default(),
            UniformComponentPlugin::<RadialBlurSettings>::default(),
        ));

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .add_render_graph_node::<ViewNodeRunner<RadialBlurNode>>(Core2d, RadialBlurLabel)
            .add_render_graph_edges(
                Core2d,
                (
                    Node2d::Tonemapping,
                    RadialBlurLabel,
                    Node2d::EndMainPassPostProcessing,
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.init_resource::<RadialBlurPipeline>();
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct RadialBlurLabel;

#[derive(Default)]
struct RadialBlurNode;

impl ViewNode for RadialBlurNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static DynamicUniformIndex<RadialBlurSettings>,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, settings_index): QueryItem<Self::ViewQuery>,
        world: &bevy::ecs::world::World,
    ) -> Result<(), NodeRunError> {
        let pipeline_res = world.resource::<RadialBlurPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_res.pipeline_id) else {
            return Ok(());
        };
        let settings_uniforms = world.resource::<ComponentUniforms<RadialBlurSettings>>();
        let Some(settings_binding) = settings_uniforms.uniforms().binding() else {
            return Ok(());
        };

        let post_process = view_target.post_process_write();
        let bind_group = render_context.render_device().create_bind_group(
            "radial_blur_bind_group",
            &pipeline_res.layout,
            &BindGroupEntries::sequential((
                post_process.source,
                &pipeline_res.sampler,
                settings_binding.clone(),
            )),
        );

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("radial_blur_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &bind_group, &[settings_index.index()]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

#[derive(Resource)]
struct RadialBlurPipeline {
    layout: BindGroupLayout,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for RadialBlurPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let layout = render_device.create_bind_group_layout(
            "radial_blur_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<RadialBlurSettings>(true),
                ),
            ),
        );
        let sampler = render_device.create_sampler(&SamplerDescriptor::default());

        let pipeline_id =
            world
                .resource_mut::<PipelineCache>()
                .queue_render_pipeline(RenderPipelineDescriptor {
                    label: Some("radial_blur_pipeline".into()),
                    layout: vec![layout.clone()],
                    vertex: fullscreen_shader_vertex_state(),
                    fragment: Some(FragmentState {
                        shader: SHADER_HANDLE,
                        shader_defs: vec![],
                        entry_point: "fragment".into(),
                        targets: vec![Some(ColorTargetState {
                            format: TextureFormat::bevy_default(),
                            blend: None,
                            write_mask: ColorWrites::ALL,
                        })],
                    }),
                    primitive: PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: MultisampleState::default(),
                    push_constant_ranges: vec![],
                });

        Self {
            layout,
            sampler,
            pipeline_id,
        }
    }
}

/// Fades `RadialBlurSettings.intensity` in while any narration bubble is active
/// and back out once none are, over real (unpaused) time - same reasoning as
/// `explain_bubble::animate_bubble_pop`.
pub fn animate_radial_blur(
    real_time: Res<Time<Real>>,
    bubbles: Query<(), With<ExplainBubble>>,
    mut settings: Query<&mut RadialBlurSettings>,
) {
    let target = if bubbles.is_empty() {
        0.0
    } else {
        MAX_INTENSITY
    };
    let step = real_time.delta_seconds() / FADE_SECS * MAX_INTENSITY;
    for mut s in settings.iter_mut() {
        if s.intensity < target {
            s.intensity = (s.intensity + step).min(target);
        } else if s.intensity > target {
            s.intensity = (s.intensity - step).max(target);
        }
    }
}

/// Keeps the bubble-compositing camera's transform and projection identical to
/// the main (`PanCam`-driven) camera's, every frame - PanCam only ever touches
/// the entity it's attached to, so without this the bubble camera would stay put
/// while the user pans/zooms the world camera, visibly detaching bubbles from
/// the nodes they're anchored to.
pub fn sync_bubble_camera(
    main: Query<(&Transform, &OrthographicProjection), (With<PanCam>, Without<BubbleCamera>)>,
    mut bubble: Query<(&mut Transform, &mut OrthographicProjection), With<BubbleCamera>>,
) {
    let Ok((main_transform, main_projection)) = main.get_single() else {
        return;
    };
    let Ok((mut transform, mut projection)) = bubble.get_single_mut() else {
        return;
    };
    *transform = *main_transform;
    *projection = main_projection.clone();
}
