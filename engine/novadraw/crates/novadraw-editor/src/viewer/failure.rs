//! Cleanup of resources acquired by one creation call. No complete-sync rollback.

use super::*;

/// Actual cleanup progress after a failed creation or Overlay operation.
#[derive(Debug, Default)]
pub struct ProjectionCleanup {
    /// Original refusal or panic, retained independently of cleanup failures.
    pub original: String,
    /// Diagnostic from a nested failed scope, retained when its parent also cleans up.
    pub prior_cleanup: Option<Arc<ProjectionCleanup>>,
    /// Cleanup steps that failed; later independent steps may still have completed.
    pub failures: Vec<ProjectionCleanupFailure>,
    /// Acquired Figure identities still attached to the old Runtime.
    pub live_figures: Vec<FigureId>,
    /// Acquired Figure identities retired by engine commit.
    pub retired_figures: Vec<FigureId>,
    /// Acquired Part identities not yet retired from the old Viewer.
    pub live_parts: Vec<EditPartId>,
    /// Acquired Part identities retired by explicit Editor cleanup.
    pub retired_parts: Vec<EditPartId>,
    /// Acquired anchors whose explicit removal has not completed.
    pub live_anchors: Vec<AnchorId>,
    /// Acquired anchors removed by explicit cleanup.
    pub retired_anchors: Vec<AnchorId>,
    /// Whether cleanup stopped at the Core fault boundary.
    pub runtime_faulted: bool,
}

/// One unsuccessful cleanup attempt.
#[derive(Debug)]
pub struct ProjectionCleanupFailure {
    /// Domain step attempted exactly once.
    pub stage: &'static str,
    /// Figure involved, when the step belongs to Core disposal.
    pub figure: Option<FigureId>,
    /// Part involved, when the step belongs to Editor lifecycle.
    pub part: Option<EditPartId>,
    /// Recoverable rejection, preserving its original error category.
    pub error: Option<Box<ViewerError>>,
    /// A callback unwound instead of completing normally.
    pub panicked: bool,
}

pub(super) struct CreationBaseline {
    figures: HashSet<FigureId>,
    parts: HashSet<EditPartId>,
}

impl<A: ModelAdapter, F: EditPartFactory<A>> GraphicalViewer<A, F> {
    /// Reads cleanup diagnostics without driving the faulted Viewer or Runtime.
    pub fn last_projection_cleanup(&self) -> Option<&ProjectionCleanup> {
        self.last_cleanup.as_deref()
    }

    pub(super) fn creation_baseline(
        &self,
        parent_figure: FigureId,
        parent: EditPartId,
    ) -> CreationBaseline {
        CreationBaseline {
            figures: self
                .runtime
                .tree()
                .child_order(parent_figure)
                .unwrap_or_default()
                .into_iter()
                .collect(),
            parts: self
                .parts
                .children(parent)
                .unwrap_or_default()
                .iter()
                .copied()
                .chain(
                    self.parts
                        .connection_parts()
                        .iter()
                        .map(|id| id.edit_part()),
                )
                .collect(),
        }
    }

    pub(super) fn finish_creation<T>(
        &mut self,
        result: std::thread::Result<Result<T, ViewerError>>,
        parent_figure: FigureId,
        parent: EditPartId,
        before: CreationBaseline,
        anchors: &[AnchorId],
    ) -> Result<T, ViewerError> {
        if matches!(result, Ok(Ok(_))) {
            return result.expect("successful creation");
        }
        let roots: Vec<_> = self
            .runtime
            .tree()
            .child_order(parent_figure)
            .unwrap_or_default()
            .into_iter()
            .filter(|id| !before.figures.contains(id))
            .collect();
        let parts: Vec<_> = self
            .parts
            .children(parent)
            .unwrap_or_default()
            .iter()
            .copied()
            .chain(
                self.parts
                    .connection_parts()
                    .iter()
                    .map(|id| id.edit_part()),
            )
            .filter(|id| !before.parts.contains(id))
            .collect();
        let mut report = ProjectionCleanup {
            original: match &result {
                Ok(Err(error)) => error.to_string(),
                Err(payload) => panic_message(payload.as_ref()),
                _ => unreachable!(),
            },
            prior_cleanup: match &result {
                Ok(Err(ViewerError::ProjectionCleanup { report, .. })) => Some(Arc::clone(report)),
                _ => None,
            },
            live_anchors: anchors.to_vec(),
            ..Default::default()
        };
        let figures = self.collect_creation_figures(&roots);
        let acquired_parts: Vec<_> = parts
            .iter()
            .flat_map(|id| self.parts.subtree_ids(*id).unwrap_or_else(|_| vec![*id]))
            .collect();
        for part in parts {
            self.cleanup_created_part(part, &mut report);
        }
        for anchor in anchors.iter().copied() {
            if self.runtime.is_faulted() {
                break;
            }
            if self.cleanup_attempt("anchor removal", None, None, &mut report, |viewer| {
                viewer.runtime.connections().remove_anchor(anchor)?;
                Ok(())
            }) {
                report.live_anchors.retain(|id| *id != anchor);
                report.retired_anchors.push(anchor);
            }
        }
        self.cleanup_created_figures(&roots, &mut report);
        self.record_figure_progress(&figures, &mut report);
        for part in acquired_parts {
            if self.parts.get(part).is_some() {
                report.live_parts.push(part);
            } else {
                report.retired_parts.push(part);
            }
        }
        self.faulted = true;
        self.drop_direct_text_edit_into(&mut report);
        let report = Arc::new(report);
        self.last_cleanup = Some(Arc::clone(&report));
        match result {
            Ok(Err(original)) if report.failures.is_empty() && !report.runtime_faulted => {
                Err(original)
            }
            Ok(Err(original)) => Err(ViewerError::ProjectionCleanup {
                original: Box::new(original),
                report,
            }),
            Err(payload) => resume_unwind(payload),
            Ok(Ok(_)) => unreachable!(),
        }
    }

    pub(super) fn collect_creation_figures(&self, roots: &[FigureId]) -> Vec<FigureId> {
        let mut figures = roots.to_vec();
        let mut index = 0;
        while index < figures.len() {
            let children = self
                .runtime
                .tree()
                .child_order(figures[index])
                .unwrap_or_default();
            figures.extend(children);
            index += 1;
        }
        figures
    }

    fn cleanup_created_part(&mut self, root: EditPartId, report: &mut ProjectionCleanup) {
        if self.runtime.is_faulted() {
            return;
        }
        let ids = if self
            .parts
            .get(root)
            .is_some_and(|part| part.kind() == PartKind::Connection)
        {
            vec![root]
        } else {
            let Ok(ids) = self.parts.subtree_ids(root) else {
                return;
            };
            ids
        };
        self.selection.reconcile(|id| !ids.contains(&id));
        // Overlay belongs to the Part; keep the normal removal order.
        let mut overlays: Vec<_> = self
            .visual_registry
            .iter()
            .filter_map(|(figure, owner)| {
                (owner.is_overlay() && owner.owner_part().is_some_and(|id| ids.contains(&id)))
                    .then_some(*figure)
            })
            .collect();
        overlays.sort();
        let overlay_figures = self.collect_creation_figures(&overlays);
        self.cleanup_created_figures(&overlays, report);
        self.record_figure_progress(&overlay_figures, report);
        for id in ids.iter().copied() {
            if self.runtime.is_faulted() {
                return;
            }
            let Some(node) = self.parts.get(id) else {
                continue;
            };
            let Some(model_id) = node.model_id() else {
                continue;
            };
            let parent_model = self
                .parts
                .parent(id)
                .and_then(|parent| self.parts.get(parent))
                .and_then(|parent| parent.model_id());
            let host = PolicyHost::new(id, model_id, parent_model);
            let _ = self.parts.set_active(id, false);
            // Each policy records attempted deactivation before calling the extension.
            if let Some(entries) = self.policies.entries_mut(id) {
                for policy in entries.values_mut() {
                    if catch_unwind(AssertUnwindSafe(|| policy.deactivate(host, &self.model)))
                        .is_err()
                    {
                        report.failures.push(ProjectionCleanupFailure {
                            stage: "policy deactivation",
                            part: Some(id),
                            figure: None,
                            error: None,
                            panicked: true,
                        });
                    }
                }
            }
            if self.active_behaviors.remove(&id) {
                self.cleanup_attempt("behavior deactivation", None, Some(id), report, |viewer| {
                    if let Some(behavior) = viewer.behaviors.get_mut(id) {
                        behavior.deactivate(&viewer.model, model_id);
                    }
                    Ok(())
                });
            }
            if let Some(projection) = self.connection_projections.remove(&model_id) {
                if projection.registered {
                    self.cleanup_attempt(
                        "connection binding removal",
                        None,
                        Some(id),
                        report,
                        |viewer| {
                            viewer
                                .runtime
                                .connections()
                                .remove_state(projection.connection)?;
                            Ok(())
                        },
                    );
                }
                let _ = self.parts.unbind_connection(projection.part);
            }
            self.model_registry.remove(&model_id);
            self.visual_registry
                .retain(|_, owner| owner.owner_part() != Some(id));
        }
        for id in ids.iter().rev().copied() {
            self.policies.remove(id);
            self.behaviors.remove(id);
        }
        // Figure retirement is performed once by the outer scope after bindings are removed.
        if let Some(connection) = self
            .parts
            .connection_parts()
            .iter()
            .copied()
            .find(|connection| connection.edit_part() == root)
        {
            let _ = self.parts.retire_connection(connection);
        } else {
            let _ = self.parts.retire_subtree(root);
        }
    }

    pub(super) fn cleanup_attempt(
        &mut self,
        stage: &'static str,
        figure: Option<FigureId>,
        part: Option<EditPartId>,
        report: &mut ProjectionCleanup,
        operation: impl FnOnce(&mut Self) -> Result<(), ViewerError>,
    ) -> bool {
        let failure = match catch_unwind(AssertUnwindSafe(|| operation(self))) {
            Ok(Ok(())) => return true,
            Ok(Err(error)) => (Some(Box::new(error)), false),
            Err(_) => (None, true),
        };
        report.failures.push(ProjectionCleanupFailure {
            stage,
            figure,
            part,
            error: failure.0,
            panicked: failure.1,
        });
        false
    }

    pub(super) fn cleanup_created_figures(
        &mut self,
        roots: &[FigureId],
        report: &mut ProjectionCleanup,
    ) {
        for root in roots.iter().copied() {
            if self.runtime.is_faulted() {
                break;
            }
            if self.runtime.tree().is_attached(root) != Ok(true) {
                continue;
            }
            self.cleanup_attempt("Figure disposal", Some(root), None, report, |viewer| {
                viewer.runtime.dispose_subtree(root)?;
                Ok(())
            });
        }
        self.visual_registry
            .retain(|figure, _| self.runtime.tree().is_attached(*figure) == Ok(true));
    }

    pub(super) fn record_figure_progress(
        &self,
        figures: &[FigureId],
        report: &mut ProjectionCleanup,
    ) {
        for figure in figures.iter().copied() {
            if self.runtime.tree().is_attached(figure) == Ok(true) {
                report.live_figures.push(figure);
            } else {
                report.retired_figures.push(figure);
            }
        }
        report.runtime_faulted = self.runtime.is_faulted();
    }
}

pub(super) fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|value| (*value).to_owned())
        })
        .unwrap_or_else(|| "extension panic".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EditPartBehavior;
    use novadraw::figure::{
        CONTAINER, ContainerCapability, FigureCapabilityBuilder, FigureCapabilityRegistrationError,
        FigureContainer, FigureLifecycle, FigureLifecycleContext, LIFECYCLE, LifecycleCapability,
    };
    use std::{
        convert::Infallible,
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    struct Model {
        revision: ModelRevision,
        events: Vec<crate::ModelEvent<u8, ()>>,
        added: bool,
        deep: bool,
    }
    impl ModelAdapter for Model {
        type ModelId = u8;
        type Event = ();
        type Error = Infallible;
        fn root(&self) -> u8 {
            0
        }
        fn revision(&self) -> ModelRevision {
            self.revision
        }
        fn children(&self, id: u8) -> Result<Vec<u8>, Infallible> {
            Ok(if id == 0 && self.added {
                vec![1]
            } else if id == 1 && self.deep {
                vec![2]
            } else {
                vec![]
            })
        }
        fn drain_events(&mut self) -> Vec<crate::ModelEvent<u8, ()>> {
            std::mem::take(&mut self.events)
        }
    }
    #[derive(Clone, Copy)]
    enum Failure {
        Configure,
        ConfigurePanic,
        Refresh,
        Activate,
        AttachPanic,
        DetachPanic,
        PolicyDeactivatePanic,
    }
    struct Probe {
        failure: Failure,
        detached: Arc<AtomicUsize>,
    }
    impl Figure for Probe {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 10.0, 10.0)
        }
        fn name(&self) -> &'static str {
            "S06 probe"
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(LIFECYCLE, LifecycleCapability::of::<Self>())?;
            out.register(CONTAINER, ContainerCapability::of::<Self>())
        }
    }
    impl FigureContainer for Probe {}
    impl FigureLifecycle for Probe {
        fn on_attached(&mut self, _: FigureLifecycleContext) {
            if matches!(self.failure, Failure::AttachPanic) {
                panic!("attach probe");
            }
        }
        fn on_detached(&mut self, _: FigureLifecycleContext) {
            self.detached.fetch_add(1, Ordering::SeqCst);
            if matches!(self.failure, Failure::DetachPanic) {
                panic!("detach probe");
            }
        }
    }
    struct Factory {
        failure: Failure,
        detached: Arc<AtomicUsize>,
        calls: Arc<Mutex<Vec<&'static str>>>,
    }
    struct Part {
        failure: Failure,
        detached: Arc<AtomicUsize>,
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    struct CleanupPolicy(Arc<Mutex<Vec<&'static str>>>);
    struct CleanupSlot;
    impl crate::PolicySlot for CleanupSlot {
        const NAME: &'static str = "cleanup-probe";
    }
    impl crate::EditPolicy<Model> for CleanupPolicy {
        fn register_handlers(_: &mut crate::PolicyHandlerBuilder<'_, Model, Self>) {}
        fn activate(&mut self, _: PolicyHost<u8>, _: &Model) -> Result<(), crate::PolicyError> {
            self.0.lock().unwrap().push("policy activate");
            Ok(())
        }
        fn deactivate(&mut self, _: PolicyHost<u8>, _: &Model) {
            self.0.lock().unwrap().push("policy deactivate");
            panic!("policy cleanup panic");
        }
    }
    impl EditPartFactory<Model> for Factory {
        fn create(
            &mut self,
            context: PartFactoryContext<u8>,
            _: &Model,
        ) -> Result<Box<dyn EditPartBehavior<Model>>, EditPartError> {
            if context.model_id() == 2 {
                return Err(EditPartError::operation("descendant factory refused"));
            }
            Ok(Box::new(Part {
                failure: self.failure,
                detached: Arc::clone(&self.detached),
                calls: Arc::clone(&self.calls),
            }))
        }
    }
    impl EditPartBehavior<Model> for Part {
        fn create_policies(
            &mut self,
            _: &Model,
            id: u8,
        ) -> Result<Vec<crate::PolicyInstallation<Model>>, EditPartError> {
            if id == 1 && matches!(self.failure, Failure::PolicyDeactivatePanic) {
                Ok(vec![
                    crate::PolicyInstallation::new(
                        crate::PolicyKey::<CleanupSlot>::new("cleanup-probe"),
                        CleanupPolicy(Arc::clone(&self.calls)),
                    )
                    .map_err(|error| EditPartError::operation(error.to_string()))?,
                ])
            } else {
                Ok(vec![])
            }
        }
        fn create_figure(&mut self, _: &Model, id: u8) -> Result<Box<dyn Figure>, EditPartError> {
            Ok(if id == 1 {
                Box::new(Probe {
                    failure: self.failure,
                    detached: Arc::clone(&self.detached),
                })
            } else {
                Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap())
            })
        }
        fn configure_visual(
            &mut self,
            _: &Model,
            id: u8,
            ctx: &mut VisualBuildContext<'_>,
        ) -> Result<(), EditPartError> {
            if id != 1 {
                return Ok(());
            }
            ctx.add_child(
                ctx.primary(),
                Box::new(RectangleFigure::new(0.0, 0.0, 2.0, 2.0).unwrap()),
            )?;
            match self.failure {
                Failure::Configure | Failure::DetachPanic => {
                    Err(EditPartError::operation("configure refused"))
                }
                Failure::ConfigurePanic => panic!("configure probe"),
                _ => Ok(()),
            }
        }
        fn refresh_visuals(
            &mut self,
            _: &Model,
            id: u8,
            _: &mut VisualUpdateContext<'_>,
        ) -> Result<(), EditPartError> {
            if id == 1 && matches!(self.failure, Failure::Refresh) {
                return Err(EditPartError::operation("refresh refused"));
            }
            Ok(())
        }
        fn activate(&mut self, _: &Model, id: u8) -> Result<(), EditPartError> {
            if id == 1 && matches!(self.failure, Failure::Activate) {
                return Err(EditPartError::operation("activation refused"));
            }
            if id == 1 {
                self.calls.lock().unwrap().push("activate");
            }
            Ok(())
        }
        fn deactivate(&mut self, _: &Model, id: u8) {
            if id == 1 {
                self.calls.lock().unwrap().push("deactivate");
            }
        }
    }
    fn viewer(failure: Failure) -> GraphicalViewer<Model, Factory> {
        GraphicalViewer::new(
            Model {
                revision: ModelRevision::initial(),
                events: vec![],
                added: false,
                deep: matches!(failure, Failure::PolicyDeactivatePanic),
            },
            Factory {
                failure,
                detached: Arc::new(AtomicUsize::new(0)),
                calls: Arc::new(Mutex::new(vec![])),
            },
            Rectangle::new(0.0, 0.0, 100.0, 100.0),
        )
        .unwrap()
    }
    fn add(viewer: &mut GraphicalViewer<Model, Factory>) -> Result<bool, ViewerError> {
        viewer.synchronize_model(|model| {
            model.added = true;
            model.revision = model.revision.next().unwrap();
            model
                .events
                .push(crate::ModelEvent::new(model.revision, 0, ()));
        })
    }
    #[test]
    fn s06_creation_failures_clear_all_acquired_registries() {
        for failure in [
            Failure::Configure,
            Failure::ConfigurePanic,
            Failure::Refresh,
            Failure::Activate,
        ] {
            let mut viewer = viewer(failure);
            let detached = Arc::clone(&viewer.factory.detached);
            let revision = viewer.applied_revision();
            let result = catch_unwind(AssertUnwindSafe(|| add(&mut viewer)));
            assert!(!matches!(result, Ok(Ok(_))));
            assert!(viewer.is_faulted());
            assert!(!viewer.runtime.is_faulted());
            assert_eq!(viewer.applied_revision(), revision);
            assert!(viewer.part_for_model(1).is_none());
            let report = viewer.last_projection_cleanup().unwrap();
            assert!(report.live_figures.is_empty());
            assert_eq!(report.retired_figures.len(), 2);
            assert!(
                report
                    .retired_figures
                    .iter()
                    .all(|id| !viewer.visual_registry.contains_key(id))
            );
            assert_eq!(detached.load(Ordering::SeqCst), 1);
            drop(viewer);
            assert_eq!(detached.load(Ordering::SeqCst), 1);
        }
    }
    #[test]
    fn s06_cleanup_panic_reports_retired_structure_and_never_repeats_callback() {
        let mut viewer = viewer(Failure::DetachPanic);
        let detached = Arc::clone(&viewer.factory.detached);
        let error = add(&mut viewer).unwrap_err();
        let ViewerError::ProjectionCleanup { original, report } = error else {
            panic!("cleanup diagnostic");
        };
        assert!(original.to_string().contains("configure refused"));
        assert!(report.runtime_faulted);
        assert!(report.live_figures.is_empty());
        assert_eq!(report.retired_figures.len(), 2);
        assert!(
            report
                .failures
                .iter()
                .any(|failure| failure.stage == "Figure disposal" && failure.panicked)
        );
        assert!(viewer.synchronize().is_err());
        assert!(
            viewer
                .add_feedback_visual(
                    None,
                    false,
                    Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap())
                )
                .is_err()
        );
        drop(viewer);
        assert_eq!(detached.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn s06_core_attach_panic_preserves_live_identity_for_isolated_exit() {
        let mut viewer = viewer(Failure::AttachPanic);
        assert!(catch_unwind(AssertUnwindSafe(|| add(&mut viewer))).is_err());
        let report = viewer.last_projection_cleanup().unwrap();
        assert!(report.runtime_faulted);
        assert_eq!(report.live_figures.len(), 1);
        assert!(report.retired_figures.is_empty());
        let detached = Arc::clone(&viewer.factory.detached);
        drop(viewer);
        assert_eq!(detached.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn s06_policy_cleanup_panic_preserves_original_error_and_continues_independent_cleanup() {
        let mut viewer = viewer(Failure::PolicyDeactivatePanic);
        let calls = Arc::clone(&viewer.factory.calls);
        let error = add(&mut viewer).unwrap_err();
        let ViewerError::ProjectionCleanup { original, report } = error else {
            panic!("cleanup report");
        };
        assert!(original.to_string().contains("descendant factory refused"));
        assert!(!report.runtime_faulted);
        assert!(report.live_figures.is_empty());
        assert!(report.live_parts.is_empty());
        assert!(
            report
                .failures
                .iter()
                .any(|failure| failure.stage == "policy deactivation" && failure.panicked)
        );
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                "activate",
                "policy activate",
                "policy deactivate",
                "deactivate"
            ]
        );
        drop(viewer);
        assert_eq!(
            calls
                .lock()
                .unwrap()
                .iter()
                .filter(|call| **call == "policy deactivate")
                .count(),
            1
        );
    }

    #[test]
    fn s06_fault_close_preserves_original_panic_when_text_feedback_cleanup_panics() {
        struct Plan(crate::DirectTextEditDescriptor);
        impl crate::DirectTextEdit<Model> for Plan {
            fn descriptor(&self) -> &crate::DirectTextEditDescriptor {
                &self.0
            }
            fn feedback(
                &mut self,
                _: &DirectTextEditState,
                _: &Model,
            ) -> Result<DirectTextFeedback, PolicyError> {
                Err(PolicyError::operation("unused feedback"))
            }
            fn command(
                &mut self,
                _: &DirectTextEditState,
                _: &Model,
            ) -> Result<Box<dyn crate::Command<Model>>, PolicyError> {
                Err(PolicyError::operation("unused command"))
            }
        }
        let mut viewer = viewer(Failure::ConfigurePanic);
        let detached = Arc::new(AtomicUsize::new(0));
        let (_, feedback) = viewer
            .add_feedback_visual(
                None,
                false,
                Box::new(Probe {
                    failure: Failure::DetachPanic,
                    detached: Arc::clone(&detached),
                }),
            )
            .unwrap();
        let descriptor = crate::DirectTextEditDescriptor::new(
            crate::DirectTextFeature::new("text-probe").unwrap(),
            "",
            ModelRevision::initial(),
            crate::TextEditMode::SingleLine,
            crate::FocusLossPolicy::Cancel,
        );
        let session = DirectTextEditSessionId::new(viewer.namespace(), 1);
        viewer.direct_text_edit = Some(ActiveDirectTextEdit {
            state: DirectTextEditState::new(session, viewer.contents(), &descriptor),
            plan: Box::new(Plan(descriptor)),
            feedback: vec![feedback],
            text_feedback: feedback,
            horizontal_scroll: 0.0,
            caret_feedback: None,
            caret_visible: false,
        });
        let panic = catch_unwind(AssertUnwindSafe(|| add(&mut viewer))).unwrap_err();
        assert_eq!(panic_message(panic.as_ref()), "configure probe");
        let report = viewer.last_projection_cleanup().unwrap();
        assert_eq!(report.original, "configure probe");
        assert!(report.runtime_faulted);
        assert!(report.retired_figures.contains(&feedback));
        assert!(viewer.direct_text_edit().is_none());
        assert!(
            matches!(viewer.take_text_input_effects().last(), Some(TextInputEffect::Release { session: released }) if *released == session)
        );
        drop(viewer);
        assert_eq!(detached.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn s06_overlay_topology_is_checked_before_any_disposal() {
        let mut viewer = viewer(Failure::Configure);
        let (_, parent) = viewer
            .add_feedback_visual(
                None,
                false,
                Box::new(RectangleFigure::new(0.0, 0.0, 5.0, 5.0).unwrap()),
            )
            .unwrap();
        let (_, child) = viewer
            .add_feedback_visual(
                None,
                false,
                Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap()),
            )
            .unwrap();
        viewer
            .runtime
            .figure(child)
            .unwrap()
            .reparent(parent)
            .unwrap();
        for batch in [[parent, child], [child, parent]] {
            assert!(viewer.remove_overlay_visuals(&batch).is_err());
            assert!(viewer.runtime.tree().is_attached(parent).unwrap());
            assert!(viewer.runtime.tree().is_attached(child).unwrap());
            assert!(!viewer.is_faulted());
        }
        let part = viewer.contents();
        viewer.visual_registry.insert(
            child,
            VisualOwner::feedback(FeedbackId::new(viewer.namespace()), Some(part)),
        );
        assert!(viewer.remove_overlay_visuals(&[parent]).is_err());
        assert!(viewer.runtime.tree().is_attached(parent).unwrap());
    }
    #[test]
    fn s06_feedback_batch_preflights_both_layers_and_styles() {
        let mut viewer = viewer(Failure::Configure);
        let owner = viewer.contents();
        let scaled = viewer.root_layers.scaled_feedback();
        let unscaled = viewer.root_layers.feedback();
        let before = [
            viewer.runtime.tree().child_order(scaled),
            viewer.runtime.tree().child_order(unscaled),
        ];
        struct InvalidBounds(Rectangle);
        impl Figure for InvalidBounds {
            fn initial_bounds(&self) -> Rectangle {
                self.0
            }
            fn name(&self) -> &'static str {
                "InvalidBounds"
            }
        }
        for invalid in [
            FeedbackVisual::unscaled(Box::new(InvalidBounds(Rectangle::new(0.0, 0.0, -1.0, 5.0)))),
            FeedbackVisual::unscaled(Box::new(InvalidBounds(Rectangle::new(
                0.0,
                0.0,
                f64::NAN,
                5.0,
            ))))
            .with_style(FigureStyle {
                alpha: Some(novadraw::graphics::Opacity::OPAQUE),
                ..Default::default()
            }),
        ] {
            let error = viewer
                .add_feedback_batch(vec![
                    (
                        Some(owner),
                        FeedbackVisual::scaled(Box::new(
                            RectangleFigure::new(0.0, 0.0, 5.0, 5.0).unwrap(),
                        )),
                    ),
                    (Some(owner), invalid),
                ])
                .unwrap_err();
            assert!(error.to_string().contains("input 1"));
            assert_eq!(viewer.runtime.tree().child_order(scaled), before[0]);
            assert_eq!(viewer.runtime.tree().child_order(unscaled), before[1]);
            assert!(!viewer.is_faulted());
            assert!(viewer.last_projection_cleanup().is_none());
        }
        assert!(viewer.add_feedback_batch(vec![]).unwrap().is_empty());
    }
    #[test]
    fn s06_overlay_installation_panic_reports_the_attached_prefix_without_publishing_owners() {
        let mut viewer = viewer(Failure::Configure);
        let detached = Arc::new(AtomicUsize::new(0));
        let result = catch_unwind(AssertUnwindSafe(|| {
            viewer.add_feedback_batch(vec![
                (
                    None,
                    FeedbackVisual::unscaled(Box::new(
                        RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap(),
                    )),
                ),
                (
                    None,
                    FeedbackVisual::scaled(Box::new(Probe {
                        failure: Failure::AttachPanic,
                        detached: Arc::clone(&detached),
                    })),
                ),
            ])
        }));
        assert!(result.is_err());
        let report = viewer.last_projection_cleanup().unwrap();
        assert!(report.runtime_faulted);
        assert_eq!(report.live_figures.len(), 2);
        assert!(report.retired_figures.is_empty());
        assert!(
            report
                .live_figures
                .iter()
                .all(|figure| !viewer.visual_registry.contains_key(figure))
        );
        drop(viewer);
        assert_eq!(detached.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn s06_overlay_late_lifecycle_panic_records_retired_prefix_and_item() {
        let mut viewer = viewer(Failure::Configure);
        let detached = Arc::new(AtomicUsize::new(0));
        let (_, first) = viewer
            .add_feedback_visual(
                None,
                false,
                Box::new(Probe {
                    failure: Failure::Configure,
                    detached: Arc::clone(&detached),
                }),
            )
            .unwrap();
        let (_, second) = viewer
            .add_feedback_visual(
                None,
                false,
                Box::new(Probe {
                    failure: Failure::DetachPanic,
                    detached: Arc::clone(&detached),
                }),
            )
            .unwrap();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || viewer.remove_overlay_visuals(&[first, second])
            ))
            .is_err()
        );
        let report = viewer.last_projection_cleanup().unwrap();
        assert!(report.runtime_faulted);
        assert!(report.live_figures.is_empty());
        assert_eq!(report.retired_figures, [first, second]);
        assert!(!viewer.visual_registry.contains_key(&first));
        assert!(!viewer.visual_registry.contains_key(&second));
        drop(viewer);
        assert_eq!(detached.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn s06_cleanup_driver_keeps_real_refusal_and_continues_independent_roots() {
        // Dispatcher evidence only: synthetic-root refusal is publicly reachable,
        // but a synthetic root is never acquired by Part/Connection creation.
        // This is not the revision-exhaustion integration case P04a/C05c/O07a.
        let mut viewer = viewer(Failure::Configure);
        let detached = Arc::new(AtomicUsize::new(0));
        let mut roots = Vec::new();
        for _ in 0..2 {
            let (_, figure) = viewer
                .add_feedback_visual(
                    None,
                    false,
                    Box::new(Probe {
                        failure: Failure::Configure,
                        detached: Arc::clone(&detached),
                    }),
                )
                .unwrap();
            roots.push(figure);
        }
        let mut synthetic_root = viewer.root_layers().scaled_feedback();
        while let Some(parent) = viewer.runtime.tree().parent_id(synthetic_root).unwrap() {
            synthetic_root = parent;
        }
        let original = add(&mut viewer).unwrap_err().to_string();
        let mut report = ProjectionCleanup {
            original: original.clone(),
            ..ProjectionCleanup::default()
        };
        viewer.cleanup_created_figures(&[roots[0], synthetic_root, roots[1]], &mut report);
        viewer.record_figure_progress(&[roots[0], synthetic_root, roots[1]], &mut report);
        assert_eq!(report.original, original);
        assert_eq!(report.failures.len(), 1);
        let failure = &report.failures[0];
        assert_eq!(failure.stage, "Figure disposal");
        assert_eq!(failure.figure, Some(synthetic_root));
        assert!(!failure.panicked);
        assert!(matches!(
            failure.error.as_deref(),
            Some(ViewerError::Runtime(
                RuntimeMutationError::SyntheticRootOperation(id)
            )) if *id == synthetic_root
        ));
        assert!(!report.runtime_faulted);
        assert_eq!(report.live_figures, [synthetic_root]);
        assert_eq!(report.retired_figures, roots);
        assert_eq!(detached.load(Ordering::SeqCst), 2);
        drop(viewer);
        assert_eq!(detached.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn s06_faulted_viewer_only_completes_an_existing_healthy_runtime_submission() {
        use novadraw::render::{DamageMode, RenderDisposition, ResourceSync};
        let surface = SurfaceInfo {
            logical_width: 100.0,
            logical_height: 100.0,
            pixel_width: 100,
            pixel_height: 100,
            scale_factor: 1.0,
        };
        for failure in [Failure::Configure, Failure::AttachPanic] {
            let mut old = viewer(failure);
            let old_part = old.contents();
            let old_figure = old.parts.get(old_part).unwrap().primary_figure();
            let FramePreparation::Ready(submission) = old
                .drive()
                .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
            else {
                panic!("initial submission");
            };
            let applied = old.applied_revision();
            let _ = catch_unwind(AssertUnwindSafe(|| add(&mut old)));
            assert!(matches!(
                old.drive()
                    .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL),
                FramePreparation::Error(FramePreparationError::Faulted)
            ));
            assert!(old.drive().advance_time(MonotonicTime::ZERO).is_err());
            assert!(old.drive().resize_logical_viewport(80.0, 80.0).is_err());
            let completion = old.drive().complete_submission(
                submission.session_id,
                submission.frame_id,
                Ok(RenderDisposition::Presented),
            );
            if matches!(failure, Failure::Configure) {
                assert!(completion.is_ok());
            } else {
                assert!(completion.is_err());
            }
            assert_eq!(old.applied_revision(), applied);
            drop(old);
            let mut rebuilt = viewer(Failure::Configure);
            assert_ne!(rebuilt.contents(), old_part);
            assert_eq!(
                rebuilt.runtime.tree().is_attached(old_figure),
                Err(novadraw::tree::TreeQueryError::ForeignFigure(old_figure))
            );
            let FramePreparation::Ready(fresh) = rebuilt
                .drive()
                .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
            else {
                panic!("new baseline");
            };
            assert!(matches!(fresh.resources, ResourceSync::Snapshot(_)));
            assert_eq!(fresh.damage.mode(), DamageMode::Full);
            assert!(
                rebuilt
                    .drive()
                    .complete_submission(
                        submission.session_id,
                        submission.frame_id,
                        Ok(RenderDisposition::Presented)
                    )
                    .is_err()
            );
            let _ = rebuilt
                .drive()
                .complete_submission(
                    fresh.session_id,
                    fresh.frame_id,
                    Ok(RenderDisposition::Presented),
                )
                .unwrap();
        }
    }
}
