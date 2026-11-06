//! Public-API consumers: compare checked projection with a fresh live computation.
use novadraw::{
    connection::{
        ChopboxAnchor, ConnectionFigure, ConnectionId, CoordinateSpace, RouterBinding, XYAnchor,
    },
    container::FreeformLayerFigure,
    figure::{
        CONTAINER, ChildClippingStrategy, ChildTransform, ContainerCapability, Figure,
        FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureContainer,
        FigureProjection, ImageFigure, LabelFigure, PROJECTION, ProjectionCapability,
        ProjectionContribution, ProjectionError, ProjectionInputKey, ProjectionInputPayload,
        ProjectionInputRegistry, ProjectionScene, RectangleFigure,
    },
    geometry::{Point, Rectangle},
    graphics::{Color, GraphicsError, PaintContext},
    render::{
        BuiltinFont, ImageData, RenderBackend, RenderCommandKind, RenderDisposition,
        RenderSubmission, SurfaceInfo, TextEngine,
    },
    runtime::{FramePreparation, FramePreparationError, Runtime, SubmissionAcknowledgement},
    tree::{FigureId, FigureTree},
};
use novadraw_backend_vello::texture::VelloTextureRenderer;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering},
};
use vello::wgpu;

const WIDTH: u32 = 256;
const HEIGHT: u32 = 128;
const INPUT_BYTES: usize = 64 * 1024;

#[derive(PartialEq, Eq)]
struct Offset(i64);
impl ProjectionInputPayload for Offset {
    fn retained_units(&self) -> u64 {
        0
    }
}
const OFFSET: ProjectionInputKey<Offset> = ProjectionInputKey::new("pixels.offset");

struct Group {
    model: Arc<AtomicI64>,
    inputs: ProjectionInputRegistry,
    checked: bool,
}
impl Figure for Group {
    fn name(&self) -> &'static str {
        "MixedPixelGroup"
    }
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 320.0, 160.0)
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())?;
        if self.checked {
            out.register(PROJECTION, ProjectionCapability::of::<Self>())?;
        }
        Ok(())
    }
}
impl FigureContainer for Group {
    fn child_transform(&self) -> ChildTransform {
        assert!(
            !self.checked,
            "checked source must never call the live fallback"
        );
        ChildTransform::translation(self.model.load(Ordering::Relaxed) as f64, 0.0)
    }
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::OverflowVisible
    }
}
impl FigureProjection for Group {
    fn project(
        &self,
        scene: &ProjectionScene<'_>,
    ) -> Result<ProjectionContribution, ProjectionError> {
        let snapshot = scene.input(&self.inputs, OFFSET, scene.root())?;
        Ok(ProjectionContribution::new(ChildTransform::translation(
            snapshot.value(OFFSET).unwrap().0 as f64,
            0.0,
        )))
    }
}

/// No declaration grants this callback reuse or gives Core a known ink envelope.
struct UnknownPaint {
    calls: Arc<AtomicUsize>,
    change_during_paint: Arc<AtomicBool>,
    model: Arc<AtomicI64>,
}
impl Figure for UnknownPaint {
    fn name(&self) -> &'static str {
        "UnknownOverflowPainter"
    }
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(12.0, 76.0, 12.0, 12.0)
    }
    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        // State read plus ink beyond declared bounds: neither may be skipped.
        assert!(context.effective_clip_bounds().is_some());
        context.set_fill_paint(Color::BLUE);
        context.fill_rect(Rectangle::new(
            bounds.x - 4.0,
            bounds.y - 4.0,
            bounds.width + 8.0,
            bounds.height + 8.0,
        ))?;
        if self.change_during_paint.swap(false, Ordering::Relaxed) {
            self.model.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }
}

struct Scene {
    runtime: Runtime,
    root: FigureId,
    group: FigureId,
    shape: FigureId,
    outer: FigureId,
    inner: FigureId,
    model: Arc<AtomicI64>,
    paint_calls: Arc<AtomicUsize>,
    change_during_paint: Arc<AtomicBool>,
}
fn scene(checked: bool, offset: i64) -> Scene {
    let model = Arc::new(AtomicI64::new(offset));
    let mut inputs = ProjectionInputRegistry::new(INPUT_BYTES);
    let reader_model = model.clone();
    inputs
        .register(OFFSET, move |_, _: &ProjectionScene<'_>| {
            Ok(Offset(reader_model.load(Ordering::Relaxed)))
        })
        .unwrap();
    let mut runtime =
        Runtime::with_text_layout_engine(FigureTree::new(), Box::new(TextEngine::new()));
    runtime
        .resources()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new_with_color(
                0.0,
                0.0,
                f64::from(WIDTH),
                f64::from(HEIGHT),
                Color::TRANSPARENT,
            )
            .unwrap(),
        ))
        .unwrap();
    let outer = runtime
        .viewports()
        .add(root, Rectangle::new(8.0, 8.0, 208.0, 108.0))
        .unwrap()
        .figure_id();
    let content = runtime
        .viewports()
        .set_contents(
            outer,
            Box::new(
                RectangleFigure::new_with_color(0.0, 0.0, 320.0, 160.0, Color::TRANSPARENT)
                    .unwrap(),
            ),
        )
        .unwrap();
    let inner = runtime
        .viewports()
        .add(content, Rectangle::new(8.0, 4.0, 192.0, 100.0))
        .unwrap()
        .figure_id();
    let group = runtime
        .viewports()
        .set_contents(
            inner,
            Box::new(Group {
                model: model.clone(),
                inputs,
                checked,
            }),
        )
        .unwrap();
    let shape = runtime
        .figure(group)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new_with_color(12.0, 8.0, 20.0, 20.0, Color::RED).unwrap(),
        ))
        .unwrap();
    let image = runtime.resources().register_image().unwrap();
    runtime
        .resources()
        .complete_image(
            image,
            ImageData::from_rgba(8, 8, [0, 255, 0, 255].repeat(64), 1.0),
        )
        .unwrap();
    runtime
        .figure(group)
        .unwrap()
        .add(Box::new(
            ImageFigure::new(image)
                .with_bounds(Rectangle::new(48.0, 8.0, 16.0, 16.0))
                .unwrap(),
        ))
        .unwrap();
    runtime
        .figure(group)
        .unwrap()
        .add(Box::new(
            LabelFigure::new("Projection")
                .with_bounds(Rectangle::new(12.0, 38.0, 130.0, 28.0))
                .unwrap(),
        ))
        .unwrap();
    let paint_calls = Arc::new(AtomicUsize::new(0));
    let change_during_paint = Arc::new(AtomicBool::new(false));
    runtime
        .figure(group)
        .unwrap()
        .add(Box::new(UnknownPaint {
            calls: paint_calls.clone(),
            change_during_paint: change_during_paint.clone(),
            model: model.clone(),
        }))
        .unwrap();
    Scene {
        runtime,
        root,
        group,
        shape,
        outer,
        inner,
        model,
        paint_calls,
        change_during_paint,
    }
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: VelloTextureRenderer,
    target: wgpu::Texture,
}
impl Gpu {
    fn new() -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        eprintln!("projection pixel adapter: {:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let renderer = VelloTextureRenderer::new(&device, &queue).unwrap();
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mixed projection pixels"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        Self {
            device,
            queue,
            renderer,
            target,
        }
    }
    fn prepare(&mut self, scene: &mut Scene) -> FramePreparation {
        let backend = self.renderer.bind(&self.target);
        scene.runtime.frames().prepare_submission(
            SurfaceInfo {
                logical_width: f64::from(WIDTH),
                logical_height: f64::from(HEIGHT),
                pixel_width: WIDTH,
                pixel_height: HEIGHT,
                scale_factor: 1.0,
            },
            backend.capabilities(),
        )
    }
    fn present(&mut self, scene: &mut Scene, frame: RenderSubmission) -> Vec<u8> {
        let result = self.renderer.bind(&self.target).submit(&frame);
        assert_eq!(result, Ok(RenderDisposition::Presented));
        assert_eq!(
            scene
                .runtime
                .frames()
                .complete_submission(frame.session_id, frame.frame_id, result)
                .unwrap(),
            SubmissionAcknowledgement::Presented,
        );
        self.pixels()
    }
    fn pixels(&self) -> Vec<u8> {
        let row = WIDTH * 4;
        assert_eq!(row % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT, 0);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mixed projection readback"),
            size: u64::from(row * HEIGHT),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            self.target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(HEIGHT),
                },
            },
            self.target.size(),
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            tx.send(result).unwrap()
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .unwrap();
        rx.recv().unwrap().unwrap();
        buffer.get_mapped_range(..).expect("mapped pixels").to_vec()
    }
}

fn render(gpu: &mut Gpu, scene: &mut Scene) -> Vec<u8> {
    scene.runtime.frames().request_full_redraw();
    let frame = gpu.prepare(scene).into_ready().unwrap();
    assert!(
        frame.commands.iter().any(|command| matches!(
            &command.kind, RenderCommandKind::DrawGlyphRun { run, .. } if !run.glyphs.is_empty()
        )),
        "mixed fixture must actually draw text"
    );
    gpu.present(scene, frame)
}
fn assert_shape_hit(scene: &Scene) {
    let surface = scene
        .runtime
        .tree()
        .local_to_surface_transform(scene.shape)
        .unwrap()
        .transform_point(Point::new(2.0, 2.0));
    assert_eq!(
        scene
            .runtime
            .tree()
            .hit_test_simple((surface.x(), surface.y()))
            .unwrap(),
        Some(scene.shape)
    );
    let inverse = scene
        .runtime
        .tree()
        .surface_to_local_transform(scene.shape)
        .unwrap()
        .unwrap();
    assert_eq!(inverse.transform_point(surface), Point::new(2.0, 2.0));
}

fn fresh_live_pixels(offset: i64) -> Vec<u8> {
    // Each Runtime owns a distinct backend session; reference submission uses
    // its own backend rather than reusing the checked scene's session.
    render(&mut Gpu::new(), &mut scene(false, offset))
}

fn save_pixels(name: &str, pixels: &[u8]) {
    if let Some(directory) = std::env::var_os("NOVADRAW_PROJECTION_PIXEL_ARTIFACT_DIR") {
        let directory = std::path::Path::new(&directory);
        assert!(
            directory.is_dir(),
            "artifact directory must exist before testing"
        );
        std::fs::write(directory.join(format!("{name}.rgba")), pixels).unwrap();
    }
}

fn add_freeform_connection(scene: &mut Scene) -> (FigureId, FigureId, ConnectionId) {
    let layer = scene
        .runtime
        .figure(scene.group)
        .unwrap()
        .add(Box::new(
            FreeformLayerFigure::new(90.0, 70.0, 100.0, 30.0).unwrap(),
        ))
        .unwrap();
    let source = scene
        .runtime
        .figure(layer)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new_with_color(5.0, 5.0, 10.0, 10.0, Color::RED).unwrap(),
        ))
        .unwrap();
    let connection = scene
        .runtime
        .figure(scene.group)
        .unwrap()
        .add(Box::new(
            ConnectionFigure::new()
                .with_stroke(Color::BLUE, 2.0)
                .unwrap(),
        ))
        .unwrap();
    let target = scene
        .runtime
        .figure(scene.group)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new_with_color(170.0, 70.0, 10.0, 10.0, Color::RED).unwrap(),
        ))
        .unwrap();
    let source_anchor = scene
        .runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(source)))
        .unwrap();
    let target_anchor = scene
        .runtime
        .connections()
        .register_anchor(Box::new(XYAnchor::new(
            Point::new(5.0, 5.0),
            CoordinateSpace::FigureLocal(target),
        )))
        .unwrap();
    let router = scene.runtime.connections().direct_router();
    let id = scene
        .runtime
        .connections()
        .register_state(
            connection,
            Some(source_anchor),
            Some(target_anchor),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();
    (layer, source, id)
}

#[test]
fn route_across_viewport_chain_preserves_explicit_topology_rejection() {
    use novadraw::connection::{ConnectionRuntimeError, RouteError, UnresolvedConnection};
    let mut checked = scene(true, 0);
    let connection_figure = checked
        .runtime
        .figure(checked.root)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .unwrap();
    let source = checked
        .runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(checked.shape)))
        .unwrap();
    let target = checked
        .runtime
        .connections()
        .register_anchor(Box::new(XYAnchor::new(
            Point::new(240.0, 90.0),
            CoordinateSpace::ChildContent(checked.root),
        )))
        .unwrap();
    let router = checked.runtime.connections().direct_router();
    let connection = checked
        .runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source),
            Some(target),
            RouterBinding::Explicit { router },
            None,
        )
        .unwrap();
    checked.runtime.frames().stabilize_for_query().unwrap();
    assert!(matches!(
        checked
            .runtime
            .connections()
            .resolve_route(connection, CoordinateSpace::ChildContent(checked.root),),
        Err(ConnectionRuntimeError::Unresolved(
            UnresolvedConnection::RouteFailed(RouteError::UnsupportedViewportTopology)
        ))
    ));
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn freeform_extreme_move_and_cross_subtree_route_match_fresh_computation() {
    let mut gpu = Gpu::new();
    let mut checked = scene(true, 0);
    let (layer, source, connection) = add_freeform_connection(&mut checked);
    let a = render(&mut gpu, &mut checked);
    let before = checked
        .runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(checked.group))
        .unwrap()
        .points()
        .clone();
    assert_eq!(
        checked.runtime.viewports().freeform_extent(layer).unwrap(),
        Rectangle::new(5.0, 5.0, 10.0, 10.0)
    );
    checked.model.store(24, Ordering::Relaxed);
    checked
        .runtime
        .figure(source)
        .unwrap()
        .set_bounds(Rectangle::new(40.0, 5.0, 10.0, 10.0))
        .unwrap();
    let b = render(&mut gpu, &mut checked);
    let after = checked
        .runtime
        .connections()
        .resolve_route(connection, CoordinateSpace::ChildContent(checked.group))
        .unwrap()
        .points()
        .clone();
    assert_ne!(before.get(0), after.get(0));
    assert_eq!(
        before.get(1),
        after.get(1),
        "other subtree endpoint stays fixed"
    );
    assert_eq!(
        checked.runtime.viewports().freeform_extent(layer).unwrap(),
        Rectangle::new(40.0, 5.0, 10.0, 10.0)
    );
    let mut reference = scene(false, 24);
    let (reference_layer, reference_source, reference_connection) =
        add_freeform_connection(&mut reference);
    reference
        .runtime
        .figure(reference_source)
        .unwrap()
        .set_bounds(Rectangle::new(40.0, 5.0, 10.0, 10.0))
        .unwrap();
    let mut reference_gpu = Gpu::new();
    assert_eq!(b, render(&mut reference_gpu, &mut reference));
    assert_eq!(
        after,
        reference
            .runtime
            .connections()
            .resolve_route(
                reference_connection,
                CoordinateSpace::ChildContent(reference.group),
            )
            .unwrap()
            .points()
            .clone()
    );
    assert_eq!(
        reference
            .runtime
            .viewports()
            .freeform_extent(reference_layer)
            .unwrap(),
        checked.runtime.viewports().freeform_extent(layer).unwrap()
    );
    assert_ne!(a, b);
    save_pixels("freeform-b", &b);
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn checked_mixed_tree_matches_fresh_live_pixels_after_move_and_nested_pan() {
    let mut gpu = Gpu::new();
    let mut checked = scene(true, 0);
    let a = render(&mut gpu, &mut checked);
    save_pixels("shared-a", &a);
    assert_eq!(a, fresh_live_pixels(0));
    let (pixels, remainder) = a.as_chunks::<4>();
    assert!(remainder.is_empty());
    assert!(pixels.contains(&[255, 0, 0, 255]));
    assert!(pixels.contains(&[0, 255, 0, 255]));
    assert!(pixels.contains(&[0, 0, 255, 255]));
    let revision = checked
        .runtime
        .tree()
        .component_revision(checked.group)
        .unwrap();
    let calls = checked.paint_calls.load(Ordering::Relaxed);
    checked.model.store(40, Ordering::Relaxed);
    // Query B does not paint or publish it.
    assert_shape_hit(&checked);
    assert_eq!(checked.paint_calls.load(Ordering::Relaxed), calls);
    let b = render(&mut gpu, &mut checked);
    save_pixels("shared-b", &b);
    assert_ne!(a, b);
    assert_eq!(b, fresh_live_pixels(40));
    assert_eq!(
        checked
            .runtime
            .tree()
            .component_revision(checked.group)
            .unwrap(),
        revision
    );
    assert!(checked.paint_calls.load(Ordering::Relaxed) > calls);
    assert_shape_hit(&checked);
    checked
        .runtime
        .viewports()
        .set_view_location(checked.outer, 12.0, 4.0)
        .unwrap();
    checked
        .runtime
        .viewports()
        .set_view_location(checked.inner, 8.0, 2.0)
        .unwrap();
    let panned = render(&mut gpu, &mut checked);
    save_pixels("nested-pan", &panned);
    let mut reference = scene(false, 40);
    let mut reference_gpu = Gpu::new();
    // Establish ranges before applying pan to the fresh complete path.
    render(&mut reference_gpu, &mut reference);
    reference
        .runtime
        .viewports()
        .set_view_location(reference.outer, 12.0, 4.0)
        .unwrap();
    reference
        .runtime
        .viewports()
        .set_view_location(reference.inner, 8.0, 2.0)
        .unwrap();
    assert_eq!(panned, render(&mut reference_gpu, &mut reference));
    assert_shape_hit(&checked);
    assert_ne!(b, panned);
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn failed_and_skipped_b_preserve_a_then_c_erases_old_overflow_pixels() {
    let mut gpu = Gpu::new();
    let mut checked = scene(true, 0);
    let a = render(&mut gpu, &mut checked);
    checked.model.store(40, Ordering::Relaxed);
    checked.change_during_paint.store(true, Ordering::Relaxed);
    checked.runtime.frames().request_full_redraw();
    assert!(matches!(
        gpu.prepare(&mut checked),
        FramePreparation::Error(FramePreparationError::Coordinate(_))
    ));
    assert_eq!(
        gpu.pixels(),
        a,
        "failed recording cannot touch the displayed target"
    );
    checked.model.store(40, Ordering::Relaxed);
    let b = gpu.prepare(&mut checked).into_ready().unwrap();
    assert!(
        b.damage.is_full(),
        "unknown old overflow requires full repair"
    );
    let acknowledgement = checked
        .runtime
        .frames()
        .complete_submission(b.session_id, b.frame_id, Ok(RenderDisposition::Skipped))
        .unwrap();
    assert_ne!(acknowledgement, SubmissionAcknowledgement::Presented);
    assert_eq!(gpu.pixels(), a);
    checked.model.store(80, Ordering::Relaxed);
    let c = gpu.prepare(&mut checked).into_ready().unwrap();
    assert!(c.damage.is_full());
    let actual = gpu.present(&mut checked, c);
    save_pixels("recovered-c", &actual);
    assert_ne!(a, actual);
    assert_eq!(actual, fresh_live_pixels(80));
    assert_shape_hit(&checked);
    // Entire old shape and overflow painter areas have been erased, not only bounds.
    for (x, y) in [(30, 24), (26, 86)] {
        let index = ((y * WIDTH + x) * 4) as usize;
        assert_eq!(&actual[index..index + 4], &[0; 4]);
    }
    assert_eq!(render(&mut gpu, &mut checked), actual);
}
