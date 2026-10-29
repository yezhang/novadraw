use std::{collections::HashSet, sync::Arc};

use super::Runtime;
use crate::{
    geometry::Rectangle,
    render::{CommandRecorder, RenderCommandKind, ResourcePayload},
    scene::{
        CaptureConfigRevision, SceneCaptureInput, SceneCapturePolicy, SceneCapturePolicyKey,
        SceneCloseReason, SceneError, SceneHandoff, SceneSnapshot, SceneSourceId,
        SceneSubscriptionId,
    },
    tree::FigureId,
};

/// Runtime-scoped capture service. Capturing never prepares or acknowledges a surface frame.
pub struct RuntimeScenes<'a> {
    runtime: &'a mut Runtime,
}

impl Runtime {
    /// Borrows the immutable scene capture service.
    pub fn scenes(&mut self) -> RuntimeScenes<'_> {
        RuntimeScenes { runtime: self }
    }

    fn scene_capture_inputs(&self) -> Result<crate::scene::CaptureInputs, SceneError> {
        Ok((
            self.tree
                .notification_input_serial()
                .ok_or(SceneError::Exhausted)?,
            self.updates
                .notification_input_serial()
                .ok_or(SceneError::Exhausted)?,
            self.activity_now,
            self.text.revision(),
        ))
    }

    fn stabilize_for_capture(&mut self) -> Result<(), SceneError> {
        let inputs = self.scene_capture_inputs()?;
        if self.scenes.stable_inputs != Some(inputs)
            || !self.mutations.is_empty()
            || self.updates.has_pending_layout()
            || !self.connections.dirty_connections().is_empty()
            || self.derivation_epoch != self.stable_epoch
        {
            self.scenes.stable_inputs = None;
            self.stabilize_for_query()
                .map_err(SceneError::Stabilization)?;
            self.scenes.stable_inputs = Some(self.scene_capture_inputs()?);
        }
        Ok(())
    }

    fn capture_subscription_inner(
        &mut self,
        id: SceneSubscriptionId,
    ) -> Result<Arc<SceneSnapshot>, SceneError> {
        let subscription = self
            .scenes
            .subscriptions
            .get(&id)
            .ok_or(SceneError::UnknownSubscription)?;
        let root = *self
            .scenes
            .sources
            .get(&subscription.source)
            .ok_or(SceneError::UnknownSource)?;
        if !self.tree.is_attached(root).map_err(SceneError::Tree)? {
            return Err(SceneError::UnknownSource);
        }
        let bounds = self.tree.figure_bounds(root).map_err(SceneError::Tree)?;
        let style = self.tree.resolved_style(root).map_err(SceneError::Tree)?;
        let rectangle = self.scenes.rectangle(
            subscription,
            SceneCaptureInput {
                border_box: Rectangle::new(0.0, 0.0, bounds.width, bounds.height),
                style: &style,
            },
        )?;
        validate_capture_rectangle(rectangle)?;
        let font = style.font.clone();
        let mut recorder = CommandRecorder::new();
        if !rectangle.is_empty() {
            self.tree
                .capture_to(&mut recorder.canvas, root, rectangle, &style, font);
        }
        recorder
            .canvas
            .validate_recording(&self.resources)
            .map_err(SceneError::Graphics)?;
        crate::render::validate_graphics_input(recorder.commands(), 1.0)
            .map_err(SceneError::InvalidGraphicsInput)?;

        // Only immutable, exact revisions referenced by this drawing enter the closure.
        // This does not touch ResourceRegistry's pending synchronization prefix.
        let mut seen = HashSet::new();
        for command in recorder.canvas.commands() {
            let (id, revision, image) = match &command.kind {
                RenderCommandKind::Image { image, .. } => {
                    (image.resource_id(), image.revision(), Some(*image))
                }
                RenderCommandKind::DrawGlyphRun { run, .. } => {
                    (run.font.resource_id(), run.font.revision(), None)
                }
                _ => continue,
            };
            let resource = self
                .resources
                .snapshot_resource(id)
                .map_err(SceneError::Resource)?;
            if resource.revision != revision {
                return Err(SceneError::ResourceMismatch);
            }
            match (&resource.payload, image) {
                (ResourcePayload::Image(data), Some(image))
                    if data.width == image.width()
                        && data.height == image.height()
                        && data.scale == image.scale() => {}
                (ResourcePayload::Font(_), None) => {}
                _ => return Err(SceneError::ResourceMismatch),
            }
            if seen.insert((id, revision)) {
                recorder.leases.push(resource);
            }
        }
        Ok(Arc::new(SceneSnapshot {
            dependencies: self
                .tree
                .scene_group
                .as_ref()
                .map(|group| group.evidence(subscription.source))
                .transpose()
                .map_err(SceneError::Admission)?,
            source: subscription.source,
            subscription: id,
            config: subscription.config,
            stable_epoch: self.stable_epoch,
            rectangle,
            style,
            drawing: recorder.finish().map_err(SceneError::Graphics)?,
        }))
    }
}

impl RuntimeScenes<'_> {
    /// Registers an external display policy through the default registry.
    pub fn register_view_policy<P: Send + Sync + 'static>(
        &mut self,
        key: crate::scene::SceneViewPolicyKey<P>,
        policy: impl crate::scene::SceneViewPolicy<P>,
    ) -> Result<(), SceneError> {
        self.check_runtime()?;
        self.runtime
            .guarded(|runtime| runtime.scenes.register_view_policy(key, policy))
    }

    /// Creates an owned read-only consumer, moved into a default or external Figure.
    pub fn create_view<P: Send + Sync + 'static>(
        &mut self,
        key: crate::scene::SceneViewPolicyKey<P>,
        payload: P,
    ) -> Result<crate::scene::SceneViewHandle, SceneError> {
        self.check_runtime()?;
        let policy = self
            .runtime
            .scenes
            .view_policies
            .get(&key.0.erased())
            .ok_or(SceneError::UnknownPolicy)?
            .clone();
        Ok(crate::scene::SceneViewHandle {
            namespace: self.runtime.tree.namespace(),
            state: Arc::new(std::sync::Mutex::new(crate::scene::view::ViewState {
                revision: 0,
                policy,
                payload: Arc::new(payload),
                installed: None,
            })),
        })
    }

    /// Binds a registered Figure consumer before accepting its first snapshot.
    pub fn bind_view(
        &mut self,
        figure: FigureId,
        source: SceneSourceId,
        subscription: SceneSubscriptionId,
        config: CaptureConfigRevision,
    ) -> Result<crate::scene::SceneViewBindingId, SceneError> {
        self.check_runtime()?;
        if source.namespace != subscription.namespace || config.get() == 0 {
            return Err(SceneError::StaleSceneDelivery);
        }
        let state = self.view_state(figure)?;
        let target = crate::scene::view::ViewTarget {
            state,
            subscription,
            config,
            epoch: None,
            closed: false,
        };
        let binding = self.bind_dependency_inner(
            figure,
            crate::scene::SCENE_SOURCE_VIEW,
            source,
            Some(target),
        )?;
        self.invalidate_view(figure);
        Ok(binding)
    }

    fn view_state(
        &mut self,
        figure: FigureId,
    ) -> Result<Arc<std::sync::Mutex<crate::scene::view::ViewState>>, SceneError> {
        self.runtime.guarded(|runtime| {
            let node = runtime.tree.node(figure).map_err(SceneError::Tree)?;
            let descriptor = node
                .capabilities
                .get(crate::scene::SCENE_VIEW)
                .map_err(|_| SceneError::UnknownPolicy)?
                .ok_or(SceneError::UnknownPolicy)?;
            let handle = descriptor
                .resolve(node.figure.as_ref())
                .ok_or(SceneError::UnknownPolicy)?;
            if handle.namespace != runtime.tree.namespace() {
                return Err(SceneError::StaleSceneDelivery);
            }
            Ok(handle.state.clone())
        })
    }

    fn invalidate_view(&mut self, figure: FigureId) {
        self.runtime
            .tree
            .mark_invalid(&mut self.runtime.updates, figure);
        // Complete old/new coverage also repairs skipped mailbox updates and cleared views.
        self.runtime.full_redraw_pending = true;
        self.runtime
            .tree
            .repaint(&mut self.runtime.updates, figure, None);
    }

    /// Retains the accepted view and its imported resource versions.
    pub fn installed_view(
        &self,
        binding: crate::scene::SceneViewBindingId,
    ) -> Result<Option<Arc<crate::scene::InstalledScene>>, SceneError> {
        self.check_runtime()?;
        let target = self
            .runtime
            .scenes
            .view_targets
            .get(&binding)
            .ok_or(SceneError::StaleSceneDelivery)?;
        Ok(target
            .state
            .lock()
            .map_err(|_| SceneError::HandoffPoisoned)?
            .installed
            .clone())
    }

    /// Advances expected capture configuration without resetting the epoch baseline.
    pub fn expect_view_config(
        &mut self,
        binding: crate::scene::SceneViewBindingId,
        config: CaptureConfigRevision,
    ) -> Result<(), SceneError> {
        self.check_runtime()?;
        let target = self
            .runtime
            .scenes
            .view_targets
            .get_mut(&binding)
            .ok_or(SceneError::StaleSceneDelivery)?;
        if target.closed || config < target.config {
            return Err(SceneError::StaleSceneDelivery);
        }
        target.config = config;
        Ok(())
    }

    /// Changes destination display configuration without requesting source capture.
    pub fn configure_view<P: Send + Sync + 'static>(
        &mut self,
        binding: crate::scene::SceneViewBindingId,
        key: crate::scene::SceneViewPolicyKey<P>,
        payload: P,
    ) -> Result<(), SceneError> {
        self.check_runtime()?;
        self.runtime.guarded(|runtime| {
            RuntimeScenes { runtime }.configure_view_inner(binding, key, payload)
        })
    }

    fn configure_view_inner<P: Send + Sync + 'static>(
        &mut self,
        binding: crate::scene::SceneViewBindingId,
        key: crate::scene::SceneViewPolicyKey<P>,
        payload: P,
    ) -> Result<(), SceneError> {
        self.check_runtime()?;
        let &(figure, _) = self
            .runtime
            .scenes
            .bindings
            .get(&binding)
            .ok_or(SceneError::StaleSceneDelivery)?;
        let target = self
            .runtime
            .scenes
            .view_targets
            .get(&binding)
            .ok_or(SceneError::StaleSceneDelivery)?;
        if target.closed {
            return Err(SceneError::StaleSceneDelivery);
        }
        let policy = self
            .runtime
            .scenes
            .view_policies
            .get(&key.0.erased())
            .ok_or(SceneError::UnknownPolicy)?
            .clone();
        let retired = {
            let mut state = target
                .state
                .lock()
                .map_err(|_| SceneError::HandoffPoisoned)?;
            let revision = state.revision.checked_add(1).ok_or(SceneError::Exhausted)?;
            state.revision = revision;
            (
                std::mem::replace(&mut state.policy, policy),
                std::mem::replace(&mut state.payload, Arc::new(payload)),
            )
        };
        drop(retired);
        self.invalidate_view(figure);
        Ok(())
    }

    /// Atomically imports a complete candidate after checking current graph admission.
    /// Returns false only for verified identical content; stale/conflict are errors.
    pub fn install_view(
        &mut self,
        binding: crate::scene::SceneViewBindingId,
        snapshot: Arc<SceneSnapshot>,
    ) -> Result<bool, SceneError> {
        self.check_runtime()?;
        let &(figure, source) = self
            .runtime
            .scenes
            .bindings
            .get(&binding)
            .ok_or(SceneError::StaleSceneDelivery)?;
        let target = self
            .runtime
            .scenes
            .view_targets
            .get(&binding)
            .ok_or(SceneError::StaleSceneDelivery)?;
        if target.closed
            || snapshot.source != source
            || snapshot.subscription != target.subscription
            || snapshot.config != target.config
            || target
                .epoch
                .is_some_and(|epoch| snapshot.stable_epoch < epoch)
        {
            return Err(SceneError::StaleSceneDelivery);
        }
        let group = self
            .runtime
            .tree
            .scene_group
            .as_ref()
            .ok_or(SceneError::Admission(
                crate::scene::SceneAdmissionError::UnknownSceneMember,
            ))?;
        let evidence = snapshot.dependencies.as_ref().ok_or(SceneError::Admission(
            crate::scene::SceneAdmissionError::UnknownSceneMember,
        ))?;
        group
            .check_snapshot(source, evidence)
            .map_err(SceneError::Admission)?;
        let state = target.state.clone();
        let installed = state
            .lock()
            .map_err(|_| SceneError::HandoffPoisoned)?
            .installed
            .clone();
        if let Some(current) = &installed
            && current.snapshot.source == snapshot.source
            && current.snapshot.subscription == snapshot.subscription
            && current.snapshot.config == snapshot.config
            && current.snapshot.stable_epoch == snapshot.stable_epoch
        {
            if *current.snapshot != *snapshot {
                return Err(SceneError::SnapshotConflict);
            }
            return Ok(false);
        }
        let revision = state
            .lock()
            .map_err(|_| SceneError::HandoffPoisoned)?
            .revision
            .checked_add(1)
            .ok_or(SceneError::Exhausted)?;
        let candidate = self.runtime.scenes.imports.prepare(
            &self.runtime.resources,
            source.namespace,
            &snapshot.drawing,
        )?;
        let group = self
            .runtime
            .tree
            .scene_group
            .as_ref()
            .ok_or(SceneError::Admission(
                crate::scene::SceneAdmissionError::UnknownSceneMember,
            ))?;
        let evidence = snapshot.dependencies.as_ref().ok_or(SceneError::Admission(
            crate::scene::SceneAdmissionError::UnknownSceneMember,
        ))?;
        let admission = group
            .install(self.runtime.tree.namespace(), figure, source, evidence)
            .map_err(SceneError::Admission)?;
        let epoch = snapshot.stable_epoch;
        let next = Arc::new(crate::scene::InstalledScene {
            snapshot,
            drawing: candidate.drawing,
            _leases: candidate.leases,
        });
        // Lock acquisition is before metadata commit. No callback or user destructor
        // runs while the group guard is held; retired values are dropped afterwards.
        let mut view = state.lock().map_err(|_| SceneError::HandoffPoisoned)?;
        let retired = admission
            .commit(|| {
                let registry = std::mem::replace(&mut self.runtime.resources, candidate.registry);
                let imports =
                    std::mem::replace(&mut self.runtime.scenes.imports, candidate.imports);
                let previous = view.installed.replace(next);
                view.revision = revision;
                self.runtime
                    .scenes
                    .view_targets
                    .get_mut(&binding)
                    .expect("validated binding")
                    .epoch = Some(epoch);
                (registry, imports, previous)
            })
            .map_err(SceneError::Admission)?;
        drop(view);
        drop(retired);
        self.invalidate_view(figure);
        Ok(true)
    }

    /// Applies source termination independently of source or target surface presentation.
    pub fn close_view(
        &mut self,
        binding: crate::scene::SceneViewBindingId,
        closed: crate::scene::SceneClosed,
    ) -> Result<(), SceneError> {
        self.check_runtime()?;
        let &(_, source) = self
            .runtime
            .scenes
            .bindings
            .get(&binding)
            .ok_or(SceneError::StaleSceneDelivery)?;
        let target = self
            .runtime
            .scenes
            .view_targets
            .get(&binding)
            .ok_or(SceneError::StaleSceneDelivery)?;
        if source != closed.source || target.subscription != closed.subscription {
            return Err(SceneError::StaleSceneDelivery);
        }
        self.unbind_view_dependency(binding)
    }

    /// Joins a composition-root group before registering sources or bindings.
    pub fn join_group(&mut self, group: crate::scene::SceneBindingGroup) -> Result<(), SceneError> {
        self.check_runtime()?;
        if self.runtime.tree.scene_group.is_some() || !self.runtime.scenes.sources.is_empty() {
            return Err(SceneError::Admission(
                crate::scene::SceneAdmissionError::UnknownSceneMember,
            ));
        }
        let admission = group
            .join(
                self.runtime.tree.namespace(),
                self.runtime.tree.scene_parents(),
            )
            .map_err(SceneError::Admission)?;
        admission
            .commit(|| self.runtime.tree.scene_group = Some(group.clone()))
            .map_err(SceneError::Admission)
    }

    /// Registers a typed structural dependency policy for an external view.
    pub fn register_view_dependency_policy<P: 'static>(
        &mut self,
        key: crate::scene::SceneViewDependencyKey<P>,
        policy: impl crate::scene::SceneViewDependencyPolicy<P>,
    ) -> Result<(), SceneError> {
        self.check_runtime()?;
        self.runtime
            .guarded(|runtime| runtime.scenes.register_dependency_policy(key, policy))
    }

    /// Binds an attached view structurally, even if it has no visible drawing yet.
    /// Rebinding invalidates the old destination identity.
    pub fn bind_view_dependency<P: 'static>(
        &mut self,
        figure: FigureId,
        key: crate::scene::SceneViewDependencyKey<P>,
        payload: P,
    ) -> Result<crate::scene::SceneViewBindingId, SceneError> {
        self.bind_dependency_inner(figure, key, payload, None)
    }

    fn bind_dependency_inner<P: 'static>(
        &mut self,
        figure: FigureId,
        key: crate::scene::SceneViewDependencyKey<P>,
        payload: P,
        mut target: Option<crate::scene::view::ViewTarget>,
    ) -> Result<crate::scene::SceneViewBindingId, SceneError> {
        self.check_runtime()?;
        if !self
            .runtime
            .tree
            .is_attached(figure)
            .map_err(SceneError::Tree)?
        {
            return Err(SceneError::Admission(
                crate::scene::SceneAdmissionError::UnknownSceneMember,
            ));
        }
        self.runtime.guarded(|runtime| {
            let source = runtime.scenes.dependency_source(key, &payload)?;
            let old_binding = runtime
                .scenes
                .bindings
                .iter()
                .find_map(|(id, (bound, _))| (*bound == figure).then_some(*id));
            if let Some(target) = target.as_mut() {
                target.epoch = old_binding
                    .filter(|id| runtime.scenes.bindings[id].1 == source)
                    .and_then(|id| runtime.scenes.view_targets.get(&id))
                    .and_then(|previous| previous.epoch);
            }
            let state = target
                .as_ref()
                .map(|target| target.state.clone())
                .or_else(|| {
                    old_binding
                        .and_then(|id| runtime.scenes.view_targets.get(&id))
                        .map(|target| target.state.clone())
                });
            let mut view = state
                .as_ref()
                .map(|state| state.lock().map_err(|_| SceneError::HandoffPoisoned))
                .transpose()?;
            let revision = view
                .as_ref()
                .map(|state| state.revision.checked_add(1).ok_or(SceneError::Exhausted))
                .transpose()?;
            let group = runtime
                .tree
                .scene_group
                .as_ref()
                .ok_or(SceneError::Admission(
                    crate::scene::SceneAdmissionError::UnknownSceneMember,
                ))?;
            let admission = group
                .binding(runtime.tree.namespace(), figure, Some(source))
                .map_err(SceneError::Admission)?;
            let binding = crate::scene::SceneViewBindingId {
                namespace: runtime.tree.namespace(),
                sequence: runtime.scenes.allocate_identity()?,
            };
            let retired = admission
                .commit(|| {
                    runtime
                        .scenes
                        .bindings
                        .retain(|_, (bound, _)| *bound != figure);
                    runtime.scenes.bindings.insert(binding, (figure, source));
                    let old_target =
                        old_binding.and_then(|id| runtime.scenes.view_targets.remove(&id));
                    if let Some(target) = target {
                        runtime.scenes.view_targets.insert(binding, target);
                    }
                    let old_scene = view.as_mut().and_then(|state| {
                        state.revision = revision.expect("checked view revision");
                        state.installed.take()
                    });
                    (old_target, old_scene)
                })
                .map_err(SceneError::Admission)?;
            drop(view);
            drop(retired);
            if state.is_some() {
                runtime.tree.mark_invalid(&mut runtime.updates, figure);
                runtime.full_redraw_pending = true;
                runtime.tree.repaint(&mut runtime.updates, figure, None);
            }
            Ok(binding)
        })
    }

    /// Retires one current destination binding.
    pub fn unbind_view_dependency(
        &mut self,
        binding: crate::scene::SceneViewBindingId,
    ) -> Result<(), SceneError> {
        self.check_runtime()?;
        let &(figure, _) =
            self.runtime
                .scenes
                .bindings
                .get(&binding)
                .ok_or(SceneError::Admission(
                    crate::scene::SceneAdmissionError::UnknownSceneMember,
                ))?;
        let group = self
            .runtime
            .tree
            .scene_group
            .as_ref()
            .ok_or(SceneError::Admission(
                crate::scene::SceneAdmissionError::UnknownSceneMember,
            ))?;
        let admission = group
            .binding(self.runtime.tree.namespace(), figure, None)
            .map_err(SceneError::Admission)?;
        let state = self
            .runtime
            .scenes
            .view_targets
            .get(&binding)
            .map(|target| target.state.clone());
        let mut view = state
            .as_ref()
            .map(|state| state.lock().map_err(|_| SceneError::HandoffPoisoned))
            .transpose()?;
        let revision = view
            .as_ref()
            .map(|state| state.revision.checked_add(1).ok_or(SceneError::Exhausted))
            .transpose()?;
        let retired = admission
            .commit(|| {
                self.runtime.scenes.bindings.remove(&binding);
                let target = self.runtime.scenes.view_targets.remove(&binding);
                let scene = view.as_mut().and_then(|state| {
                    state.revision = revision.expect("checked view revision");
                    state.installed.take()
                });
                (target, scene)
            })
            .map_err(SceneError::Admission)?;
        drop(view);
        drop(retired);
        if state.is_some() {
            self.invalidate_view(figure);
        }
        Ok(())
    }

    fn check_runtime(&self) -> Result<(), SceneError> {
        if self.runtime.faulted {
            Err(SceneError::Faulted)
        } else {
            Ok(())
        }
    }

    /// Registers an external policy through the same registry as the default policy.
    pub fn register_capture_policy<P: 'static>(
        &mut self,
        key: SceneCapturePolicyKey<P>,
        policy: impl SceneCapturePolicy<P>,
    ) -> Result<(), SceneError> {
        self.check_runtime()?;
        self.runtime
            .guarded(|runtime| runtime.scenes.register_policy(key, policy))
    }

    /// Registers an attached capture root, without transferring its mutation authority.
    pub fn register_source(&mut self, root: FigureId) -> Result<SceneSourceId, SceneError> {
        self.check_runtime()?;
        if !self
            .runtime
            .tree
            .is_attached(root)
            .map_err(SceneError::Tree)?
        {
            return Err(SceneError::UnknownSource);
        }
        let source = SceneSourceId {
            namespace: self.runtime.tree.namespace(),
            sequence: self.runtime.scenes.allocate_identity()?,
        };
        if let Some(group) = &self.runtime.tree.scene_group {
            let admission = group
                .source(source, Some(root))
                .map_err(SceneError::Admission)?;
            admission
                .commit(|| self.runtime.scenes.sources.insert(source, root))
                .map_err(SceneError::Admission)?;
        } else {
            self.runtime.scenes.sources.insert(source, root);
        }
        Ok(source)
    }

    /// Creates an independent subscription. Host drives initial capture explicitly.
    pub fn subscribe<P: 'static>(
        &mut self,
        source: SceneSourceId,
        key: SceneCapturePolicyKey<P>,
        payload: P,
    ) -> Result<(SceneSubscriptionId, CaptureConfigRevision, SceneHandoff), SceneError> {
        self.check_runtime()?;
        self.runtime.scenes.subscribe(source, key, payload)
    }

    /// Changes configuration without changing source state or its stable epoch.
    pub fn configure<P: 'static>(
        &mut self,
        id: SceneSubscriptionId,
        key: SceneCapturePolicyKey<P>,
        payload: P,
    ) -> Result<CaptureConfigRevision, SceneError> {
        self.check_runtime()?;
        self.runtime
            .guarded(|runtime| runtime.scenes.configure(id, key, payload))
    }

    /// Reports initial/configuration work or new source input since the last attempt.
    /// Failed attempts do not continuously request retry without new input.
    pub fn needs_capture(&self, id: SceneSubscriptionId) -> Result<bool, SceneError> {
        self.check_runtime()?;
        let subscription = self
            .runtime
            .scenes
            .subscriptions
            .get(&id)
            .ok_or(SceneError::UnknownSubscription)?;
        let member_generation = self
            .runtime
            .tree
            .scene_group
            .as_ref()
            .map(|group| group.member_generation(subscription.source))
            .transpose()
            .map_err(SceneError::Admission)?;
        Ok(
            subscription.attempted_inputs != Some(self.runtime.scene_capture_inputs()?)
                || subscription.attempted_member_generation != member_generation
                || !self.runtime.mutations.is_empty(),
        )
    }

    /// Explicitly drives one capture or retry, including when the source surface is paused.
    /// A failure is handed to this subscription only; no candidate is published on failure.
    pub fn capture(&mut self, id: SceneSubscriptionId) -> Result<Arc<SceneSnapshot>, SceneError> {
        self.check_runtime()?;
        if !self.runtime.scenes.subscriptions.contains_key(&id) {
            return Err(SceneError::UnknownSubscription);
        }
        let result = self.runtime.stabilize_for_capture().and_then(|()| {
            self.runtime
                .guarded(|runtime| runtime.capture_subscription_inner(id))
        });
        let inputs = self.runtime.scene_capture_inputs()?;
        let source = self
            .runtime
            .scenes
            .subscriptions
            .get(&id)
            .ok_or(SceneError::UnknownSubscription)?
            .source;
        let member_generation = self
            .runtime
            .tree
            .scene_group
            .as_ref()
            .map(|group| group.member_generation(source))
            .transpose()
            .map_err(SceneError::Admission)?;
        let subscription = self
            .runtime
            .scenes
            .subscriptions
            .get_mut(&id)
            .ok_or(SceneError::UnknownSubscription)?;
        subscription.attempted_inputs = Some(inputs);
        subscription.attempted_member_generation = member_generation;
        self.runtime
            .guarded(|runtime| runtime.scenes.publish(id, result.clone()))?;
        result
    }

    /// Cancels one subscription and publishes a terminal event independently of repaint.
    pub fn cancel(&mut self, id: SceneSubscriptionId) -> Result<(), SceneError> {
        self.runtime
            .guarded(|runtime| runtime.scenes.cancel(id, SceneCloseReason::Cancelled))
    }

    /// Retires a source and every subscription observing that source.
    pub fn remove_source(&mut self, id: SceneSourceId) -> Result<(), SceneError> {
        self.check_runtime()?;
        if !self.runtime.scenes.sources.contains_key(&id) {
            return Err(SceneError::UnknownSource);
        }
        if let Some(group) = &self.runtime.tree.scene_group {
            // Metadata retires before any host waker or user payload destructor.
            group
                .source(id, None)
                .map_err(SceneError::Admission)?
                .commit(|| ())
                .map_err(SceneError::Admission)?;
        }
        self.runtime.guarded(|runtime| {
            runtime
                .scenes
                .remove_source(id, SceneCloseReason::SourceRemoved)
        })
    }
}

fn validate_capture_rectangle(rect: Rectangle) -> Result<(), SceneError> {
    if [
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        rect.x + rect.width,
        rect.y + rect.height,
    ]
    .into_iter()
    .any(|value| !value.is_finite())
        || rect.width < 0.0
        || rect.height < 0.0
    {
        return Err(SceneError::InvalidCaptureRectangle);
    }
    Ok(())
}
