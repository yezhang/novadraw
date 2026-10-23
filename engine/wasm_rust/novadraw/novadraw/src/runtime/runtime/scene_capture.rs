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

    fn scene_capture_inputs(&self) -> Result<(u64, u64, crate::event::MonotonicTime), SceneError> {
        Ok((
            self.tree
                .notification_input_serial()
                .ok_or(SceneError::Exhausted)?,
            self.updates
                .notification_input_serial()
                .ok_or(SceneError::Exhausted)?,
            self.activity_now,
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
        let font = crate::text::FontDescriptor::parse(&style.font).map_err(|error| {
            SceneError::Stabilization(super::FramePreparationError::Text(error))
        })?;
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
        self.runtime.scenes.register_source(root)
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
        Ok(
            subscription.attempted_inputs != Some(self.runtime.scene_capture_inputs()?)
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
        self.runtime
            .scenes
            .subscriptions
            .get_mut(&id)
            .ok_or(SceneError::UnknownSubscription)?
            .attempted_inputs = Some(inputs);
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
