use novadraw::prelude::*;
use novadraw::render::{BackendCapabilities, SurfaceInfo};
use std::{cell::Cell, rc::Rc};

struct PaintCounter(Rc<Cell<usize>>);

impl Figure for PaintCounter {
    fn name(&self) -> &'static str {
        "AuditPaintCounter"
    }

    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 100.0, 100.0)
    }

    fn paint_figure(&self, _canvas: &mut NdCanvas) {
        self.0.set(self.0.get() + 1);
    }
}

fn main() {
    let mut tree = FigureTree::new();
    let root = tree.builder().set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
    let child = tree.builder().add_child(root, Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    tree.builder().set_layout_manager(root, Box::new(StackLayout::new())).unwrap();
    let mut runtime = Runtime::new(tree);
    let before = runtime.tree().figure_bounds(child).unwrap();
    let first = runtime.prepare_frame();
    let after = runtime.tree().figure_bounds(child).unwrap();
    println!("builder_frame_ready={} before={before:?} after={after:?} root_valid={}",
        first.is_some(), runtime.tree().is_valid(root));
    runtime.figure(root).unwrap().revalidate().unwrap();
    runtime.prepare_frame();
    println!("after_explicit_revalidate={:?}", runtime.tree().figure_bounds(child).unwrap());

    let paints = Rc::new(Cell::new(0));
    let mut tree = FigureTree::new();
    tree.builder().set_contents(Box::new(PaintCounter(paints.clone())));
    let mut runtime = Runtime::new(tree);
    let surface = SurfaceInfo {
        logical_width: 100.0, logical_height: 100.0,
        pixel_width: 100, pixel_height: 100, scale_factor: 1.0,
    };
    let frame = runtime.prepare_submission_state(surface, BackendCapabilities::RETAINED_PARTIAL);
    println!("initial_submission_ready={} paint_calls={}",
        matches!(frame, novadraw::runtime::FramePreparation::Ready(_)), paints.get());
}
