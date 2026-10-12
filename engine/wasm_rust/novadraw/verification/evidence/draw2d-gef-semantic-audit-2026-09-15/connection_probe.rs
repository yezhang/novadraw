//! Standalone reproductions for router geometry and group invalidation.
use novadraw_scene::{
    ChopboxAnchor, ConnectionFigure, CoordinateSpace, DirectRouter, FanRouter, FigureTree,
    Point, RectangleFigure, RouterBinding, Runtime, SelfLoopRouter, XYAnchor,
};

fn main() {
    let mut runtime = Runtime::new(FigureTree::new());
    let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 600.0, 400.0)));
    let owner = runtime.add_figure(root, Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 40.0)));
    let figure = runtime.add_figure(root, Box::new(ConnectionFigure::new()));
    let source = runtime.connections().register_anchor(Box::new(ChopboxAnchor::new(owner)));
    let target = runtime.connections().register_anchor(Box::new(ChopboxAnchor::new(owner)));
    let router = runtime.connections().register_router(Box::new(
        SelfLoopRouter::new(Box::new(DirectRouter), 32.0).unwrap(),
    ));
    let connection = runtime.connections().register_state(
        figure, Some(source), Some(target), RouterBinding::Explicit { router }, None,
    ).unwrap();
    let route = runtime.connections().resolve_route(connection, CoordinateSpace::ChildContent(root)).unwrap();
    println!("wide_self_loop owner=(0,0,400,40): {:?}", route.points());
    println!("has_point_beyond_owner_right: {}", route.points().iter().any(|p| p.x() > 400.0));

    let mut runtime = Runtime::new(FigureTree::new());
    let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 600.0, 400.0)));
    let space = CoordinateSpace::ChildContent(root);
    let a = runtime.connections().register_anchor(Box::new(XYAnchor::new(Point::new(0.0, 40.0), space)));
    let b = runtime.connections().register_anchor(Box::new(XYAnchor::new(Point::new(100.0, 40.0), space)));
    let router = runtime.connections().register_router(Box::new(
        FanRouter::new(Box::new(DirectRouter), 16.0).unwrap(),
    ));
    let mut connections = Vec::new();
    for (source, target) in [(a, b), (b, a)] {
        let figure = runtime.add_figure(root, Box::new(ConnectionFigure::new()));
        connections.push(runtime.connections().register_state(
            figure, Some(source), Some(target), RouterBinding::Explicit { router }, None,
        ).unwrap());
    }
    let first = runtime.connections().resolve_route(connections[0], space).unwrap();
    let second = runtime.connections().resolve_route(connections[1], space).unwrap();
    println!("opposite_fan_first: {:?}", first.points());
    println!("opposite_fan_second: {:?}", second.points());
    println!("opposite_fan_midpoints_equal: {}", first.points().get(1) == second.points().get(1));

    let mut runtime = Runtime::new(FigureTree::new());
    let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 600.0, 400.0)));
    let space = CoordinateSpace::ChildContent(root);
    let a = runtime.connections().register_anchor(Box::new(XYAnchor::new(Point::new(0.0, 40.0), space)));
    let b = runtime.connections().register_anchor(Box::new(XYAnchor::new(Point::new(100.0, 40.0), space)));
    let fan = runtime.connections().register_router(Box::new(
        FanRouter::new(Box::new(DirectRouter), 16.0).unwrap(),
    ));
    let direct = runtime.connections().register_router(Box::new(DirectRouter));
    runtime.connections().set_layer_router(root, fan).unwrap();
    let first_figure = runtime.add_figure(root, Box::new(ConnectionFigure::new()));
    let second_figure = runtime.add_figure(root, Box::new(ConnectionFigure::new()));
    let inherited = runtime.connections().register_state(
        first_figure, Some(a), Some(b), RouterBinding::Inherited { layer: root }, None,
    ).unwrap();
    let explicit = runtime.connections().register_state(
        second_figure, Some(a), Some(b), RouterBinding::Explicit { router: fan }, None,
    ).unwrap();
    runtime.connections().resolve_route(inherited, space).unwrap();
    runtime.connections().resolve_route(explicit, space).unwrap();
    runtime.connections().set_layer_router(root, direct).unwrap();
    println!("old_fan_explicit_member_marked_dirty: expected=true actual={}",
        runtime.connections().dirty().contains(&explicit));
}
