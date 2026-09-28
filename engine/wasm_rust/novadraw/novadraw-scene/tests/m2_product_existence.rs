use novadraw_core::Color;
use novadraw_geometry::{Rectangle, Vec2};
use novadraw_render::{NdCanvas, command::RenderCommandKind};
use novadraw_scene::{
    Bounded, ChildClippingStrategy, Direction, EllipseFigure, Figure, FigureId, FigureNode,
    FigureTree, InteractionState, LayoutState, LineBorder, NodeState, PolygonFigure,
    PolylineFigure, RectangleFigure, RootFigure, RoundedRectangleFigure, Runtime, TriangleFigure,
    ViewportFigure,
};

const ROOT_COLOR: Color = Color::rgba(0.10, 0.20, 0.30, 1.0);
const ROOT_BORDER_COLOR: Color = Color::rgba(0.15, 0.25, 0.35, 1.0);
const CHILD_COLOR: Color = Color::rgba(0.40, 0.50, 0.60, 1.0);
const CHILD_BORDER_COLOR: Color = Color::rgba(0.45, 0.55, 0.65, 1.0);

#[test]
fn architecture_level_runtime_types_are_public() {
    fn assert_type<T>() {}

    assert_type::<FigureId>();
    assert_type::<FigureNode>();
    assert_type::<FigureTree>();
    assert_type::<NodeState>();
    assert_type::<LayoutState>();
    assert_type::<InteractionState>();

    let runtime = Runtime::default();
    assert!(runtime.tree().contents().is_none());
    assert!(runtime.interaction().mouse_target().is_none());
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

    fn paint_figure(&self, gc: &mut NdCanvas) {
        let b = self.bounds;
        gc.fill_rect(0.0, 0.0, b.width, b.height, self.figure_color);
    }

    fn paint_border(&self, gc: &mut NdCanvas) {
        let b = self.bounds;
        gc.fill_rect(0.0, 0.0, b.width, b.height, self.border_color);
    }
}

#[test]
fn deferred_builtin_figures_remain_importable_without_entering_the_m2_gate() {
    let figures: Vec<Box<dyn Figure>> = vec![
        Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 10.0)),
        Box::new(EllipseFigure::new(0.0, 0.0, 20.0, 10.0)),
        Box::new(PolygonFigure::from_points(vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(20.0, 0.0),
            Vec2::new(10.0, 10.0),
        ])),
        Box::new(RoundedRectangleFigure::new(0.0, 0.0, 20.0, 10.0, 4.0)),
        Box::new(TriangleFigure::new_with_direction(
            0.0,
            0.0,
            20.0,
            10.0,
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

    let figures: Vec<Box<dyn Figure>> = vec![
        Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 10.0).with_child_clipping_strategy(strategy)),
        Box::new(EllipseFigure::new(0.0, 0.0, 20.0, 10.0).with_child_clipping_strategy(strategy)),
        Box::new(PolylineFigure::new(0.0, 0.0, 20.0, 10.0).with_child_clipping_strategy(strategy)),
        Box::new(
            PolygonFigure::from_points(vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(20.0, 0.0),
                Vec2::new(10.0, 10.0),
            ])
            .with_child_clipping_strategy(strategy),
        ),
        Box::new(
            RoundedRectangleFigure::new(0.0, 0.0, 20.0, 10.0, 4.0)
                .with_child_clipping_strategy(strategy),
        ),
        Box::new(TriangleFigure::new(0.0, 0.0, 20.0, 10.0).with_child_clipping_strategy(strategy)),
        Box::new(RootFigure::new(0.0, 0.0, 20.0, 10.0).with_child_clipping_strategy(strategy)),
        Box::new(ViewportFigure::new(0.0, 0.0, 20.0, 10.0).with_child_clipping_strategy(strategy)),
    ];

    assert!(figures.iter().all(|figure| {
        figure
            .container()
            .is_some_and(|container| container.child_clipping_strategy() == strategy)
    }));
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
                Vec2::new(0.0, 0.0),
                Vec2::new(20.0, 0.0),
                Vec2::new(10.0, 10.0),
            ])
            .with_border(border()),
        ),
        Box::new(RoundedRectangleFigure::new(0.0, 0.0, 20.0, 10.0, 4.0).with_border(border())),
        Box::new(TriangleFigure::new(0.0, 0.0, 20.0, 10.0).with_border(border())),
        Box::new(RootFigure::new(0.0, 0.0, 20.0, 10.0).with_border(border())),
        Box::new(ViewportFigure::new(0.0, 0.0, 20.0, 10.0).with_border(border())),
    ];

    assert!(figures.iter().all(|figure| figure.get_border().is_some()));
    assert!(
        figures
            .iter()
            .all(|figure| figure.initial_insets() == (1.0, 2.0, 3.0, 4.0))
    );
}

#[test]
fn m2_figure_tree_product_api_exposes_tree_box_and_z_order_roles() {
    let mut scene = FigureTree::new();
    let root_id = scene
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
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

    assert_eq!(scene.child_order(root_id), Some(vec![bottom_id, top_id]));
    assert_eq!(scene.child_z_index(root_id, bottom_id), Some(0));
    assert_eq!(scene.child_z_index(root_id, top_id), Some(1));
    assert_eq!(scene.hit_test_simple((50.0, 50.0)), Some(top_id));

    assert!(
        scene
            .builder()
            .send_child_to_back(root_id, top_id)
            .expect("valid FigureTree construction")
    );
    assert_eq!(scene.child_order(root_id), Some(vec![top_id, bottom_id]));
    assert_eq!(scene.hit_test_simple((50.0, 50.0)), Some(bottom_id));

    assert!(scene.is_visible(bottom_id));
    assert!(scene.is_enabled(bottom_id));
    assert!(
        scene
            .builder()
            .set_visible(root_id, false)
            .expect("valid FigureTree construction")
    );
    assert!(!scene.is_effectively_visible(bottom_id));
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
    assert!(!scene.is_effectively_enabled(bottom_id));
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
        )));
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

    let gc = Runtime::new(scene).record_full_frame();
    let fill_colors: Vec<_> = gc
        .commands()
        .iter()
        .filter_map(|command| match &command.kind {
            RenderCommandKind::FillRect { color, .. } => Some(color),
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
