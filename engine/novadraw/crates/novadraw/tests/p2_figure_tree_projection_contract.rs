//! Mixed-tree checks for the existing same-size placement contract.
mod support;

use novadraw::{
    event::{AncestorEvent, AncestorEventKind, AncestorListener, ListenerDirective},
    figure::{
        Figure, FigureMeasurement, ImageFigure, LabelFigure, MeasureConstraints, RectangleFigure,
    },
    geometry::{Dimension, Point, Rectangle, Translatable},
    graphics::{Color, GraphicsError, PaintContext},
    layout::{LayoutError, LayoutManager, LayoutOutput, LayoutSceneQuery},
    render::{
        BuiltinFont, FontDescriptor, GlyphRun, ImageData, RenderCommandKind, RenderSubmission,
        ResourceId, TextConstraints, TextEngine, TextError, TextLayout, TextLayoutEngine,
    },
    runtime::Runtime,
    tree::{FigureId, FigureTree},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

struct ExternalPaint(Arc<AtomicUsize>);
impl Figure for ExternalPaint {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 20.0, 20.0)
    }
    fn name(&self) -> &'static str {
        "ExternalProjectionPaint"
    }
    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        context.set_fill_paint(Color::BLUE);
        context.fill_rect(bounds)
    }
}

struct CountingText {
    engine: TextEngine,
    calls: Arc<AtomicUsize>,
}
impl TextLayoutEngine for CountingText {
    fn revision(&self) -> u64 {
        self.engine.revision()
    }
    fn register_font(
        &mut self,
        id: ResourceId,
        revision: u64,
        bytes: &[u8],
    ) -> Result<(), TextError> {
        self.engine.register_font(id, revision, bytes)
    }
    fn remove_font(&mut self, id: ResourceId) {
        self.engine.remove_font(id);
    }
    fn layout(
        &mut self,
        text: &str,
        font: &FontDescriptor,
        constraints: TextConstraints,
    ) -> Result<TextLayout, TextError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.engine.layout(text, font, constraints)
    }
}

struct ExternalLayout(Arc<AtomicUsize>);
impl LayoutManager for ExternalLayout {
    fn preferred_measurement(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        Ok(FigureMeasurement::new(300.0, 200.0, None))
    }
    fn minimum_size(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        Ok(Dimension::ZERO)
    }
    fn layout(
        &mut self,
        owner: FigureId,
        query: &LayoutSceneQuery<'_>,
        output: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        let placements = [
            Rectangle::new(10.0, 10.0, 20.0, 20.0),
            Rectangle::new(50.0, 10.0, 20.0, 20.0),
            Rectangle::new(90.0, 10.0, 140.0, 30.0),
            Rectangle::new(10.0, 60.0, 20.0, 20.0),
        ];
        for ((child, _), bounds) in query.children(owner)?.into_iter().zip(placements) {
            output.set_child_bounds(child, bounds);
        }
        Ok(())
    }
}

struct Ancestors(Arc<Mutex<Vec<AncestorEvent>>>);
impl AncestorListener for Ancestors {
    fn ancestor_changed(&self, event: AncestorEvent) -> ListenerDirective {
        self.0.lock().unwrap().push(event);
        ListenerDirective::Keep
    }
}

struct Fixture {
    runtime: Runtime,
    root: FigureId,
    group: FigureId,
    children: Vec<FigureId>,
    layouts: Arc<AtomicUsize>,
    paints: Arc<AtomicUsize>,
    text_calls: Arc<AtomicUsize>,
    ancestors: Arc<Mutex<Vec<AncestorEvent>>>,
}

fn fixture() -> Fixture {
    let text_calls = Arc::new(AtomicUsize::new(0));
    let mut runtime = Runtime::with_text_layout_engine(
        FigureTree::new(),
        Box::new(CountingText {
            engine: TextEngine::new(),
            calls: text_calls.clone(),
        }),
    );
    runtime
        .resources()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 600.0, 500.0).unwrap(),
        ))
        .unwrap();
    let group = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(40.0, 30.0, 300.0, 200.0).unwrap(),
        ))
        .unwrap();
    let image = runtime.resources().register_image().unwrap();
    runtime
        .resources()
        .complete_image(image, ImageData::from_rgba(2, 2, vec![255; 16], 1.0))
        .unwrap();
    let paints = Arc::new(AtomicUsize::new(0));
    let figures: Vec<Box<dyn Figure>> = vec![
        Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0).unwrap()),
        Box::new(ImageFigure::new(image)),
        Box::new(LabelFigure::new("Mixed projection")),
        Box::new(ExternalPaint(paints.clone())),
    ];
    let mut children = Vec::new();
    for figure in figures {
        children.push(runtime.figure(group).unwrap().add(figure).unwrap());
    }
    let layouts = Arc::new(AtomicUsize::new(0));
    runtime
        .figure(group)
        .unwrap()
        .set_layout_manager(Box::new(ExternalLayout(layouts.clone())))
        .unwrap();
    support::record_full_submission(&mut runtime);
    layouts.store(0, Ordering::Relaxed);
    paints.store(0, Ordering::Relaxed);
    let ancestors = Arc::new(Mutex::new(Vec::new()));
    runtime
        .observations()
        .add_ancestor_listener(Box::new(Ancestors(ancestors.clone())))
        .unwrap();
    Fixture {
        runtime,
        root,
        group,
        children,
        layouts,
        paints,
        text_calls,
        ancestors,
    }
}

fn glyphs(submission: &RenderSubmission) -> Vec<(GlyphRun, Point)> {
    submission
        .commands
        .iter()
        .filter_map(|command| {
            if let RenderCommandKind::DrawGlyphRun { run, origin, .. } = &command.kind {
                Some((run.clone(), *origin))
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn same_size_group_move_preserves_local_assets_and_updates_hits_and_notifications() {
    let mut f = fixture();
    let before = support::record_full_submission(&mut f.runtime);
    let before_glyphs = glyphs(&before);
    assert!(!before_glyphs.is_empty());
    let before_images = before
        .commands
        .iter()
        .filter_map(|command| {
            if let RenderCommandKind::Image {
                image,
                source_rect,
                dest_rect,
                ..
            } = command.kind
            {
                Some((image, source_rect, dest_rect))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(before_images.len(), 1);
    let bounds = f
        .children
        .iter()
        .map(|id| f.runtime.tree().figure_bounds(*id).unwrap())
        .collect::<Vec<_>>();
    let before_paints = f.paints.load(Ordering::Relaxed);
    let before_text = f.text_calls.load(Ordering::Relaxed);
    assert!(before_text > 0);
    f.runtime
        .figure(f.group)
        .unwrap()
        .set_bounds(Rectangle::new(70.0, 55.0, 300.0, 200.0))
        .unwrap();
    let after = support::record_full_submission(&mut f.runtime);
    assert_eq!(f.layouts.load(Ordering::Relaxed), 0);
    assert_eq!(f.text_calls.load(Ordering::Relaxed), before_text);
    // No validity declaration authorizes skipping an arbitrary paint callback.
    assert!(f.paints.load(Ordering::Relaxed) > before_paints);
    for (id, old) in f.children.iter().zip(bounds) {
        assert_eq!(f.runtime.tree().figure_bounds(*id).unwrap(), old);
        let mut surface = Point::new(1.0, 1.0);
        surface.transform(f.runtime.tree().local_to_surface_transform(*id).unwrap());
        assert_eq!(surface, Point::new(71.0 + old.x, 56.0 + old.y));
        assert_eq!(
            f.runtime
                .tree()
                .hit_test_simple((surface.x(), surface.y()))
                .unwrap(),
            Some(*id)
        );
        let mut local = surface;
        local.transform(
            f.runtime
                .tree()
                .surface_to_local_transform(*id)
                .unwrap()
                .unwrap(),
        );
        assert_eq!(local, Point::new(1.0, 1.0));
    }
    let after_glyphs = glyphs(&after);
    assert_eq!(before_glyphs, after_glyphs);
    for ((before, _), (after, _)) in before_glyphs.iter().zip(&after_glyphs) {
        assert_eq!(
            before.glyphs.buffer().as_slice().as_ptr(),
            after.glyphs.buffer().as_slice().as_ptr()
        );
    }
    let after_images = after
        .commands
        .iter()
        .filter_map(|command| {
            if let RenderCommandKind::Image {
                image,
                source_rect,
                dest_rect,
                ..
            } = command.kind
            {
                Some((image, source_rect, dest_rect))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(before_images, after_images);
    let events = f.ancestors.lock().unwrap();
    for child in &f.children {
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == AncestorEventKind::Moved
                    && event.figure_id == *child
                    && event.parent_id == f.group)
                .count(),
            1
        );
    }
}

#[test]
fn mixed_group_resize_and_reparent_revalidate_real_layout_and_current_coordinates() {
    let mut f = fixture();
    f.runtime
        .figure(f.group)
        .unwrap()
        .set_bounds(Rectangle::new(40.0, 30.0, 320.0, 210.0))
        .unwrap();
    support::record_full_submission(&mut f.runtime);
    assert!(f.layouts.load(Ordering::Relaxed) > 0);
    let new_parent = f
        .runtime
        .figure(f.root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new(100.0, 80.0, 400.0, 300.0).unwrap(),
        ))
        .unwrap();
    f.runtime
        .figure(f.group)
        .unwrap()
        .reparent(new_parent)
        .unwrap();
    support::record_full_submission(&mut f.runtime);
    assert_eq!(
        f.runtime.tree().parent_id(f.group).unwrap(),
        Some(new_parent)
    );
    let shape = f.children[0];
    let mut surface = Point::new(1.0, 1.0);
    surface.transform(f.runtime.tree().local_to_surface_transform(shape).unwrap());
    assert_eq!(surface, Point::new(151.0, 121.0));
    assert_eq!(
        f.runtime
            .tree()
            .hit_test_simple((surface.x(), surface.y()))
            .unwrap(),
        Some(shape)
    );
}
