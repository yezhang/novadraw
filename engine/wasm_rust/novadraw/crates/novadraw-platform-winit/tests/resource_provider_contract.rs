use std::path::PathBuf;

use novadraw::render::ImageData;
use novadraw::runtime::ResourceStatus;
use novadraw::runtime::Runtime;
use novadraw_platform_winit::FileResourceProvider;

#[test]
fn file_provider_keeps_io_decode_and_runtime_completion_separate() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/scenes/assets/image-demo.svg");
    let provider = FileResourceProvider::new();
    let bytes = provider.load(&fixture).expect("fixture file");
    let image_data = ImageData::decode(&bytes, 1.0).expect("fixture decode");

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
        .complete_image_request(request, image_data)
        .expect("resource completion");

    assert_eq!(
        runtime.resources().status(image.resource_id()),
        Ok(&ResourceStatus::Ready { revision: 1 })
    );
}

#[test]
fn file_provider_preserves_the_original_io_error() {
    let provider = FileResourceProvider::new();
    let missing = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("missing-resource.bin");

    let error = provider.load(missing).expect_err("missing file must fail");

    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}
