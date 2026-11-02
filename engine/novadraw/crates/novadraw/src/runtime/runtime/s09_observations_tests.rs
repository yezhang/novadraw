use super::*;
use crate::event::{NotificationRecord, ObservationListener};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

struct Observer(Rc<Cell<usize>>);
impl ObservationListener for Observer {
    fn observed(
        &self,
        _: &NotificationRecord,
        _: StableSceneQuery<'_>,
    ) -> crate::event::ListenerDirective {
        self.0.set(self.0.get() + 1);
        crate::event::ListenerDirective::Keep
    }
}

#[test]
fn listener_exhaustion_rejects_all_families_without_faulting_or_removing_existing_owner() {
    let mut runtime = Runtime::empty();
    let owner = runtime
        .set_contents(Box::new(
            crate::figure::RectangleFigure::new(0.0, 0.0, 10.0, 10.0).unwrap(),
        ))
        .unwrap();
    let observed = Rc::new(Cell::new(0));
    let existing = runtime
        .add_observation_listener_scoped(
            ListenerScope::Figure(owner),
            Box::new(Observer(observed.clone())),
        )
        .unwrap();
    runtime.updates.exhaust_listener_ids_for_test();
    let before = runtime.updates.listener_admission_facts_for_test();
    // The eight entry families share the same checked allocator, including scoped admission.
    struct Typed;
    impl crate::event::FigureListener for Typed {
        fn figure_moved(&self, _: crate::event::FigureEvent) -> crate::event::ListenerDirective {
            Default::default()
        }
    }
    impl crate::event::CoordinateListener for Typed {
        fn coordinate_system_changed(
            &self,
            _: crate::event::FigureEvent,
        ) -> crate::event::ListenerDirective {
            Default::default()
        }
    }
    impl crate::event::AncestorListener for Typed {
        fn ancestor_changed(
            &self,
            _: crate::event::AncestorEvent,
        ) -> crate::event::ListenerDirective {
            Default::default()
        }
    }
    impl crate::event::PropertyChangeListener for Typed {
        fn property_changed(
            &self,
            _: &crate::event::PropertyChangeEvent,
        ) -> crate::event::ListenerDirective {
            Default::default()
        }
    }
    impl crate::event::ActionListener for Typed {
        fn action_performed(
            &self,
            _: crate::event::ActionEvent,
        ) -> crate::event::ListenerDirective {
            Default::default()
        }
    }
    impl crate::event::LayoutListener for Typed {
        fn layout_changed(&self, _: crate::event::LayoutEvent) -> crate::event::ListenerDirective {
            Default::default()
        }
    }
    macro_rules! rejected {
        ($call:expr) => {
            assert!(matches!(
                $call,
                Err(RuntimeMutationError::ListenerIdExhausted)
            ));
        };
    }
    rejected!(runtime.add_update_listener(Box::new(())));
    rejected!(runtime.add_figure_listener(Box::new(Typed)));
    rejected!(runtime.add_coordinate_listener(Box::new(Typed)));
    rejected!(runtime.add_ancestor_listener(Box::new(Typed)));
    rejected!(runtime.add_property_listener(Box::new(Typed)));
    rejected!(runtime.add_action_listener(Box::new(Typed)));
    rejected!(runtime.add_layout_listener(Box::new(Typed)));
    rejected!(runtime.add_observation_listener(Box::new(Observer(observed.clone()))));
    rejected!(runtime.add_observation_listener_scoped(
        ListenerScope::Figure(owner),
        Box::new(Observer(observed.clone()))
    ));
    rejected!(runtime.add_update_listener_scoped(ListenerScope::Figure(owner), Box::new(())));
    rejected!(runtime.add_figure_listener_scoped(ListenerScope::Figure(owner), Box::new(Typed)));
    rejected!(
        runtime.add_coordinate_listener_scoped(ListenerScope::Figure(owner), Box::new(Typed))
    );
    rejected!(runtime.add_ancestor_listener_scoped(ListenerScope::Figure(owner), Box::new(Typed)));
    rejected!(runtime.add_property_listener_scoped(ListenerScope::Figure(owner), Box::new(Typed)));
    rejected!(runtime.add_action_listener_scoped(ListenerScope::Figure(owner), Box::new(Typed)));
    rejected!(runtime.add_layout_listener_scoped(ListenerScope::Figure(owner), Box::new(Typed)));
    assert_eq!(before, runtime.updates.listener_admission_facts_for_test());
    assert!(!runtime.is_faulted());
    runtime.frames().stabilize_for_query().unwrap();
    assert!(observed.get() > 0);
    assert!(runtime.remove_listener(existing).unwrap());
    rejected!(runtime.add_update_listener(Box::new(())));
}

#[test]
fn ordinary_observer_panic_faults_runtime_and_allows_subscription_cleanup() {
    struct Panic;
    impl ObservationListener for Panic {
        fn observed(
            &self,
            _: &NotificationRecord,
            _: StableSceneQuery<'_>,
        ) -> crate::event::ListenerDirective {
            panic!("external observer");
        }
    }
    let mut runtime = Runtime::empty();
    runtime
        .set_contents(Box::new(
            crate::figure::RectangleFigure::new(0.0, 0.0, 10.0, 10.0).unwrap(),
        ))
        .unwrap();
    let healthy = runtime.add_update_listener(Box::new(())).unwrap();
    let failing = runtime.add_observation_listener(Box::new(Panic)).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| runtime.frames().stabilize_for_query())).is_err());
    assert!(runtime.is_faulted());
    assert_eq!(
        runtime.observation_checkpoint(),
        Err(StableQueryError::Faulted)
    );
    assert!(runtime.remove_listener(healthy).unwrap());
    assert!(runtime.remove_listener(failing).unwrap());
    assert!(!runtime.remove_listener(failing).unwrap());
    assert!(matches!(
        runtime.add_update_listener(Box::new(())),
        Err(RuntimeMutationError::Faulted)
    ));
}
