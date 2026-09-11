use bevy::prelude::*;

/// Marks the second camera that composites narration bubbles (see
/// `systems::explain_bubble`) on top of the main world camera's output.
/// Renders `RenderLayers::layer(1)` only, with a transparent clear, at a higher
/// `Camera.order` than the main camera - so bubble content draws over an already
/// fully-rendered (and, while a bubble is up, radially blurred) world without
/// itself being blurred, since `systems::radial_blur`'s post-process only runs on
/// views carrying a `RadialBlurSettings` component, which this camera doesn't have.
/// `systems::radial_blur::sync_bubble_camera` keeps its transform/projection
/// identical to the main camera's every frame, since PanCam only drives the main
/// camera and the bubble camera has to track the same pan/zoom to stay aligned
/// with the node it's anchored to.
#[derive(Component, Debug)]
pub struct BubbleCamera;
