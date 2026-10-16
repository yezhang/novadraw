use novadraw::advanced::{FigureNode, InteractionState, LayoutState, NodeState};
use novadraw::container::ViewportFigure;
use novadraw::figure::{
    Bounded, ChildClippingStrategy, Direction, FigureContainer, RootFigure, border::LineBorder,
};
use novadraw::geometry::{Insets, Point, Rectangle};
use novadraw::graphics::Color;
use novadraw::graphics::{GraphicsError, PaintContext};
use novadraw::render::{BackendCapabilities, SurfaceInfo, command::RenderCommandKind};
use novadraw::{
    figure::EllipseFigure, figure::Figure, figure::PolygonFigure, figure::PolylineFigure,
    figure::RectangleFigure, figure::RoundedRectangleFigure, figure::TriangleFigure,
    runtime::FramePreparation, runtime::Runtime, tree::FigureId, tree::FigureTree,
};

const ROOT_COLOR: Color = Color::rgba(0.10, 0.20, 0.30, 1.0);
const ROOT_BORDER_COLOR: Color = Color::rgba(0.15, 0.25, 0.35, 1.0);
const CHILD_COLOR: Color = Color::rgba(0.40, 0.50, 0.60, 1.0);
const CHILD_BORDER_COLOR: Color = Color::rgba(0.45, 0.55, 0.65, 1.0);

#[test]
fn architecture_level_runtime_types_are_available_through_advanced() {
    fn assert_type<T>() {}

    assert_type::<FigureId>();
    assert_type::<FigureNode>();
    assert_type::<FigureTree>();
    assert_type::<NodeState>();
    assert_type::<LayoutState>();
    assert_type::<InteractionState>();

    let mut runtime = Runtime::default();
    assert!(runtime.tree().contents().is_none());
    assert!(runtime.input().state().mouse_target().is_none());
}

#[derive(Clone, Debug)]
struct PaintMarkerFigure {
    bounds: Rectangle,
    figure_color: Color,
    border_color: Color,
}

impl PaintMarkerFigure {
    fn new(bounds: Rectangle, figure_color: Color, border_color: Color) -> Self {
        Self {
            bounds,
            figure_color,
            border_color,
        }
    }
}

impl Bounded for PaintMarkerFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "PaintMarkerFigure"
    }
}

impl Figure for PaintMarkerFigure {
    fn initial_bounds(&self) -> Rectangle {
        Bounded::bounds(self)
    }

    fn name(&self) -> &'static str {
        Bounded::name(self)
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        context.set_fill_paint(self.figure_color);
        context.fill_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))
    }

    fn paint_border(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        context.set_fill_paint(self.border_color);
        context.fill_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))
    }
}

#[test]
fn deferred_builtin_figures_remain_importable_without_entering_the_m2_gate() {
    let figures: Vec<Box<dyn Figure>> = vec![
        Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 10.0)),
        Box::new(EllipseFigure::new(0.0, 0.0, 20.0, 10.0)),
        Box::new(PolygonFigure::from_points(vec![
            Point::new(0.0, 0.0),
            Point::new(20.0, 0.0),
            Point::new(10.0, 10.0),
        ])),
        Box::new(RoundedRectangleFigure::new(0.0, 0.0, 20.0, 10.0, 4.0)),
        Box::new(TriangleFigure::new_with_direction(
            Rectangle::new(0.0, 0.0, 20.0, 10.0),
            Direction::South,
        )),
    ];

    let names: Vec<_> = figures.iter().map(|figure| figure.name()).collect();
    assert_eq!(
        names,
        vec![
            "RectangleFigure",
            "EllipseFigure",
            "PolygonFigure",
            "RoundedRectangleFigure",
            "TriangleFigure"
        ]
    );
    assert!(
        figures
            .iter()
            .all(|figure| figure.initial_bounds().width > 0.0)
    );
}

#[test]
fn existing_product_figures_expose_child_clipping_strategy_api() {
    let strategy = ChildClippingStrategy::DoNotClipChildBounds;

    let strategies = [
        FigureContainer::child_clipping_strategy(
            &RectangleFigure::new(0.0, 0.0, 20.0, 10.0).with_child_clipping_strategy(strategy),
        ),
        FigureContainer::child_clipping_strategy(
            &EllipseFigure::new(0.0, 0.0, 20.0, 10.0).with_child_clipping_strategy(strategy),
        ),
        FigureContainer::child_clipping_strategy(
            &PolylineFigure::new(0.0, 0.0, 20.0, 10.0).with_child_clipping_strategy(strategy),
        ),
        FigureContainer::child_clipping_strategy(
            &PolygonFigure::from_points(vec![
                Point::new(0.0, 0.0),
                Point::new(20.0, 0.0),
                Point::new(10.0, 10.0),
            ])
            .with_child_clipping_strategy(strategy),
        ),
        FigureContainer::child_clipping_strategy(
            &RoundedRectangleFigure::new(0.0, 0.0, 20.0, 10.0, 4.0)
                .with_child_clipping_strategy(strategy),
        ),
        FigureContainer::child_clipping_strategy(
            &TriangleFigure::new(Rectangle::new(0.0, 0.0, 20.0, 10.0))
                .with_child_clipping_strategy(strategy),
        ),
        FigureContainer::child_clipping_strategy(
            &RootFigure::new(0.0, 0.0, 20.0, 10.0).with_child_clipping_strategy(strategy),
        ),
        FigureContainer::child_clipping_strategy(
            &ViewportFigure::new(0.0, 0.0, 20.0, 10.0).with_child_clipping_strategy(strategy),
        ),
    ];

    assert!(strategies.into_iter().all(|actual| actual == strategy));
}

#[test]
fn existing_product_figures_expose_border_api() {
    let border = || LineBorder::new(Color::BLACK, 1.0).with_insets(1.0, 2.0, 3.0, 4.0);

    let figures: Vec<Box<dyn Figure>> = vec![
        Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 10.0).with_border(border())),
        Box::new(EllipseFigure::new(0.0, 0.0, 20.0, 10.0).with_border(border())),
        Box::new(PolylineFigure::new(0.0, 0.0, 20.0, 10.0).with_border(border())),
        Box::new(
            PolygonFigure::from_points(vec![
                Point::new(0.0, 0.0),
                Point::new(20.0, 0.0),
                Point::new(10.0, 10.0),
            ])
            .with_border(border()),
        ),
        Box::new(RoundedRectangleFigure::new(0.0, 0.0, 20.0, 10.0, 4.0).with_border(border())),
        Box::new(TriangleFigure::new(Rectangle::new(0.0, 0.0, 20.0, 10.0)).with_border(border())),
        Box::new(RootFigure::new(0.0, 0.0, 20.0, 10.0).with_border(border())),
        Box::new(ViewportFigure::new(0.0, 0.0, 20.0, 10.0).with_border(border())),
    ];

    assert!(figures.iter().all(|figure| figure.get_border().is_some()));
    assert!(
        figures
            .iter()
            .all(|figure| figure.initial_insets() == Insets::new(1.0, 2.0, 3.0, 4.0))
    );
}

#[test]
fn m2_figure_tree_product_api_exposes_tree_box_and_z_order_roles() {
    let mut scene = FigureTree::new();
    let root_id = scene
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)))
        .expect("valid FigureTree construction");
    let bottom_id = scene
        .builder()
        .add_child(
            root_id,
            Box::new(RectangleFigure::new(20.0, 20.0, 80.0, 80.0)),
        )
        .expect("valid FigureTree construction");
    let top_id = scene
        .builder()
        .add_child(
            root_id,
            Box::new(EllipseFigure::new(30.0, 30.0, 80.0, 80.0)),
        )
        .expect("valid FigureTree construction");

    let root_block = scene.node(root_id).expect("root block should exist");
    assert_eq!(root_block.id(), root_id);
    assert_eq!(root_block.children_count(), 2);
    assert_eq!(
        root_block.figure_bounds(),
        Rectangle::new(0.0, 0.0, 200.0, 200.0)
    );
    assert_eq!(root_block.state().bounds(), root_block.figure_bounds());
    assert!(root_block.state().is_visible());
    assert!(root_block.state().is_enabled());
    assert_eq!(root_block.layout_state().constraint_count(), 0);

    assert_eq!(scene.child_order(root_id), Ok(vec![bottom_id, top_id]));
    assert_eq!(scene.child_z_index(root_id, bottom_id), Ok(Some(0)));
    assert_eq!(scene.child_z_index(root_id, top_id), Ok(Some(1)));
    assert_eq!(scene.hit_test_simple((50.0, 50.0)), Some(top_id));

    assert!(
        scene
            .builder()
            .send_child_to_back(root_id, top_id)
            .expect("valid FigureTree construction")
    );
    assert_eq!(scene.child_order(root_id), Ok(vec![top_id, bottom_id]));
    assert_eq!(scene.hit_test_simple((50.0, 50.0)), Some(bottom_id));

    assert!(scene.is_visible(bottom_id).unwrap());
    assert!(scene.is_enabled(bottom_id).unwrap());
    assert!(
        scene
            .builder()
            .set_visible(root_id, false)
            .expect("valid FigureTree construction")
    );
    assert!(!scene.is_effectively_visible(bottom_id).unwrap());
    assert!(
        scene
            .builder()
            .set_visible(root_id, true)
            .expect("valid FigureTree construction")
    );
    assert!(
        scene
            .builder()
            .set_enabled(root_id, false)
            .expect("valid FigureTree construction")
    );
    assert!(!scene.is_effectively_enabled(bottom_id).unwrap());
}

#[test]
fn m2_three_phase_paint_order_is_observable_from_product_api() {
    let mut scene = FigureTree::new();
    let root_id = scene
        .builder()
        .set_contents(Box::new(PaintMarkerFigure::new(
            Rectangle::new(0.0, 0.0, 100.0, 100.0),
            ROOT_COLOR,
            ROOT_BORDER_COLOR,
        )))
        .expect("valid FigureTree construction");
    scene
        .builder()
        .add_child(
            root_id,
            Box::new(PaintMarkerFigure::new(
                Rectangle::new(10.0, 10.0, 40.0, 40.0),
                CHILD_COLOR,
                CHILD_BORDER_COLOR,
            )),
        )
        .expect("valid FigureTree construction");

    let mut runtime = Runtime::new(scene);
    let FramePreparation::Ready(submission) = runtime.frames().prepare_submission(
        SurfaceInfo {
            logical_width: 100.0,
            logical_height: 100.0,
            pixel_width: 100,
            pixel_height: 100,
            scale_factor: 1.0,
        },
        BackendCapabilities::RETAINED_PARTIAL,
    ) else {
        panic!("initial scene must produce a submission");
    };
    let fill_colors: Vec<_> = submission
        .commands
        .iter()
        .filter_map(|command| match &command.kind {
            RenderCommandKind::FillRect {
                paint: novadraw::graphics::Paint::Solid(color),
                ..
            } => Some(color),
            _ => None,
        })
        .copied()
        .collect();

    assert_eq!(
        fill_colors,
        vec![
            ROOT_COLOR,
            CHILD_COLOR,
            CHILD_BORDER_COLOR,
            ROOT_BORDER_COLOR
        ]
    );
}
