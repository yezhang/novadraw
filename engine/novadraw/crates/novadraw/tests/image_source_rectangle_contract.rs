use novadraw::geometry::Rectangle;
use novadraw::graphics::GraphicsError;
use novadraw::render::{
    CommandRecorder, ImageDrawError, ImageRegion, ImageResourceRef, RenderCommandKind, ResourceId,
};
use uuid::Uuid;

fn image() -> ImageResourceRef {
    ImageResourceRef::new(ResourceId::new(Uuid::nil(), 1), 3, 80, 40, 2.0)
}

#[test]
fn full_image_commands_record_the_physical_source_bounds() {
    let mut recorder = CommandRecorder::new();
    {
        let mut context = recorder.paint();
        context
            .draw_image(image(), Rectangle::new(4.0, 5.0, 40.0, 20.0))
            .unwrap();
        context
            .draw_image(image(), Rectangle::new(10.0, 20.0, 30.0, 40.0))
            .unwrap();
    }

    let RenderCommandKind::Image {
        source_rect,
        dest_rect,
        ..
    } = recorder.commands()[0].kind
    else {
        panic!("expected Image");
    };
    assert_eq!(source_rect, Rectangle::new(0.0, 0.0, 80.0, 40.0));
    assert_eq!(dest_rect, Rectangle::new(4.0, 5.0, 40.0, 20.0));

    let RenderCommandKind::Image {
        source_rect,
        dest_rect,
        ..
    } = recorder.commands()[1].kind
    else {
        panic!("expected second Image");
    };
    assert_eq!(source_rect, Rectangle::new(0.0, 0.0, 80.0, 40.0));
    assert_eq!(dest_rect, Rectangle::new(10.0, 20.0, 30.0, 40.0));
}

#[test]
fn image_region_records_source_pixels_destination_and_alpha() {
    let mut recorder = CommandRecorder::new();
    {
        let mut context = recorder.paint();
        context
            .set_opacity(novadraw::graphics::Opacity::try_new(0.5).unwrap())
            .unwrap();
        assert_eq!(
            context.draw_image_region(
                ImageRegion::new(image(), Rectangle::new(10.0, 5.0, 20.0, 10.0))
                    .expect("valid source region"),
                Rectangle::new(100.0, 200.0, 60.0, 80.0),
            ),
            Ok(())
        );
    }

    let RenderCommandKind::Image {
        image,
        source_rect,
        dest_rect,
        alpha,
    } = recorder.commands()[1].kind
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
        let mut recorder = CommandRecorder::new();
        let result = match ImageRegion::new(image(), source_rect) {
            Ok(region) => recorder
                .paint()
                .draw_image_region(region, dest_rect)
                .map_err(|error| match error {
                    GraphicsError::Image(error) => error,
                    other => panic!("unexpected graphics error: {other}"),
                }),
            Err(error) => Err(error),
        };
        assert_eq!(result, Err(expected));
        assert!(recorder.commands().is_empty());
    }
}

#[test]
fn image_region_zero_extent_and_zero_alpha_are_noops() {
    let source = Rectangle::new(0.0, 0.0, 10.0, 10.0);
    let destination = Rectangle::new(20.0, 30.0, 40.0, 50.0);
    let mut recorder = CommandRecorder::new();
    let mut context = recorder.paint();
    assert_eq!(
        context.draw_image_region(
            ImageRegion::new(
                image(),
                Rectangle::new(source.x, source.y, 0.0, source.height),
            )
            .expect("zero-width source region is valid"),
            destination,
        ),
        Ok(())
    );
    assert_eq!(
        context.draw_image_region(
            ImageRegion::new(image(), source).expect("valid source region"),
            Rectangle::new(destination.x, destination.y, destination.width, 0.0),
        ),
        Ok(())
    );
    context
        .set_opacity(novadraw::graphics::Opacity::TRANSPARENT)
        .unwrap();
    let command_count = 1;
    assert_eq!(
        context.draw_image_region(
            ImageRegion::new(image(), source).expect("valid source region"),
            destination,
        ),
        Ok(())
    );
    drop(context);
    assert_eq!(recorder.commands().len(), command_count);
    assert_eq!(
        ImageRegion::new(image(), Rectangle::new(79.0, 0.0, 2.0, 1.0)),
        Err(ImageDrawError::SourceOutOfBounds)
    );
    assert_eq!(recorder.commands().len(), command_count);
}
