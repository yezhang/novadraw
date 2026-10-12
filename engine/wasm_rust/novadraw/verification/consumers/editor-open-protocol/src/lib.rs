#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        convert::Infallible,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    use novadraw::{
        event::{DispatchOutcome, KeyModifiers, MouseButton},
        figure::{Figure, RectangleFigure, RootFigure},
        geometry::{Point, Rectangle},
    };
    use novadraw_editor::overlay::{HandleActionKey, HandleActionProtocol};
    use novadraw_editor::{
        EditPartBehavior, EditPartError, EditPartFactory, EditPartId, EditPolicy, EditorDomain,
        GraphicalViewer, InteractionRevision, ModelAdapter, ModelEvent, ModelRevision,
        PartFactoryContext, PolicyError, PolicyHandlerBuilder, PolicyHost, PolicyInstallation,
        PolicyKey, PolicySlot, RequestKey, RequestMetadata, RequestProtocol, RequestResolution,
        Tool, ToolContext, ToolDirective, ToolError, ToolKey, ToolMouseEvent, ViewerInputOutcome,
    };

    const ROOT: ModelId = ModelId(1);
    const NODE: ModelId = ModelId(2);

    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    struct ModelId(u64);

    struct Model;

    impl ModelAdapter for Model {
        type ModelId = ModelId;
        type Event = ();
        type Error = Infallible;

        fn root(&self) -> Self::ModelId {
            ROOT
        }

        fn revision(&self) -> ModelRevision {
            ModelRevision::initial()
        }

        fn children(&self, model: Self::ModelId) -> Result<Vec<Self::ModelId>, Self::Error> {
            Ok(if model == ROOT {
                vec![NODE]
            } else {
                Vec::new()
            })
        }

        fn drain_events(&mut self) -> Vec<ModelEvent<Self::ModelId, Self::Event>> {
            Vec::new()
        }
    }

    struct AlignToGuide;

    #[derive(Clone, Copy)]
    struct AlignPayload {
        coordinate: f64,
    }

    impl RequestProtocol<Model> for AlignToGuide {
        type Payload = AlignPayload;
        type Contribution = f64;

        fn key() -> RequestKey<Self> {
            RequestKey::new("external-align-to-guide")
        }

        fn reduce(
            contributions: Vec<Self::Contribution>,
        ) -> Result<RequestResolution<Self::Contribution>, PolicyError> {
            Ok(RequestResolution::all(contributions))
        }
    }

    struct GuidePolicy {
        handled: Rc<Cell<usize>>,
    }

    impl GuidePolicy {
        fn align(
            &mut self,
            _host: PolicyHost<ModelId>,
            _metadata: &RequestMetadata,
            payload: &AlignPayload,
            _model: &Model,
        ) -> Result<Option<f64>, PolicyError> {
            self.handled.set(self.handled.get() + 1);
            Ok(Some(payload.coordinate))
        }
    }

    impl EditPolicy<Model> for GuidePolicy {
        fn register_handlers(builder: &mut PolicyHandlerBuilder<'_, Model, Self>) {
            builder.handle::<AlignToGuide>(Self::align);
        }
    }

    struct GuidePolicySlot;

    impl PolicySlot for GuidePolicySlot {
        const NAME: &'static str = "external-guide-policy";
    }

    struct Part {
        handled: Rc<Cell<usize>>,
        duplicate_slot: bool,
    }

    impl EditPartBehavior<Model> for Part {
        fn create_figure(
            &mut self,
            _model: &Model,
            model_id: ModelId,
        ) -> Result<Box<dyn Figure>, EditPartError> {
            Ok(if model_id == ROOT {
                Box::new(RootFigure::new(0.0, 0.0, 200.0, 120.0))
            } else {
                Box::new(RectangleFigure::new(20.0, 20.0, 80.0, 50.0))
            })
        }

        fn create_policies(
            &mut self,
            _model: &Model,
            model_id: ModelId,
        ) -> Result<Vec<PolicyInstallation<Model>>, EditPartError> {
            if model_id == ROOT {
                return Ok(Vec::new());
            }
            let mut policies = vec![
                PolicyInstallation::new(
                    PolicyKey::<GuidePolicySlot>::new("external-guide-policy"),
                    GuidePolicy {
                        handled: Rc::clone(&self.handled),
                    },
                )
                .map_err(|error| EditPartError::operation(error.to_string()))?,
            ];
            if self.duplicate_slot {
                policies.push(
                    PolicyInstallation::new(
                        PolicyKey::<GuidePolicySlot>::new("external-guide-policy"),
                        GuidePolicy {
                            handled: Rc::clone(&self.handled),
                        },
                    )
                    .map_err(|error| EditPartError::operation(error.to_string()))?,
                );
            }
            Ok(policies)
        }
    }

    struct Factory {
        handled: Rc<Cell<usize>>,
        duplicate_slot: bool,
    }

    impl EditPartFactory<Model> for Factory {
        fn create(
            &mut self,
            _context: PartFactoryContext<ModelId>,
            _model: &Model,
        ) -> Result<Box<dyn EditPartBehavior<Model>>, EditPartError> {
            Ok(Box::new(Part {
                handled: Rc::clone(&self.handled),
                duplicate_slot: self.duplicate_slot,
            }))
        }
    }

    struct GuideHandle;

    impl HandleActionProtocol for GuideHandle {
        type Payload = f64;

        fn key() -> HandleActionKey<Self> {
            HandleActionKey::new("external-guide-handle")
        }
    }

    #[derive(Default)]
    struct ToolState {
        activated: usize,
        pressed: usize,
        cancelled: usize,
        contributions: Vec<f64>,
    }

    struct MarqueeTool {
        owner: EditPartId,
        coordinate: f64,
        state: Rc<RefCell<ToolState>>,
        panic_on_press: bool,
    }

    impl Tool<Model> for MarqueeTool {
        fn key(&self) -> novadraw_editor::ErasedToolKey {
            ToolKey::<Self>::new("external-marquee").erase()
        }

        fn activate(&mut self, _context: &mut ToolContext<'_, Model>) -> Result<(), ToolError> {
            self.state.borrow_mut().activated += 1;
            Ok(())
        }

        fn cancel(&mut self, _context: &mut ToolContext<'_, Model>) -> Result<(), ToolError> {
            self.state.borrow_mut().cancelled += 1;
            Ok(())
        }

        fn mouse_pressed(
            &mut self,
            context: &mut ToolContext<'_, Model>,
            event: ToolMouseEvent,
        ) -> Result<(ToolDirective, ViewerInputOutcome), ToolError> {
            assert!(!self.panic_on_press, "external Tool panic");
            let outcome = context.dispatch_pressed(event, false)?;
            let metadata = RequestMetadata::new(event.revision(), vec![self.owner]);
            let resolution = context.dispatch::<AlignToGuide>(
                &metadata,
                &AlignPayload {
                    coordinate: self.coordinate,
                },
            )?;
            let mut state = self.state.borrow_mut();
            state.pressed += 1;
            state.contributions = resolution.into_contributions();
            Ok((ToolDirective::Continue, outcome))
        }

        fn mouse_moved(
            &mut self,
            context: &mut ToolContext<'_, Model>,
            event: ToolMouseEvent,
        ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
            Ok((
                ToolDirective::Continue,
                context.dispatch_moved(event.location()),
            ))
        }

        fn mouse_released(
            &mut self,
            context: &mut ToolContext<'_, Model>,
            event: ToolMouseEvent,
        ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
            Ok((
                ToolDirective::Deactivate,
                context.dispatch_released(event.location(), event.button()),
            ))
        }
    }

    #[test]
    fn external_tool_request_policy_and_handle_share_the_open_protocol() {
        let handled = Rc::new(Cell::new(0));
        let tool_state = Rc::new(RefCell::new(ToolState::default()));
        let mut viewer = GraphicalViewer::new(
            Model,
            Factory {
                handled: Rc::clone(&handled),
                duplicate_slot: false,
            },
            Rectangle::new(0.0, 0.0, 200.0, 120.0),
        )
        .unwrap();
        let owner = viewer.part_for_model(NODE).unwrap();
        viewer
            .add_handle_visual_with_action::<GuideHandle>(
                owner,
                64.0,
                Box::new(RectangleFigure::new(30.0, 30.0, 16.0, 16.0)),
            )
            .unwrap();

        let mut domain = EditorDomain::new();
        let factory_state = Rc::clone(&tool_state);
        domain
            .register_handle_tool::<GuideHandle, MarqueeTool>(move |_context, owner, coordinate| {
                Ok(MarqueeTool {
                    owner,
                    coordinate: *coordinate,
                    state: Rc::clone(&factory_state),
                    panic_on_press: false,
                })
            })
            .unwrap();
        assert!(
            domain
                .register_handle_tool::<GuideHandle, MarqueeTool>(|_context, owner, coordinate| Ok(
                    MarqueeTool {
                        owner,
                        coordinate: *coordinate,
                        state: Rc::new(RefCell::new(ToolState::default())),
                        panic_on_press: false,
                    }
                ),)
                .is_err()
        );

        domain
            .pointer_pressed(
                &mut viewer,
                Point::new(34.0, 34.0),
                MouseButton::Left,
                KeyModifiers::default(),
            )
            .unwrap();
        domain
            .pointer_released(&mut viewer, Point::new(34.0, 34.0), MouseButton::Left)
            .unwrap();

        assert_eq!(handled.get(), 1);
        let state = tool_state.borrow();
        assert_eq!(state.activated, 1);
        assert_eq!(state.pressed, 1);
        assert_eq!(state.contributions, vec![64.0]);
        assert_eq!(
            domain.active_tool_key(),
            ToolKey::<novadraw_editor::SelectionTool>::new("selection").erase()
        );
        assert_eq!(
            InteractionRevision::initial().value(),
            1,
            "external consumer uses the same revision type"
        );
    }

    #[test]
    fn external_tool_panic_faults_the_viewer() {
        let handled = Rc::new(Cell::new(0));
        let state = Rc::new(RefCell::new(ToolState::default()));
        let mut viewer = GraphicalViewer::new(
            Model,
            Factory {
                handled,
                duplicate_slot: false,
            },
            Rectangle::new(0.0, 0.0, 200.0, 120.0),
        )
        .unwrap();
        let owner = viewer.part_for_model(NODE).unwrap();
        let mut domain = EditorDomain::new();
        domain
            .set_active_tool(
                &mut viewer,
                MarqueeTool {
                    owner,
                    coordinate: 0.0,
                    state,
                    panic_on_press: true,
                },
            )
            .unwrap();

        let panic = catch_unwind(AssertUnwindSafe(|| {
            domain.pointer_pressed(
                &mut viewer,
                Point::new(30.0, 30.0),
                MouseButton::Left,
                KeyModifiers::default(),
            )
        }));
        assert!(panic.is_err());
        assert!(viewer.is_faulted());
    }

    #[test]
    fn duplicate_external_policy_slot_is_rejected_before_activation() {
        let result = GraphicalViewer::new(
            Model,
            Factory {
                handled: Rc::new(Cell::new(0)),
                duplicate_slot: true,
            },
            Rectangle::new(0.0, 0.0, 200.0, 120.0),
        );
        assert!(result.is_err());
    }
}
