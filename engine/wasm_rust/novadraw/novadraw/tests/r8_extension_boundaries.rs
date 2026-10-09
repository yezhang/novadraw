use novadraw::render::{
    BackendCapabilities, CommandRecorder, ImageResourceRef, RenderCapability, RenderCommandKind,
    ResourceId, UnsupportedRenderCapability,
};
use novadraw::{
    Rectangle,
    graphics::{GraphicsError, PaintContext},
};
use uuid::Uuid;

#[test]
fn projective_composition_requires_explicit_backend_support() {
    let capability = RenderCapability::ProjectiveComposition;

    assert_eq!(
        BackendCapabilities::FULL_FRAME_ONLY.require(capability),
        Err(UnsupportedRenderCapability { capability })
    );
    assert_eq!(
        BackendCapabilities::RETAINED_PARTIAL.require(capability),
        Err(UnsupportedRenderCapability { capability })
    );

    let supported = BackendCapabilities {
        projective_composition: true,
        ..BackendCapabilities::RETAINED_PARTIAL
    };
    assert_eq!(supported.require(capability), Ok(()));
    assert_eq!(
        BackendCapabilities::FULL_FRAME_ONLY.require(RenderCapability::GlyphRuns),
        Err(UnsupportedRenderCapability {
            capability: RenderCapability::GlyphRuns,
        })
    );
    assert_eq!(
        BackendCapabilities::RETAINED_PARTIAL.require(RenderCapability::GlyphRuns),
        Ok(())
    );
    assert_eq!(
        BackendCapabilities::RETAINED_PARTIAL.require(RenderCapability::ImageResources),
        Ok(())
    );
}

struct Scene3DFrame {
    color_target: ImageResourceRef,
}

fn compose_scene3d_frame(
    context: &mut PaintContext<'_>,
    frame: &Scene3DFrame,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), GraphicsError> {
    context.draw_image(frame.color_target, Rectangle::new(x, y, width, height))
}

#[test]
fn scene3d_frame_embeds_through_the_existing_2d_image_boundary() {
    let frame = Scene3DFrame {
        color_target: ImageResourceRef::new(ResourceId::new(Uuid::nil(), 1), 1, 2, 2, 1.0),
    };
    let mut recorder = CommandRecorder::new();
    compose_scene3d_frame(&mut recorder.paint(), &frame, 10.0, 20.0, 300.0, 200.0).unwrap();

    let RenderCommandKind::Image {
        image, dest_rect, ..
    } = &recorder.commands()[0].kind
    else {
        panic!("Scene3D embedding must remain an ordinary 2D image command");
    };

    assert_eq!((image.width(), image.height()), (2, 2));
    assert_eq!(*dest_rect, Rectangle::new(10.0, 20.0, 300.0, 200.0));
}
