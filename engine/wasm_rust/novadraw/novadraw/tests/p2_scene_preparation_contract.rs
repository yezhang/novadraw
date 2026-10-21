//! Independent consumer of frozen preparation and atomic layout publication.
use novadraw::{
    figure::{
        Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureDrawing,
        FigureMeasurement, FigurePreparation, FigurePresentation, FigureStyle, PREPARATION,
        PreparationCapability, PreparationScene, PreparedScene,
    },
    geometry::{Dimension, Rectangle},
    graphics::{Color, GraphicsError, PaintContext},
    layout::LayoutOutput,
    render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
    runtime::Runtime,
    text::{MeasureContext, TextError},
    tree::FigureTree,
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

struct Empty;
impl FigureDrawing for Empty {
    fn paint(&self, _: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        Ok(())
    }
}

fn presentation(width: f64, height: f64) -> FigurePresentation {
    FigurePresentation::new(
        FigureMeasurement::new(width, height, None),
        Dimension::new(0.0, 0.0),
        Rectangle::new(0.0, 0.0, width, height),
        Arc::new(Empty),
    )
    .unwrap()
}

struct ExternalPage {
    mode: Arc<AtomicU64>,
}
impl Figure for ExternalPage {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 120.0, 80.0)
    }
    fn name(&self) -> &'static str {
        "ExternalPage"
    }
    fn register_capabilities(
        &self,
        output: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        output.register(PREPARATION, PreparationCapability::of::<Self>())
    }
}
impl FigurePreparation for ExternalPage {
    fn revision(&self) -> u64 {
        self.mode.load(Ordering::Relaxed)
    }
    fn prepare(
        &self,
        _: &mut MeasureContext<'_>,
        _: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        Ok(presentation(120.0, 80.0))
    }
    fn prepare_scene(
        &self,
        _: &mut MeasureContext<'_>,
        _: Rectangle,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparedScene, TextError> {
        let mode = self.revision();
        let children = scene.children(scene.root())?;
        let color = scene.style(children[0])?.foreground;
        let root = presentation(120.0, 80.0).with_data(Arc::new((mode, color)));
        let mut candidate = PreparedScene::new(root);
        let mut output = LayoutOutput::new();
        output.set_child_bounds(
            children[0],
            Rectangle::new(if mode == 0 { 0.0 } else { 10.0 }, 0.0, 60.0, 30.0),
        );
        output.set_child_bounds(
            children[1],
            Rectangle::new(80.0, 0.0, if mode == 1 { f64::NAN } else { 20.0 }, 30.0),
        );
        if mode == 4 {
            output.set_child_bounds(scene.root(), Rectangle::new(0.0, 0.0, 1.0, 1.0));
        }
        candidate.layout(scene.root(), output);
        candidate.present(
            children[0],
            presentation(60.0, 30.0).with_hit_regions(vec![
                Rectangle::new(0.0, 0.0, 20.0, 10.0),
                Rectangle::new(40.0, 20.0, 20.0, 10.0),
            ])?,
        );
        if mode == 2 {
            return Err(TextError::InvalidPreparedScene(
                "external measurement failed".into(),
            ));
        }
        if mode == 3 {
            self.mode.store(4, Ordering::Relaxed);
        }
        Ok(candidate)
    }
}

fn fixture() -> (
    Runtime,
    novadraw::tree::FigureId,
    [novadraw::tree::FigureId; 2],
    Arc<AtomicU64>,
) {
    let mode = Arc::new(AtomicU64::new(0));
    let mut tree = FigureTree::new();
    let page = tree
        .builder()
        .set_contents(Box::new(ExternalPage { mode: mode.clone() }))
        .unwrap();
    let a = tree
        .builder()
        .add_child_with_bounds(page, Rectangle::new(1.0, 2.0, 10.0, 10.0), Color::RED)
        .unwrap();
    let b = tree
        .builder()
        .add_child_with_bounds(page, Rectangle::new(3.0, 4.0, 10.0, 10.0), Color::RED)
        .unwrap();
    (Runtime::new(tree), page, [a, b], mode)
}

fn frame(runtime: &mut Runtime) -> bool {
    runtime.frames().request_full_redraw();
    let result = runtime.frames().prepare_submission(
        SurfaceInfo {
            logical_width: 120.0,
            logical_height: 80.0,
            pixel_width: 120,
            pixel_height: 80,
            scale_factor: 1.0,
        },
        BackendCapabilities::RETAINED_PARTIAL,
    );
    if let Ok(frame) = result.into_ready() {
        let _ = runtime
            .frames()
            .complete_submission(
                frame.session_id,
                frame.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .unwrap();
        true
    } else {
        false
    }
}

#[test]
fn failure_keeps_all_old_bounds_and_snapshot_data() {
    for failure in [1, 2, 3, 4] {
        let (mut runtime, page, children, mode) = fixture();
        assert!(frame(&mut runtime));
        let before = runtime.tree().node(children[0]).unwrap().bounds();
        let before_b = runtime.tree().node(children[1]).unwrap().bounds();
        mode.store(failure, Ordering::Relaxed);
        assert!(!frame(&mut runtime));
        assert_eq!(runtime.tree().node(children[0]).unwrap().bounds(), before);
        assert_eq!(runtime.tree().node(children[1]).unwrap().bounds(), before_b);
        assert_eq!(
            runtime
                .tree()
                .prepared_presentation(page)
                .unwrap()
                .unwrap()
                .data::<(u64, Color)>()
                .unwrap()
                .0,
            0
        );
    }
}

#[test]
fn fragment_blank_does_not_hit_but_normal_descendants_remain_reachable() {
    let (mut runtime, page, children, _) = fixture();
    assert!(frame(&mut runtime));
    assert_eq!(
        runtime.tree().hit_test_simple((5.0, 5.0)),
        Some(children[0])
    );
    assert_eq!(runtime.tree().hit_test_simple((30.0, 15.0)), Some(page));
    assert_eq!(
        runtime.tree().hit_test_simple((50.0, 25.0)),
        Some(children[0])
    );
    let grandchild = runtime
        .figure(children[0])
        .unwrap()
        .add(Box::new(novadraw::figure::RectangleFigure::new_with_color(
            25.0,
            12.0,
            10.0,
            8.0,
            Color::RED,
        )))
        .unwrap();
    assert!(frame(&mut runtime));
    assert_eq!(
        runtime.tree().hit_test_simple((30.0, 15.0)),
        Some(grandchild)
    );
}

#[test]
fn inherited_paint_only_style_invalidates_prepared_subtree() {
    let (mut runtime, page, _, _) = fixture();
    assert!(frame(&mut runtime));
    runtime
        .figure(page)
        .unwrap()
        .set_style(FigureStyle {
            foreground: Some(Color::RED),
            ..FigureStyle::default()
        })
        .unwrap();
    assert!(frame(&mut runtime));
    assert_eq!(
        runtime
            .tree()
            .prepared_presentation(page)
            .unwrap()
            .unwrap()
            .data::<(u64, Color)>()
            .unwrap()
            .1,
        Color::RED
    );
}
