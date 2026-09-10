use novadraw_render::{
    BackendCapabilities, ImageResourceRef, NdCanvas, RenderCapability, RenderCommandKind,
    ResourceId, UnsupportedRenderCapability,
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
    canvas: &mut NdCanvas,
    frame: &Scene3DFrame,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) {
    canvas.draw_image_with_size(frame.color_target, x, y, width, height);
}

#[test]
fn scene3d_frame_embeds_through_the_existing_2d_image_boundary() {
    let frame = Scene3DFrame {
        color_target: ImageResourceRef::new(ResourceId::new(Uuid::nil(), 1), 1, 2, 2, 1.0),
    };
    let mut canvas = NdCanvas::new();

    compose_scene3d_frame(&mut canvas, &frame, 10.0, 20.0, 300.0, 200.0);

    let RenderCommandKind::Image {
        image,
        dest_rect,
        src_rect,
        ..
    } = &canvas.commands()[0].kind
    else {
        panic!("Scene3D embedding must remain an ordinary 2D image command");
    };

    assert_eq!((image.width(), image.height()), (2, 2));
    assert_eq!((dest_rect[0].x, dest_rect[0].y), (10.0, 20.0));
    assert_eq!((dest_rect[1].x, dest_rect[1].y), (310.0, 220.0));
    assert!(src_rect.is_none());
}
