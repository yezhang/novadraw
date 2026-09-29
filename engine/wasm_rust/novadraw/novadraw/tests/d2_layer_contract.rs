use novadraw::{
    EventContext, Figure, FigureEventHandler, FigureTree, LayerError, LayerFigure, LayerKey,
    LayerPlacement, LayeredPane, MouseButton, MouseEvent, Rectangle, RectangleFigure, Runtime,
};

fn key(value: &str) -> LayerKey {
    LayerKey::new(value).expect("test layer key must be valid")
}

struct EnqueueLayersFigure {
    bounds: Rectangle,
    pane: novadraw::FigureId,
}

impl Figure for EnqueueLayersFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "EnqueueLayersFigure"
    }

    fn event_handler(&self) -> Option<&dyn FigureEventHandler> {
        Some(self)
    }
}

impl FigureEventHandler for EnqueueLayersFigure {
    fn on_mouse_pressed(&self, _event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        let first = key("callback-first");
        let second = key("callback-second");
        ctx.add_layer_later(
            self.pane,
            Box::new(LayerFigure::new(0.0, 0.0, 100.0, 80.0)),
            first.clone(),
            LayerPlacement::Last,
        );
        ctx.add_layer_later(
            self.pane,
            Box::new(LayerFigure::new(0.0, 0.0, 100.0, 80.0)),
            second.clone(),
            LayerPlacement::Last,
        );
        ctx.move_layer_later(self.pane, first, LayerPlacement::After(second));
        true
    }
}

#[test]
fn transparent_layer_returns_descendant_but_never_itself() {
    let mut tree = FigureTree::new();
    let layer = tree
        .builder()
        .set_contents(Box::new(LayerFigure::new(0.0, 0.0, 100.0, 100.0)));

    assert_eq!(tree.hit_test_simple((50.0, 50.0)), None);

    let child = tree
        .builder()
        .add_child(
            layer,
            Box::new(RectangleFigure::new(20.0, 20.0, 30.0, 30.0)),
        )
        .expect("valid FigureTree construction");
    assert_eq!(tree.hit_test_simple((25.0, 25.0)), Some(child));
}

#[test]
fn layered_pane_rejects_duplicate_keys_and_generic_add() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 120.0)))
        .expect("valid Runtime mutation");
    let pane = runtime
        .add_layered_pane(root, Rectangle::new(0.0, 0.0, 200.0, 120.0))
        .expect("pane should be added")
        .pane_id();
    let content = key("content");

    let layer = {
        let mut handle = runtime.layered_pane(pane).expect("pane handle");
        let layer = handle
            .add_layer(
                Box::new(LayerFigure::new(0.0, 0.0, 200.0, 120.0)),
                content.clone(),
                LayerPlacement::Last,
            )
            .expect("first key should be accepted");
        assert_eq!(
            handle.add_layer(
                Box::new(LayerFigure::new(0.0, 0.0, 200.0, 120.0)),
                content,
                LayerPlacement::Last,
            ),
            Err(LayerError::DuplicateKey)
        );
        assert_eq!(
            handle.add_layer(
                Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)),
                key("not-layer"),
                LayerPlacement::Last,
            ),
            Err(LayerError::NotLayer)
        );
        layer
    };

    assert_eq!(
        runtime
            .container(pane)
            .unwrap()
            .add(Box::new(LayerFigure::new(0.0, 0.0, 200.0, 120.0)),),
        Err(novadraw::RuntimeMutationError::LayeredParent(pane))
    );
    assert_eq!(
        runtime.container(pane).unwrap().remove(layer),
        Err(novadraw::RuntimeMutationError::LayeredParent(pane))
    );
    assert_eq!(
        runtime
            .layered_pane(pane)
            .expect("pane handle")
            .layer_ids()
            .expect("consistent state")
            .len(),
        1
    );
}

#[test]
fn runtime_registers_layered_pane_used_as_contents() {
    let mut runtime = Runtime::empty();
    let pane = runtime
        .set_contents(Box::new(LayeredPane::new(0.0, 0.0, 160.0, 100.0)))
        .expect("valid Runtime mutation");
    let layer = runtime
        .layered_pane(pane)
        .expect("contents pane should be registered")
        .add_layer(
            Box::new(LayerFigure::new(0.0, 0.0, 160.0, 100.0)),
            key("content"),
            LayerPlacement::Last,
        )
        .expect("typed add should work");

    assert_eq!(runtime.tree().parent_id(layer), Some(pane));
}

#[test]
fn layer_order_drives_reverse_z_hit_testing() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 120.0)))
        .expect("valid Runtime mutation");
    let pane = runtime
        .add_layered_pane(root, Rectangle::new(0.0, 0.0, 200.0, 120.0))
        .expect("pane should be added")
        .pane_id();
    let lower_key = key("lower");
    let middle_key = key("middle");
    let upper_key = key("upper");

    let (lower, middle, upper) = {
        let mut handle = runtime.layered_pane(pane).expect("pane handle");
        let lower = handle
            .add_layer(
                Box::new(LayerFigure::new(0.0, 0.0, 200.0, 120.0)),
                lower_key.clone(),
                LayerPlacement::Last,
            )
            .expect("lower layer");
        let upper = handle
            .add_layer(
                Box::new(LayerFigure::new(0.0, 0.0, 200.0, 120.0)),
                upper_key.clone(),
                LayerPlacement::Last,
            )
            .expect("upper layer");
        let middle = handle
            .add_layer(
                Box::new(LayerFigure::new(0.0, 0.0, 200.0, 120.0)),
                middle_key,
                LayerPlacement::Before(upper_key.clone()),
            )
            .expect("middle layer");
        assert_eq!(
            handle.layer_ids().expect("ordered layers"),
            vec![lower, middle, upper]
        );
        (lower, middle, upper)
    };
    let lower_child = runtime
        .container(lower)
        .unwrap()
        .add(Box::new(RectangleFigure::new(10.0, 10.0, 40.0, 40.0)))
        .expect("valid Runtime mutation");
    let upper_child = runtime
        .container(upper)
        .unwrap()
        .add(Box::new(RectangleFigure::new(10.0, 10.0, 40.0, 40.0)))
        .expect("valid Runtime mutation");
    let _middle_child = runtime
        .container(middle)
        .unwrap()
        .add(Box::new(RectangleFigure::new(10.0, 10.0, 40.0, 40.0)))
        .expect("valid Runtime mutation");

    assert_eq!(
        runtime.tree().hit_test_simple((20.0, 20.0)),
        Some(upper_child)
    );

    runtime
        .layered_pane(pane)
        .expect("pane handle")
        .move_layer(&lower_key, LayerPlacement::After(upper_key))
        .expect("move should succeed");

    assert_eq!(
        runtime.tree().hit_test_simple((20.0, 20.0)),
        Some(lower_child)
    );
}

#[test]
fn reparent_and_remove_update_both_membership_indexes() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 160.0)))
        .expect("valid Runtime mutation");
    let left = runtime
        .add_layered_pane(root, Rectangle::new(0.0, 0.0, 140.0, 160.0))
        .expect("left pane")
        .pane_id();
    let right = runtime
        .add_layered_pane(root, Rectangle::new(160.0, 0.0, 140.0, 160.0))
        .expect("right pane")
        .pane_id();
    let old_key = key("old");
    let new_key = key("new");
    let layer = runtime
        .layered_pane(left)
        .expect("left handle")
        .add_layer(
            Box::new(LayerFigure::new(0.0, 0.0, 140.0, 160.0)),
            old_key.clone(),
            LayerPlacement::Last,
        )
        .expect("layer");

    assert_eq!(
        runtime.figure(layer).unwrap().reparent(right),
        Err(novadraw::RuntimeMutationError::LayeredParent(left))
    );
    assert_eq!(runtime.tree().parent_id(layer), Some(left));

    runtime
        .layered_pane(right)
        .expect("right handle")
        .reparent_layer(layer, new_key.clone(), LayerPlacement::Last)
        .expect("reparent should succeed");

    assert_eq!(
        runtime
            .layered_pane(left)
            .expect("left handle")
            .layer(&old_key),
        Err(LayerError::UnknownKey)
    );
    assert_eq!(
        runtime
            .layered_pane(right)
            .expect("right handle")
            .layer(&new_key),
        Ok(layer)
    );

    let removed = runtime
        .layered_pane(right)
        .expect("right handle")
        .remove_layer(&new_key)
        .expect("remove should succeed");
    assert_eq!(removed, layer);
    assert_eq!(runtime.tree().parent_id(layer), None);
}

#[test]
fn layer_key_rejects_empty_values() {
    assert!(LayerKey::new("").is_err());
    assert_eq!(LayerKey::new("content").unwrap().as_str(), "content");
}

#[test]
fn callback_layer_mutations_commit_in_fifo_order() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 240.0, 120.0)))
        .expect("valid Runtime mutation");
    let pane = runtime
        .add_layered_pane(root, Rectangle::new(120.0, 0.0, 100.0, 80.0))
        .expect("pane")
        .pane_id();
    runtime
        .container(root)
        .unwrap()
        .add(Box::new(EnqueueLayersFigure {
            bounds: Rectangle::new(0.0, 0.0, 100.0, 80.0),
            pane,
        }))
        .expect("valid Runtime mutation");

    runtime.dispatch_mouse_pressed(20.0, 20.0, MouseButton::Left);

    let handle = runtime.layered_pane(pane).expect("pane handle");
    let first = handle.layer(&key("callback-first")).expect("first layer");
    let second = handle.layer(&key("callback-second")).expect("second layer");
    assert_eq!(
        handle.layer_ids().expect("ordered layers"),
        vec![second, first]
    );
}
