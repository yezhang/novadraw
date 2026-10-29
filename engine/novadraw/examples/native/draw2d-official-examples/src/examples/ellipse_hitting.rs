use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use novadraw::{
    event::{EventContext, FigureEventHandler, MouseEvent},
    figure::{
        Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError, INPUT, InputCapability,
        RectangleFigure,
    },
    geometry::Rectangle,
    graphics::{Color, GraphicsError, PaintContext},
    layout::FlowLayout,
    runtime::Runtime,
    tree::FigureTree,
};

const WIDTH: f64 = 400.0;
const HEIGHT: f64 = 300.0;
const SPACING: f64 = 5.0;
const HOVER: Color = Color::rgba(1.0, 1.0, 0.0, 1.0);

struct HoverEllipse {
    bounds: Rectangle,
    hovered: Arc<AtomicBool>,
}

impl HoverEllipse {
    fn new(bounds: Rectangle, hovered: Arc<AtomicBool>) -> Self {
        Self { bounds, hovered }
    }
}

impl Figure for HoverEllipse {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "Ellipse"
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        context.set_fill_paint(if self.hovered.load(Ordering::Relaxed) {
            HOVER
        } else {
            Color::WHITE
        });
        context.fill_ellipse(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))?;
        context.set_stroke_paint(Color::BLACK);
        context.set_stroke_width(1.0)?;
        context.stroke_ellipse(Rectangle::new(
            0.5,
            0.5,
            bounds.width - 1.0,
            bounds.height - 1.0,
        ))
    }

    fn precise_hit(&self, x: f64, y: f64, bounds: Rectangle) -> bool {
        let radius_x = bounds.width / 2.0;
        let radius_y = bounds.height / 2.0;
        if radius_x <= 0.0 || radius_y <= 0.0 {
            return false;
        }
        let x = (x - radius_x) / radius_x;
        let y = (y - radius_y) / radius_y;
        x * x + y * y <= 1.0
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())
    }
}

impl FigureEventHandler for HoverEllipse {
    fn on_mouse_entered(&self, _event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        self.hovered.store(true, Ordering::Relaxed);
        context.effects().repaint(None);
        true
    }

    fn on_mouse_exited(&self, _event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        self.hovered.store(false, Ordering::Relaxed);
        context.effects().repaint(None);
        true
    }
}

pub fn ellipse_hitting() -> Runtime {
    fixture().0
}

fn fixture() -> (Runtime, Vec<Arc<AtomicBool>>) {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(
            RectangleFigure::new_with_color(0.0, 0.0, WIDTH, HEIGHT, Color::WHITE)
                .expect("valid figure bounds"),
        ))
        .expect("EllipseHitting root");
    tree.builder()
        .set_layout_manager(
            root,
            Box::new(
                FlowLayout::new()
                    .with_spacing(SPACING)
                    .with_row_spacing(SPACING),
            ),
        )
        .expect("EllipseHitting FlowLayout");

    let mut states = Vec::with_capacity(4);
    for index in 1..=4 {
        let hovered = Arc::new(AtomicBool::new(false));
        tree.builder()
            .add_child(
                root,
                Box::new(HoverEllipse::new(
                    Rectangle::new(
                        0.0,
                        0.0,
                        20.0 + 10.0 * f64::from(index) + f64::from(index % 2),
                        60.0 - 10.0 * f64::from(index) + f64::from(index / 2),
                    ),
                    hovered.clone(),
                )),
            )
            .expect("EllipseHitting ellipse");
        states.push(hovered);
    }
    tree.builder()
        .validate_subtree(root)
        .expect("EllipseHitting layout");
    (Runtime::new(tree), states)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figure_tree_and_sizes_match_the_official_example() {
        let (runtime, _) = fixture();
        let root = runtime.tree().contents().expect("root");
        let children = runtime.tree().child_order(root).expect("ellipses");
        assert_eq!(children.len(), 4);
        let sizes = children
            .into_iter()
            .map(|child| runtime.tree().figure_bounds(child).expect("ellipse bounds"))
            .map(|bounds| (bounds.width, bounds.height))
            .collect::<Vec<_>>();
        assert_eq!(
            sizes,
            vec![(31.0, 50.0), (40.0, 41.0), (51.0, 31.0), (60.0, 22.0)]
        );
    }

    #[test]
    fn pointer_entry_and_exit_match_the_mouse_motion_listener() {
        let (mut runtime, states) = fixture();
        let root = runtime.tree().contents().expect("root");
        let first = runtime.tree().child_order(root).expect("ellipses")[0];
        let bounds = runtime.tree().figure_bounds(first).expect("ellipse bounds");

        runtime
            .input()
            .dispatch_mouse_moved(bounds.center().x(), bounds.center().y());
        assert!(states[0].load(Ordering::Relaxed));

        runtime.input().dispatch_mouse_moved(390.0, 290.0);
        assert!(!states[0].load(Ordering::Relaxed));
    }
}
