use super::registry::Instance;
use super::{FeedbackDeclaration, ProtocolKey, SnapError, SnapGeometry, SnapRegistration};
use crate::{EditPartFactory, FeedbackVisual, GraphicalViewer, ModelAdapter};
use std::{
    any::{Any, TypeId},
    error::Error,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// Open feedback renderer shared by internal and external strategies.
/// Configuration is immutable during calls; changes use checked replacement.
pub trait FeedbackRenderer: 'static {
    /// Owned candidate declaration's typed payload.
    type Payload: 'static;
    /// Original renderer error.
    type Error: Error + 'static;
    /// Typed renderer family identity.
    fn key() -> ProtocolKey<Self>
    where
        Self: Sized;
    /// Creates owned overlay visuals using the final, actual-plan geometry.
    fn render(
        &self,
        payload: &Self::Payload,
        geometry: &SnapGeometry,
    ) -> Result<Vec<FeedbackVisual>, Self::Error>;
}

/// Registry identity of a configured renderer.
pub struct RendererId<R: FeedbackRenderer> {
    instance: Instance,
    marker: PhantomData<fn() -> R>,
}
impl<R: FeedbackRenderer> Copy for RendererId<R> {}
impl<R: FeedbackRenderer> Clone for RendererId<R> {
    fn clone(&self) -> Self {
        *self
    }
}
trait ErasedRenderer {
    fn render(
        &self,
        payload: &dyn Any,
        geometry: &SnapGeometry,
    ) -> Result<Vec<FeedbackVisual>, SnapError>;
}
struct Renderer<R: FeedbackRenderer>(R);
impl<R: FeedbackRenderer> ErasedRenderer for Renderer<R> {
    fn render(
        &self,
        payload: &dyn Any,
        geometry: &SnapGeometry,
    ) -> Result<Vec<FeedbackVisual>, SnapError> {
        let payload = payload
            .downcast_ref::<R::Payload>()
            .ok_or(SnapError::Identity)?;
        self.0
            .render(payload, geometry)
            .map_err(|e| SnapError::extension("snap.feedback.render", e))
    }
}
pub(crate) struct RendererEntry {
    instance: Instance,
    payload: TypeId,
    renderer: Box<dyn ErasedRenderer>,
}
impl SnapRegistration<'_> {
    /// Installs the unique renderer for a typed declaration payload.
    pub fn register_renderer<R: FeedbackRenderer>(
        &mut self,
        _key: ProtocolKey<R>,
        renderer: R,
    ) -> Result<RendererId<R>, SnapError> {
        let payload = TypeId::of::<R::Payload>();
        if self
            .registry
            .renderers
            .iter()
            .any(|entry| entry.payload == payload)
        {
            return Err(SnapError::Invalid(
                "feedback payload already has a renderer",
            ));
        }
        let instance = Instance::new(self.registry.owner);
        self.registry.renderers.push(RendererEntry {
            instance,
            payload,
            renderer: Box::new(Renderer(renderer)),
        });
        self.registry.changed();
        Ok(RendererId {
            instance,
            marker: PhantomData,
        })
    }
    /// Replaces a live renderer and retires old receipts through registry revision.
    pub fn replace_renderer<R: FeedbackRenderer>(
        &mut self,
        id: RendererId<R>,
        renderer: R,
    ) -> Result<RendererId<R>, SnapError> {
        let index = self
            .registry
            .renderers
            .iter()
            .position(|entry| entry.instance == id.instance)
            .ok_or(SnapError::Identity)?;
        let instance = id.instance.replaced();
        self.registry.renderers[index] = RendererEntry {
            instance,
            payload: TypeId::of::<R::Payload>(),
            renderer: Box::new(Renderer(renderer)),
        };
        self.registry.changed();
        Ok(RendererId {
            instance,
            marker: PhantomData,
        })
    }
    /// Removes a live renderer, invalidating existing solutions.
    pub fn remove_renderer<R: FeedbackRenderer>(
        &mut self,
        id: RendererId<R>,
    ) -> Result<(), SnapError> {
        let index = self
            .registry
            .renderers
            .iter()
            .position(|entry| entry.instance == id.instance)
            .ok_or(SnapError::Identity)?;
        self.registry.renderers.remove(index);
        self.registry.changed();
        Ok(())
    }
}
impl<A: ModelAdapter, F: EditPartFactory<A>> GraphicalViewer<A, F> {
    pub(crate) fn render_snap_feedback(
        &mut self,
        declarations: &[FeedbackDeclaration],
        geometry: &SnapGeometry,
        limit: usize,
    ) -> Result<Vec<FeedbackVisual>, SnapError> {
        self.snap_ready()?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut visuals = Vec::new();
            for declaration in declarations {
                let payload = declaration.erased_payload();
                let entry = self
                    .snapping
                    .renderers
                    .iter()
                    .find(|entry| entry.payload == payload.type_id())
                    .ok_or(SnapError::Identity)?;
                let mut rendered = entry.renderer.render(payload, geometry)?;
                if rendered.len() > limit.saturating_sub(visuals.len()) {
                    return Err(SnapError::Budget("rendered feedback budget exceeded"));
                }
                visuals.append(&mut rendered);
            }
            Ok(visuals)
        }));
        self.finish_snap_boundary(result, "snap.feedback.render")
    }
}

/// Surface reference-line declaration, also available to third-party strategies.
pub struct ReferenceLine {
    point: novadraw::geometry::Point,
    normal: novadraw::geometry::Vec2,
}
impl ReferenceLine {
    /// Checks a finite point and nonzero normal.
    pub fn new(
        point: novadraw::geometry::Point,
        normal: novadraw::geometry::Vec2,
    ) -> Result<Self, SnapError> {
        super::geometry::finite_point(point)?;
        let length = normal.x().hypot(normal.y());
        if !length.is_finite() || length == 0.0 {
            return Err(SnapError::Invalid("invalid reference line normal"));
        }
        Ok(Self {
            point,
            normal: novadraw::geometry::Vec2::new(normal.x() / length, normal.y() / length),
        })
    }
    /// Payload identity used by ordinary feedback declarations and renderers.
    pub fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("reference-line")
    }
}

/// Ordinary registered renderer for built-in or externally contributed reference lines.
pub struct ReferenceLineRenderer {
    color: novadraw::graphics::Color,
    stroke: novadraw::render::StrokeStyle,
    padding: f64,
}
impl ReferenceLineRenderer {
    /// Uses explicit surface style and extent padding; no UI style is chosen by the solver.
    pub fn new(
        color: novadraw::graphics::Color,
        stroke: novadraw::render::StrokeStyle,
        padding: f64,
    ) -> Result<Self, SnapError> {
        if !padding.is_finite() || padding < 0.0 {
            return Err(SnapError::Invalid("invalid reference line padding"));
        }
        Ok(Self {
            color,
            stroke,
            padding,
        })
    }
}
impl FeedbackRenderer for ReferenceLineRenderer {
    type Payload = ReferenceLine;
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("reference-line-renderer")
    }
    fn render(
        &self,
        line: &ReferenceLine,
        geometry: &SnapGeometry,
    ) -> Result<Vec<FeedbackVisual>, SnapError> {
        let tangent = novadraw::geometry::Vec2::new(-line.normal.y(), line.normal.x());
        let mut low = 0.0_f64;
        let mut high = 0.0_f64;
        for &point in geometry.points() {
            let delta = point - line.point;
            let distance = delta.x() * tangent.x() + delta.y() * tangent.y();
            if !distance.is_finite() {
                return Err(SnapError::Invalid("reference line extent overflow"));
            }
            low = low.min(distance);
            high = high.max(distance);
        }
        let endpoint = |distance: f64| {
            super::geometry::finite_point(novadraw::geometry::Point::new(
                line.point.x() + tangent.x() * distance,
                line.point.y() + tangent.y() * distance,
            ))
        };
        let a = endpoint(low - self.padding)?;
        let b = endpoint(high + self.padding)?;
        let figure = novadraw::figure::PolylineFigure::new_with_color(
            a.x(),
            a.y(),
            b.x(),
            b.y(),
            self.color,
        )
        .and_then(|figure| figure.with_stroke_style(self.stroke.clone()))
        .map_err(|error| SnapError::extension("snap.reference-line.figure", error))?;
        Ok(vec![FeedbackVisual::unscaled(Box::new(figure))])
    }
}
