use novadraw::geometry::Rectangle;
use novadraw::render::{ImageDrawError, ImageResourceRef, NdCanvas, RenderCommandKind, ResourceId};
use uuid::Uuid;

fn image() -> ImageResourceRef {
    ImageResourceRef::new(ResourceId::new(Uuid::nil(), 1), 3, 80, 40, 2.0)
}

#[test]
fn full_image_commands_record_the_physical_source_bounds() {
    let mut canvas = NdCanvas::new();

    canvas.draw_image(image(), 4.0, 5.0);
    canvas.draw_image_with_size(image(), 10.0, 20.0, 30.0, 40.0);

    let RenderCommandKind::Image {
        source_rect,
        dest_rect,
        ..
    } = canvas.commands()[0].kind
    else {
        panic!("expected Image");
    };
    assert_eq!(source_rect, Rectangle::new(0.0, 0.0, 80.0, 40.0));
    assert_eq!(dest_rect, Rectangle::new(4.0, 5.0, 40.0, 20.0));

    let RenderCommandKind::Image {
        source_rect,
        dest_rect,
        ..
    } = canvas.commands()[1].kind
    else {
        panic!("expected second Image");
    };
    assert_eq!(source_rect, Rectangle::new(0.0, 0.0, 80.0, 40.0));
    assert_eq!(dest_rect, Rectangle::new(10.0, 20.0, 30.0, 40.0));
}

#[test]
fn image_region_records_source_pixels_destination_and_alpha() {
    let mut canvas = NdCanvas::new();
    canvas.global_alpha(0.5);

    assert_eq!(
        canvas.draw_image_region(
            image(),
            Rectangle::new(10.0, 5.0, 20.0, 10.0),
            Rectangle::new(100.0, 200.0, 60.0, 80.0),
        ),
        Ok(())
    );

    let RenderCommandKind::Image {
        image,
        source_rect,
        dest_rect,
        alpha,
    } = canvas.commands()[1].kind
    else {
        panic!("expected Image");
    };
    assert_eq!((image.width(), image.height()), (80, 40));
    assert_eq!(source_rect, Rectangle::new(10.0, 5.0, 20.0, 10.0));
    assert_eq!(dest_rect, Rectangle::new(100.0, 200.0, 60.0, 80.0));
    assert_eq!(alpha, 0.5);
}

#[test]
fn image_region_rejects_invalid_geometry_without_recording_work() {
    let invalid_cases = [
        (
            Rectangle::new(f64::NAN, 0.0, 10.0, 10.0),
            Rectangle::new(0.0, 0.0, 10.0, 10.0),
            ImageDrawError::NonFiniteSource,
        ),
        (
            Rectangle::new(0.0, 0.0, 10.0, 10.0),
            Rectangle::new(0.0, f64::INFINITY, 10.0, 10.0),
            ImageDrawError::NonFiniteDestination,
        ),
        (
            Rectangle::new(0.0, 0.0, 10.0, 10.0),
            Rectangle::new(f64::MAX, 0.0, f64::MAX, 10.0),
            ImageDrawError::NonFiniteDestination,
        ),
        (
            Rectangle::new(0.0, 0.0, -1.0, 10.0),
            Rectangle::new(0.0, 0.0, 10.0, 10.0),
            ImageDrawError::NegativeSourceExtent,
        ),
        (
            Rectangle::new(0.0, 0.0, 10.0, 10.0),
            Rectangle::new(0.0, 0.0, 10.0, -1.0),
            ImageDrawError::NegativeDestinationExtent,
        ),
        (
            Rectangle::new(70.0, 0.0, 20.0, 10.0),
            Rectangle::new(0.0, 0.0, 10.0, 10.0),
            ImageDrawError::SourceOutOfBounds,
        ),
    ];

    for (source_rect, dest_rect, expected) in invalid_cases {
        let mut canvas = NdCanvas::new();
        assert_eq!(
            canvas.draw_image_region(image(), source_rect, dest_rect),
            Err(expected)
        );
        assert!(canvas.commands().is_empty());
        assert!(canvas.damage().is_empty());
    }
}

#[test]
fn image_region_zero_extent_and_zero_alpha_are_noops() {
    let source = Rectangle::new(0.0, 0.0, 10.0, 10.0);
    let destination = Rectangle::new(20.0, 30.0, 40.0, 50.0);
    let mut canvas = NdCanvas::new();

    assert_eq!(
        canvas.draw_image_region(
            image(),
            Rectangle::new(source.x, source.y, 0.0, source.height),
            destination,
        ),
        Ok(())
    );
    assert_eq!(
        canvas.draw_image_region(
            image(),
            source,
            Rectangle::new(destination.x, destination.y, destination.width, 0.0),
        ),
        Ok(())
    );
    canvas.global_alpha(0.0);
    let command_count = canvas.commands().len();
    assert_eq!(
        canvas.draw_image_region(image(), source, destination),
        Ok(())
    );
    assert_eq!(canvas.commands().len(), command_count);
    assert_eq!(
        canvas.draw_image_region(image(), Rectangle::new(79.0, 0.0, 2.0, 1.0), destination,),
        Err(ImageDrawError::SourceOutOfBounds)
    );
    assert_eq!(canvas.commands().len(), command_count);
}
