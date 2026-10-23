//! Open display policies and read-only Figure consumers.
use super::import::ImportLease;
use super::{SceneCapturePolicyKey, SceneError, SceneSnapshot};
use crate::{
    advanced::RuntimeNamespace,
    figure::{
        CapabilityKey, Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError,
        FigureDrawing, FigureMeasurement, FigurePreparation, FigurePresentation,
        MeasureConstraints, PREPARATION, PreparationCapability, PreparationMeasureError,
        PreparationMeasureErrorKind, PreparationMeasurement, PreparationScene,
    },
    geometry::{Affine2D, Dimension, Rectangle},
    graphics::{GraphicsError, PaintContext},
    render::RecordedDrawing,
    text::{MeasureContext, TextError},
};
use std::{
    any::Any,
    sync::{Arc, Mutex},
};

/// Typed identity of a destination display policy.
pub struct SceneViewPolicyKey<P>(pub(crate) SceneCapturePolicyKey<P>);
impl<P> SceneViewPolicyKey<P> {
    /// Creates a stable namespaced policy identity.
    pub const fn new(namespace: &'static str, name: &'static str) -> Self {
        Self(SceneCapturePolicyKey::new(namespace, name))
    }
}
impl<P> Copy for SceneViewPolicyKey<P> {}
impl<P> Clone for SceneViewPolicyKey<P> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Content placement. Core checks all metrics before publishing drawing.
#[derive(Clone, Copy, Debug)]
pub struct SceneViewPlacement {
    /// Source drawing to destination content transform.
    pub transform: Affine2D,
    /// Additional content clip, intersected with the Figure's normal clip.
    pub clip: Rectangle,
}

/// Destination-owned measurement and placement; receives content domains only.
pub trait SceneViewPolicy<P>: Send + Sync + 'static {
    /// Measures content; Border insets are applied separately by Core.
    fn measure(
        &self,
        payload: &P,
        natural: Dimension,
        constraints: MeasureConstraints,
    ) -> Result<PreparationMeasurement, SceneError>;
    /// Places drawing inside the laid-out client area.
    fn place(
        &self,
        payload: &P,
        natural: Dimension,
        client: Rectangle,
    ) -> Result<SceneViewPlacement, SceneError>;
}

/// Default thumbnail configuration, independent of source capture configuration.
#[derive(Clone, Copy, Debug, Default)]
pub struct AspectFitView {
    /// Explicit content target box, or the natural source extent.
    pub preferred: Option<Dimension>,
}
/// Default display policy, registered through the public policy registry.
pub const ASPECT_FIT_VIEW: SceneViewPolicyKey<AspectFitView> =
    SceneViewPolicyKey::new("novadraw.scene", "aspect-fit");
pub(crate) struct AspectFitPolicy;

fn valid_size(size: Dimension) -> Result<(), SceneError> {
    if [size.width, size.height]
        .into_iter()
        .any(|v| !v.is_finite() || v < 0.0)
    {
        Err(SceneError::InvalidCaptureRectangle)
    } else {
        Ok(())
    }
}
impl SceneViewPolicy<AspectFitView> for AspectFitPolicy {
    fn measure(
        &self,
        payload: &AspectFitView,
        natural: Dimension,
        constraints: MeasureConstraints,
    ) -> Result<PreparationMeasurement, SceneError> {
        valid_size(natural)?;
        let preferred = payload.preferred.unwrap_or(natural);
        valid_size(preferred)?;
        let size = if natural.width == 0.0 || natural.height == 0.0 {
            Dimension::ZERO
        } else {
            let width = constraints
                .max_width()
                .map_or(preferred.width, |w| preferred.width.min(w));
            let height = constraints
                .max_height()
                .map_or(preferred.height, |h| preferred.height.min(h));
            let scale = if payload.preferred.is_some() {
                (width / natural.width).min(height / natural.height)
            } else {
                match (constraints.max_width(), constraints.max_height()) {
                    (Some(w), Some(h)) => (w / natural.width).min(h / natural.height).min(1.0),
                    (Some(w), None) => (w / natural.width).min(1.0),
                    (None, Some(h)) => (h / natural.height).min(1.0),
                    (None, None) => 1.0,
                }
            };
            Dimension::new(natural.width * scale, natural.height * scale)
        };
        valid_size(size)?;
        Ok(PreparationMeasurement {
            preferred: FigureMeasurement::new(size.width, size.height, None),
            minimum: Dimension::ZERO,
        })
    }
    fn place(
        &self,
        _: &AspectFitView,
        natural: Dimension,
        client: Rectangle,
    ) -> Result<SceneViewPlacement, SceneError> {
        valid_size(natural)?;
        valid_size(Dimension::new(client.width, client.height))?;
        let scale = if natural.width == 0.0 || natural.height == 0.0 {
            0.0
        } else {
            (client.width / natural.width).min(client.height / natural.height)
        };
        Ok(SceneViewPlacement {
            transform: Affine2D::from_translation(client.x, client.y)
                * Affine2D::from_uniform_scale(scale),
            clip: client,
        })
    }
}

type MeasureCallback = dyn Fn(&dyn Any, Dimension, MeasureConstraints) -> Result<PreparationMeasurement, SceneError>
    + Send
    + Sync;
type PlaceCallback =
    dyn Fn(&dyn Any, Dimension, Rectangle) -> Result<SceneViewPlacement, SceneError> + Send + Sync;
#[derive(Clone)]
pub(crate) struct ViewPolicy {
    pub(crate) measure: Arc<MeasureCallback>,
    pub(crate) place: Arc<PlaceCallback>,
}
pub(crate) struct ViewState {
    pub(crate) revision: u64,
    pub(crate) policy: ViewPolicy,
    pub(crate) payload: Arc<dyn Any + Send + Sync>,
    pub(crate) installed: Option<Arc<InstalledScene>>,
}
pub(crate) struct ViewTarget {
    pub(crate) state: Arc<Mutex<ViewState>>,
    pub(crate) subscription: super::SceneSubscriptionId,
    pub(crate) config: super::CaptureConfigRevision,
    pub(crate) epoch: Option<u64>,
    pub(crate) closed: bool,
}
/// Owned destination result. Retaining it retains its imported resource versions.
pub struct InstalledScene {
    pub(crate) snapshot: Arc<SceneSnapshot>,
    pub(crate) drawing: RecordedDrawing,
    pub(crate) _leases: Vec<Arc<ImportLease>>,
}
impl InstalledScene {
    /// Returns the original producer result.
    pub fn snapshot(&self) -> &SceneSnapshot {
        &self.snapshot
    }
    /// Returns drawing rewritten to this destination's resource identities.
    pub fn drawing(&self) -> &RecordedDrawing {
        &self.drawing
    }
}

/// Read-only consumer. Only its destination Runtime can replace its state.
pub struct SceneViewHandle {
    pub(crate) namespace: RuntimeNamespace,
    pub(crate) state: Arc<Mutex<ViewState>>,
}
impl SceneViewHandle {
    /// Returns this consumer's checked presentation input revision.
    pub fn revision(&self) -> Result<u64, SceneError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| SceneError::HandoffPoisoned)?
            .revision)
    }
    /// Retains the current accepted result and its CPU resource leases.
    pub fn installed(&self) -> Result<Option<Arc<InstalledScene>>, SceneError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| SceneError::HandoffPoisoned)?
            .installed
            .clone())
    }
    /// Measures content through the selected typed display policy.
    pub fn measure(
        &self,
        constraints: MeasureConstraints,
    ) -> Result<PreparationMeasurement, SceneError> {
        let (policy, payload, installed) = {
            let state = self.state.lock().map_err(|_| SceneError::HandoffPoisoned)?;
            (
                state.policy.clone(),
                state.payload.clone(),
                state.installed.clone(),
            )
        };
        let natural = installed.as_ref().map_or(Dimension::ZERO, |scene| {
            let rect = scene.snapshot.rectangle;
            Dimension::new(rect.width, rect.height)
        });
        checked_measurement((policy.measure)(payload.as_ref(), natural, constraints)?)
    }
    /// Prepares an immutable drawing for a default or external Figure.
    pub fn prepare(&self, client: Rectangle) -> Result<FigurePresentation, TextError> {
        let (policy, payload, installed) = {
            let state = self
                .state
                .lock()
                .map_err(|_| scene_text_error(SceneError::HandoffPoisoned))?;
            (
                state.policy.clone(),
                state.payload.clone(),
                state.installed.clone(),
            )
        };
        let natural = installed.as_ref().map_or(Dimension::ZERO, |scene| {
            let rect = scene.snapshot.rectangle;
            Dimension::new(rect.width, rect.height)
        });
        let placement =
            (policy.place)(payload.as_ref(), natural, client).map_err(scene_text_error)?;
        if placement
            .transform
            .coeffs()
            .into_iter()
            .any(|v| !v.is_finite())
            || [
                placement.clip.x,
                placement.clip.y,
                placement.clip.width,
                placement.clip.height,
                placement.clip.x + placement.clip.width,
                placement.clip.y + placement.clip.height,
            ]
            .into_iter()
            .any(|v| !v.is_finite())
            || placement.clip.width < 0.0
            || placement.clip.height < 0.0
        {
            return Err(TextError::InvalidMetric);
        }
        let measurement = checked_measurement(
            (policy.measure)(payload.as_ref(), natural, MeasureConstraints::UNBOUNDED)
                .map_err(scene_text_error)?,
        )
        .map_err(scene_text_error)?;
        FigurePresentation::new(
            measurement.preferred,
            measurement.minimum,
            client,
            Arc::new(ViewDrawing {
                installed,
                placement,
                client,
            }),
        )
    }
}
pub(crate) fn scene_text_error(error: SceneError) -> TextError {
    TextError::SceneView(error.to_string())
}
fn checked_measurement(
    measurement: PreparationMeasurement,
) -> Result<PreparationMeasurement, SceneError> {
    valid_size(measurement.preferred.size())?;
    valid_size(measurement.minimum)?;
    if measurement.preferred.baseline.is_some_and(|baseline| {
        !baseline.is_finite() || baseline < 0.0 || baseline > measurement.preferred.height
    }) {
        return Err(SceneError::Policy("invalid view baseline".into()));
    }
    Ok(measurement)
}
struct ViewDrawing {
    installed: Option<Arc<InstalledScene>>,
    placement: SceneViewPlacement,
    client: Rectangle,
}
impl FigureDrawing for ViewDrawing {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        let Some(scene) = &self.installed else {
            return Ok(());
        };
        if scene.snapshot.rectangle.is_empty()
            || self.placement.clip.is_empty()
            || self.client.is_empty()
        {
            return Ok(());
        }
        context.push_state();
        let result = (|| {
            context.clip_rect(self.client)?;
            context.clip_rect(self.placement.clip)?;
            context.concat_transform(self.placement.transform)?;
            context.replay(&scene.drawing)
        })();
        result.and(context.pop_state())
    }
}

/// External Figures expose the same read-only consumer through a typed descriptor.
pub trait SceneViewConsumer {
    /// Exposes the owned read-only view consumed by this Figure.
    fn scene_view(&self) -> &SceneViewHandle;
}
/// Typed resolver shared by default and external Figure consumers.
#[derive(Clone, Copy)]
pub struct SceneViewCapability {
    resolve: for<'a> fn(&'a dyn Figure) -> Option<&'a SceneViewHandle>,
}
impl SceneViewCapability {
    /// Builds a descriptor for any Figure implementing the consumer protocol.
    pub fn of<T: Figure + SceneViewConsumer + 'static>() -> Self {
        Self {
            resolve: |figure| {
                figure
                    .as_any()
                    .downcast_ref::<T>()
                    .map(SceneViewConsumer::scene_view)
            },
        }
    }
    pub(crate) fn resolve(self, figure: &dyn Figure) -> Option<&SceneViewHandle> {
        (self.resolve)(figure)
    }
}
/// Scene view consumer descriptor identity.
pub const SCENE_VIEW: CapabilityKey<SceneViewCapability> =
    CapabilityKey::new("novadraw.scene.view");

/// Ordinary prepared Figure displaying an accepted immutable source scene.
pub struct SceneViewFigure {
    bounds: Rectangle,
    view: SceneViewHandle,
    border: Option<Arc<dyn crate::figure::Border>>,
}
impl SceneViewFigure {
    /// Moves a read-only consumer into a detached Figure.
    pub fn new(view: SceneViewHandle) -> Self {
        Self {
            bounds: Rectangle::ZERO,
            view,
            border: None,
        }
    }
    /// Sets detached placement.
    pub fn with_bounds(mut self, bounds: Rectangle) -> Self {
        self.bounds = bounds;
        self
    }
    /// Adds an ordinary Border, measured by Core exactly once.
    pub fn with_border(mut self, border: impl crate::figure::Border + 'static) -> Self {
        self.border = Some(Arc::new(border));
        self
    }
}
impl crate::figure::BorderedFigure for SceneViewFigure {
    fn border(&self) -> Option<&Arc<dyn crate::figure::Border>> {
        self.border.as_ref()
    }
    fn replace_border(
        &mut self,
        border: Option<Arc<dyn crate::figure::Border>>,
    ) -> Option<Arc<dyn crate::figure::Border>> {
        std::mem::replace(&mut self.border, border)
    }
}
impl SceneViewConsumer for SceneViewFigure {
    fn scene_view(&self) -> &SceneViewHandle {
        &self.view
    }
}
impl Figure for SceneViewFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }
    fn initial_insets(&self) -> crate::geometry::Insets {
        self.border
            .as_ref()
            .map_or(crate::geometry::Insets::ZERO, |border| border.get_insets())
    }
    fn name(&self) -> &'static str {
        "SceneViewFigure"
    }
    fn get_border(&self) -> Option<&dyn crate::figure::Border> {
        self.border.as_deref()
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(SCENE_VIEW, SceneViewCapability::of::<Self>())?;
        out.register(
            crate::figure::BORDER,
            crate::figure::BorderCapability::of::<Self>(),
        )?;
        out.register(PREPARATION, PreparationCapability::of::<Self>())
    }
}
impl FigurePreparation for SceneViewFigure {
    fn revision(&self) -> u64 {
        self.view.revision().expect("Core-owned view state")
    }
    fn measure_scene(
        &self,
        _: &mut MeasureContext<'_>,
        constraints: MeasureConstraints,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparationMeasurement, PreparationMeasureError> {
        self.view
            .measure(constraints)
            .map_err(|error| PreparationMeasureError {
                figure: scene.root(),
                outer_constraints: constraints,
                content_constraints: Some(constraints),
                kind: PreparationMeasureErrorKind::Text(Box::new(scene_text_error(error))),
            })
    }
    fn prepare(
        &self,
        _: &mut MeasureContext<'_>,
        client: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        self.view.prepare(client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conflict_and_broken_closure_leave_installed_content_and_graph_unchanged() {
        let group = crate::scene::SceneBindingGroup::default();
        let mut producer = crate::runtime::Runtime::empty();
        let root = producer
            .set_contents(Box::new(crate::figure::RectangleFigure::new(
                0.0, 0.0, 100.0, 50.0,
            )))
            .unwrap();
        producer.scenes().join_group(group.clone()).unwrap();
        let source = producer.scenes().register_source(root).unwrap();
        let (subscription, config, _) = producer
            .scenes()
            .subscribe(
                source,
                crate::scene::SOURCE_LOCAL_CAPTURE,
                crate::scene::SourceLocalCapture::default(),
            )
            .unwrap();
        let snapshot = producer.scenes().capture(subscription).unwrap();
        let mut destination = crate::runtime::Runtime::empty();
        destination.scenes().join_group(group.clone()).unwrap();
        let view = destination
            .scenes()
            .create_view(ASPECT_FIT_VIEW, AspectFitView::default())
            .unwrap();
        let figure = destination
            .set_contents(Box::new(
                SceneViewFigure::new(view).with_bounds(Rectangle::new(0.0, 0.0, 100.0, 50.0)),
            ))
            .unwrap();
        let binding = destination
            .scenes()
            .bind_view(figure, source, subscription, config)
            .unwrap();
        destination
            .scenes()
            .install_view(binding, snapshot.clone())
            .unwrap();
        let current = destination
            .scenes()
            .installed_view(binding)
            .unwrap()
            .unwrap();
        let revision = group.graph_revision().unwrap();
        let mut conflict = (*snapshot).clone();
        conflict.rectangle.width -= 1.0;
        assert_eq!(
            destination
                .scenes()
                .install_view(binding, Arc::new(conflict)),
            Err(SceneError::SnapshotConflict)
        );
        let mut broken = (*snapshot).clone();
        broken.stable_epoch += 1;
        broken
            .drawing
            .resources
            .ready
            .push(crate::render::ResourceUpdate {
                id: crate::render::ResourceId::new(uuid::Uuid::nil(), 1),
                revision: 1,
                payload: crate::render::ResourcePayload::Image(Arc::new(
                    crate::render::ImageData::from_rgba(1, 1, vec![255; 4], 1.0),
                )),
            });
        assert_eq!(
            destination.scenes().install_view(binding, Arc::new(broken)),
            Err(SceneError::ResourceMismatch)
        );
        assert_eq!(group.graph_revision().unwrap(), revision);
        assert!(Arc::ptr_eq(
            &current,
            &destination
                .scenes()
                .installed_view(binding)
                .unwrap()
                .unwrap()
        ));
    }
    #[test]
    fn aspect_measurement_obeys_natural_size_axis_hints_and_explicit_box() {
        let policy = AspectFitPolicy;
        let natural = Dimension::new(200.0, 100.0);
        for (constraints, expected) in [
            (MeasureConstraints::UNBOUNDED, natural),
            (
                MeasureConstraints::width(80.0).unwrap(),
                Dimension::new(80.0, 40.0),
            ),
            (
                MeasureConstraints::height(20.0).unwrap(),
                Dimension::new(40.0, 20.0),
            ),
            (
                MeasureConstraints::bounded(100.0, 30.0).unwrap(),
                Dimension::new(60.0, 30.0),
            ),
            (MeasureConstraints::width(0.0).unwrap(), Dimension::ZERO),
        ] {
            let result = policy
                .measure(&AspectFitView::default(), natural, constraints)
                .unwrap();
            assert_eq!(result.preferred.size(), expected);
            assert_eq!(result.minimum, Dimension::ZERO);
        }
        let explicit = AspectFitView {
            preferred: Some(Dimension::new(400.0, 150.0)),
        };
        assert_eq!(
            policy
                .measure(&explicit, natural, MeasureConstraints::UNBOUNDED)
                .unwrap()
                .preferred
                .size(),
            Dimension::new(300.0, 150.0)
        );
        assert_eq!(
            policy
                .measure(&explicit, Dimension::ZERO, MeasureConstraints::UNBOUNDED)
                .unwrap()
                .preferred
                .size(),
            Dimension::ZERO
        );
        assert!(
            policy
                .measure(
                    &AspectFitView {
                        preferred: Some(Dimension::new(f64::INFINITY, 1.0))
                    },
                    natural,
                    MeasureConstraints::UNBOUNDED
                )
                .is_err()
        );
    }
    #[test]
    fn aspect_placement_preserves_client_origin_and_rejects_overflow_at_preparation() {
        let placement = AspectFitPolicy
            .place(
                &AspectFitView::default(),
                Dimension::new(100.0, 50.0),
                Rectangle::new(3.0, 7.0, 200.0, 200.0),
            )
            .unwrap();
        assert_eq!(
            placement
                .transform
                .transform_point(crate::geometry::Point::ORIGIN),
            crate::geometry::Point::new(3.0, 7.0)
        );
        assert_eq!(
            placement
                .transform
                .transform_point(crate::geometry::Point::new(100.0, 50.0)),
            crate::geometry::Point::new(203.0, 107.0)
        );
        assert!(
            AspectFitPolicy
                .place(
                    &AspectFitView::default(),
                    Dimension::new(-1.0, 1.0),
                    Rectangle::ZERO
                )
                .is_err()
        );
    }
}
