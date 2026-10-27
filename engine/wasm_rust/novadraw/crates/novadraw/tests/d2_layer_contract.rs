use novadraw::container::{LayerError, LayerFigure, LayerKey, LayerPlacement, LayeredPane};
use novadraw::event::{EventContext, FigureEventHandler, MouseButton, MouseEvent};
use novadraw::figure::{
    FigureCapabilityBuilder, FigureCapabilityRegistrationError, INPUT, InputCapability,
};
use novadraw::runtime::RuntimeMutationError;
use novadraw::{
    figure::Figure, figure::RectangleFigure, geometry::Rectangle, runtime::Runtime,
    tree::FigureTree,
};

fn key(value: &str) -> LayerKey {
    LayerKey::new(value).expect("test layer key must be valid")
}

struct EnqueueLayersFigure {
    bounds: Rectangle,
    pane: novadraw::tree::FigureId,
}

impl Figure for EnqueueLayersFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "EnqueueLayersFigure"
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())
    }
}

impl FigureEventHandler for EnqueueLayersFigure {
    fn on_mouse_pressed(&self, _event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        let first = key("callback-first");
        let second = key("callback-second");
        ctx.effects().add_layer_later(
            self.pane,
            Box::new(LayerFigure::new(0.0, 0.0, 100.0, 80.0)),
            first.clone(),
            LayerPlacement::Last,
        );
        ctx.effects().add_layer_later(
            self.pane,
            Box::new(LayerFigure::new(0.0, 0.0, 100.0, 80.0)),
            second.clone(),
            LayerPlacement::Last,
        );
        ctx.effects()
            .move_layer_later(self.pane, first, LayerPlacement::After(second));
        true
    }
}

#[test]
fn transparent_layer_returns_descendant_but_never_itself() {
    let mut tree = FigureTree::new();
    let layer = tree
        .builder()
        .set_contents(Box::new(LayerFigure::new(0.0, 0.0, 100.0, 100.0)))
        .expect("valid FigureTree construction");

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
        .layers()
        .add_pane(root, Rectangle::new(0.0, 0.0, 200.0, 120.0))
        .expect("pane should be added");
    let content = key("content");

    let layer = {
        let layer = runtime
            .layers()
            .add(
                pane,
                Box::new(LayerFigure::new(0.0, 0.0, 200.0, 120.0)),
                content.clone(),
                LayerPlacement::Last,
            )
            .expect("first key should be accepted");
        assert_eq!(
            runtime.layers().add(
                pane,
                Box::new(LayerFigure::new(0.0, 0.0, 200.0, 120.0)),
                content,
                LayerPlacement::Last,
            ),
            Err(LayerError::DuplicateKey)
        );
        assert_eq!(
            runtime.layers().add(
                pane,
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
            .figure(pane)
            .unwrap()
            .add(Box::new(LayerFigure::new(0.0, 0.0, 200.0, 120.0)),),
        Err(RuntimeMutationError::LayeredParent(pane))
    );
    assert_eq!(
        runtime.figure(pane).unwrap().remove(layer),
        Err(RuntimeMutationError::LayeredParent(pane))
    );
    assert_eq!(
        runtime.layers().ids(pane).expect("consistent state").len(),
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
        .layers()
        .add(
            pane,
            Box::new(LayerFigure::new(0.0, 0.0, 160.0, 100.0)),
            key("content"),
            LayerPlacement::Last,
        )
        .expect("typed add should work");

    assert_eq!(runtime.tree().parent_id(layer), Ok(Some(pane)));
}

#[test]
fn layer_order_drives_reverse_z_hit_testing() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 120.0)))
        .expect("valid Runtime mutation");
    let pane = runtime
        .layers()
        .add_pane(root, Rectangle::new(0.0, 0.0, 200.0, 120.0))
        .expect("pane should be added");
    let lower_key = key("lower");
    let middle_key = key("middle");
    let upper_key = key("upper");

    let (lower, middle, upper) = {
        let lower = runtime
            .layers()
            .add(
                pane,
                Box::new(LayerFigure::new(0.0, 0.0, 200.0, 120.0)),
                lower_key.clone(),
                LayerPlacement::Last,
            )
            .expect("lower layer");
        let upper = runtime
            .layers()
            .add(
                pane,
                Box::new(LayerFigure::new(0.0, 0.0, 200.0, 120.0)),
                upper_key.clone(),
                LayerPlacement::Last,
            )
            .expect("upper layer");
        let middle = runtime
            .layers()
            .add(
                pane,
                Box::new(LayerFigure::new(0.0, 0.0, 200.0, 120.0)),
                middle_key,
                LayerPlacement::Before(upper_key.clone()),
            )
            .expect("middle layer");
        assert_eq!(
            runtime.layers().ids(pane).expect("ordered layers"),
            vec![lower, middle, upper]
        );
        (lower, middle, upper)
    };
    let lower_child = runtime
        .figure(lower)
        .unwrap()
        .add(Box::new(RectangleFigure::new(10.0, 10.0, 40.0, 40.0)))
        .expect("valid Runtime mutation");
    let upper_child = runtime
        .figure(upper)
        .unwrap()
        .add(Box::new(RectangleFigure::new(10.0, 10.0, 40.0, 40.0)))
        .expect("valid Runtime mutation");
    let _middle_child = runtime
        .figure(middle)
        .unwrap()
        .add(Box::new(RectangleFigure::new(10.0, 10.0, 40.0, 40.0)))
        .expect("valid Runtime mutation");

    assert_eq!(
        runtime.tree().hit_test_simple((20.0, 20.0)),
        Some(upper_child)
    );

    runtime
        .layers()
        .move_to(pane, &lower_key, LayerPlacement::After(upper_key))
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
        .layers()
        .add_pane(root, Rectangle::new(0.0, 0.0, 140.0, 160.0))
        .expect("left pane");
    let right = runtime
        .layers()
        .add_pane(root, Rectangle::new(160.0, 0.0, 140.0, 160.0))
        .expect("right pane");
    let old_key = key("old");
    let new_key = key("new");
    let layer = runtime
        .layers()
        .add(
            left,
            Box::new(LayerFigure::new(0.0, 0.0, 140.0, 160.0)),
            old_key.clone(),
            LayerPlacement::Last,
        )
        .expect("layer");

    assert_eq!(
        runtime.figure(layer).unwrap().reparent(right),
        Err(RuntimeMutationError::LayeredParent(left))
    );
    assert_eq!(runtime.tree().parent_id(layer), Ok(Some(left)));

    runtime
        .layers()
        .reparent(layer, right, new_key.clone(), LayerPlacement::Last)
        .expect("reparent should succeed");

    assert_eq!(
        runtime.layers().layer(left, &old_key),
        Err(LayerError::UnknownKey)
    );
    assert_eq!(runtime.layers().layer(right, &new_key), Ok(layer));

    let removed = runtime
        .layers()
        .remove(right, &new_key)
        .expect("remove should succeed");
    assert_eq!(removed, layer);
    assert_eq!(
        runtime.tree().parent_id(layer),
        Err(novadraw::tree::TreeQueryError::UnknownFigure(layer))
    );
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
        .layers()
        .add_pane(root, Rectangle::new(120.0, 0.0, 100.0, 80.0))
        .expect("pane");
    runtime
        .figure(root)
        .unwrap()
        .add(Box::new(EnqueueLayersFigure {
            bounds: Rectangle::new(0.0, 0.0, 100.0, 80.0),
            pane,
        }))
        .expect("valid Runtime mutation");

    runtime
        .input()
        .dispatch_mouse_pressed(20.0, 20.0, MouseButton::Left);

    let first = runtime
        .layers()
        .layer(pane, &key("callback-first"))
        .expect("first layer");
    let second = runtime
        .layers()
        .layer(pane, &key("callback-second"))
        .expect("second layer");
    assert_eq!(
        runtime.layers().ids(pane).expect("ordered layers"),
        vec![second, first]
    );
}

struct ResourceLayer {
    resource: novadraw::render::ResourceId,
    attached: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Figure for ResourceLayer {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }

    fn name(&self) -> &'static str {
        "ExternalResourceLayer"
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        use novadraw::figure::{
            LAYER, LIFECYCLE, LayerCapability, LifecycleCapability, RESOURCE_DEPENDENCIES,
            ResourceDependenciesCapability,
        };
        out.register(LAYER, LayerCapability)?;
        out.register(
            RESOURCE_DEPENDENCIES,
            ResourceDependenciesCapability::of::<Self>(),
        )?;
        out.register(LIFECYCLE, LifecycleCapability::of::<Self>())
    }
}

impl novadraw::figure::FigureResourceDependencies for ResourceLayer {
    fn resource_dependencies(&self) -> Vec<novadraw::render::ResourceId> {
        vec![self.resource, self.resource]
    }
}

impl novadraw::figure::FigureLifecycle for ResourceLayer {
    fn on_attached(&mut self, _: novadraw::figure::FigureLifecycleContext) {
        self.attached
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn keyed_layer_resource_rejection_preserves_keys_topology_and_lifecycle() {
    use novadraw::runtime::ResourceError;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    for retired in [false, true] {
        let mut runtime = Runtime::empty();
        let pane = runtime
            .set_contents(Box::new(LayeredPane::new(0.0, 0.0, 100.0, 80.0)))
            .unwrap();
        let existing = runtime
            .layers()
            .add(
                pane,
                Box::new(LayerFigure::new(0.0, 0.0, 100.0, 80.0)),
                key("existing"),
                LayerPlacement::Last,
            )
            .unwrap();
        let mut other = Runtime::empty();
        let resource = if retired {
            let resource = runtime.resources().register_image().unwrap().resource_id();
            runtime.resources().remove(resource).unwrap();
            resource
        } else {
            other.resources().register_image().unwrap().resource_id()
        };
        let attached = Arc::new(AtomicUsize::new(0));
        let candidate = key("candidate");
        assert_eq!(
            runtime.layers().add(
                pane,
                Box::new(ResourceLayer {
                    resource,
                    attached: attached.clone()
                }),
                candidate.clone(),
                LayerPlacement::Before(key("existing")),
            ),
            Err(LayerError::Resource(if retired {
                ResourceError::UnknownResource
            } else {
                ResourceError::WrongNamespace
            })),
        );
        assert_eq!(runtime.layers().ids(pane).unwrap(), [existing]);
        assert_eq!(runtime.tree().child_order(pane).unwrap(), [existing]);
        assert_eq!(
            runtime.layers().layer(pane, &candidate),
            Err(LayerError::UnknownKey)
        );
        assert_eq!(attached.load(Ordering::SeqCst), 0);
        assert!(!runtime.is_faulted());
        runtime
            .layers()
            .add(
                pane,
                Box::new(LayerFigure::new(0.0, 0.0, 100.0, 80.0)),
                candidate,
                LayerPlacement::Before(key("existing")),
            )
            .unwrap();
        assert_eq!(runtime.layers().ids(pane).unwrap()[1], existing);
    }
}

#[test]
fn keyed_layer_pending_resource_registration_is_deduplicated_and_released() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let mut runtime = Runtime::empty();
    let pane = runtime
        .set_contents(Box::new(LayeredPane::new(0.0, 0.0, 100.0, 80.0)))
        .unwrap();
    let resource = runtime.resources().register_image().unwrap().resource_id();
    let attached = Arc::new(AtomicUsize::new(0));
    let layer = runtime
        .layers()
        .add(
            pane,
            Box::new(ResourceLayer {
                resource,
                attached: attached.clone(),
            }),
            key("resource"),
            LayerPlacement::Last,
        )
        .unwrap();
    assert_eq!(attached.load(Ordering::SeqCst), 1);
    assert!(
        runtime
            .resources()
            .remove_dependency(resource, layer)
            .unwrap()
    );
    assert!(
        !runtime
            .resources()
            .remove_dependency(resource, layer)
            .unwrap()
    );
    runtime.resources().add_dependency(resource, layer).unwrap();
    runtime.layers().remove(pane, &key("resource")).unwrap();
    assert!(
        !runtime
            .resources()
            .remove_dependency(resource, layer)
            .unwrap()
    );
}

struct EnqueueResourceLayer {
    pane: novadraw::tree::FigureId,
    resource: novadraw::render::ResourceId,
    attached: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Figure for EnqueueResourceLayer {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 20.0, 20.0)
    }
    fn name(&self) -> &'static str {
        "EnqueueResourceLayer"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())
    }
}

impl FigureEventHandler for EnqueueResourceLayer {
    fn on_mouse_pressed(&self, _: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        ctx.effects().add_layer_later(
            self.pane,
            Box::new(ResourceLayer {
                resource: self.resource,
                attached: self.attached.clone(),
            }),
            key("queued"),
            LayerPlacement::Last,
        );
        ctx.effects().add_layer_later(
            self.pane,
            Box::new(LayerFigure::new(0.0, 0.0, 100.0, 80.0)),
            key("queued"),
            LayerPlacement::Last,
        );
        true
    }
}

#[test]
fn deferred_layer_resource_rejection_keeps_error_and_allows_next_request() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 80.0)))
        .unwrap();
    let pane = runtime
        .layers()
        .add_pane(root, Rectangle::new(100.0, 0.0, 100.0, 80.0))
        .unwrap();
    let mut other = Runtime::empty();
    let resource = other.resources().register_image().unwrap().resource_id();
    let attached = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    runtime
        .figure(root)
        .unwrap()
        .add(Box::new(EnqueueResourceLayer {
            pane,
            resource,
            attached: attached.clone(),
        }))
        .unwrap();
    runtime
        .input()
        .dispatch_mouse_pressed(5.0, 5.0, MouseButton::Left);
    assert!(matches!(
        runtime.take_deferred_mutation_errors().as_slice(),
        [RuntimeMutationError::Resource(
            novadraw::runtime::ResourceError::WrongNamespace
        )]
    ));
    assert_eq!(runtime.layers().ids(pane).unwrap().len(), 1);
    assert!(runtime.layers().layer(pane, &key("queued")).is_ok());
    assert_eq!(attached.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(!runtime.is_faulted());
}
