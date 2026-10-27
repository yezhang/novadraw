use novadraw::render::{ImageData, ImageDecodeError};
use novadraw::runtime::Runtime;
use novadraw::runtime::{ImageCompletionError, ResourceError, ResourceStatus};

const SVG: &[u8] = include_bytes!("../../../examples/scenes/assets/image-demo.svg");

#[test]
fn replacement_request_rejects_stale_completion_without_mutating_resource() {
    let mut runtime = Runtime::empty();
    let image = runtime
        .resources()
        .register_image()
        .expect("image registration");
    let stale = runtime
        .resources()
        .begin_request(image.resource_id())
        .expect("first request");
    let current = runtime
        .resources()
        .begin_request(image.resource_id())
        .expect("replacement request");

    assert_eq!(
        runtime
            .resources()
            .complete_image_request_bytes(stale, SVG, 1.0),
        Err(ImageCompletionError::Resource(ResourceError::StaleRequest))
    );
    assert_eq!(
        runtime.resources().status(image.resource_id()),
        Ok(&ResourceStatus::Pending)
    );

    runtime
        .resources()
        .complete_image_request_bytes(current, SVG, 1.0)
        .expect("current request completion");
    assert_eq!(
        runtime.resources().status(image.resource_id()),
        Ok(&ResourceStatus::Ready { revision: 1 })
    );
}

#[test]
fn decode_and_runtime_failures_remain_distinct() {
    let mut runtime = Runtime::empty();
    let image = runtime
        .resources()
        .register_image()
        .expect("image registration");
    let request = runtime
        .resources()
        .begin_request(image.resource_id())
        .expect("image request");

    assert_eq!(
        runtime
            .resources()
            .complete_image_request_bytes(request, b"not an image", 1.0),
        Err(ImageCompletionError::Decode(
            ImageDecodeError::UnsupportedFormat
        ))
    );
    assert!(matches!(
        runtime.resources().status(image.resource_id()),
        Ok(ResourceStatus::Failed { reason }) if reason.contains("not supported")
    ));

    let mut foreign = Runtime::empty();
    let foreign_image = foreign
        .resources()
        .register_image()
        .expect("foreign image registration");
    let foreign_request = foreign
        .resources()
        .begin_request(foreign_image.resource_id())
        .expect("foreign request");
    assert_eq!(
        runtime
            .resources()
            .complete_image_request_bytes(foreign_request, SVG, 1.0),
        Err(ImageCompletionError::Resource(
            ResourceError::WrongNamespace
        ))
    );
}

#[test]
fn direct_completion_invalidates_an_outstanding_provider_request() {
    let mut runtime = Runtime::empty();
    let image = runtime
        .resources()
        .register_image()
        .expect("image registration");
    let request = runtime
        .resources()
        .begin_request(image.resource_id())
        .expect("image request");

    runtime
        .resources()
        .complete_image(image, ImageData::from_rgba(1, 1, vec![255; 4], 1.0))
        .expect("direct completion");
    assert_eq!(
        runtime
            .resources()
            .complete_image_request(request, ImageData::from_rgba(1, 1, vec![0; 4], 1.0)),
        Err(ResourceError::StaleRequest)
    );
    assert_eq!(
        runtime.resources().status(image.resource_id()),
        Ok(&ResourceStatus::Ready { revision: 1 })
    );
}

#[test]
fn provider_failure_consumes_only_the_matching_request() {
    let mut runtime = Runtime::empty();
    let image = runtime
        .resources()
        .register_image()
        .expect("image registration");
    let request = runtime
        .resources()
        .begin_request(image.resource_id())
        .expect("image request");

    runtime
        .resources()
        .fail_request(request, "file permission denied")
        .expect("matching failure");
    assert!(matches!(
        runtime.resources().status(image.resource_id()),
        Ok(ResourceStatus::Failed { reason }) if reason == "file permission denied"
    ));
    assert_eq!(
        runtime.resources().fail_request(request, "duplicate"),
        Err(ResourceError::StaleRequest)
    );
}
