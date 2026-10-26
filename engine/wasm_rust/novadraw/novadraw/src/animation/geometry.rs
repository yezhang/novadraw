use std::{any::Any, sync::Arc};

use crate::{
    Affine2D, Figure, Rectangle,
    geometry::{Insets, Point},
};

use super::{AnimationError, AnimationValue};

/// Bounds in the parent's child-content domain. Size never introduces an affine scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutBounds(Rectangle);

impl LayoutBounds {
    /// Accepts finite, nonnegative sizes, including zero.
    pub fn try_new(bounds: Rectangle) -> Result<Self, AnimationError> {
        if bounds.is_valid() {
            Ok(Self(bounds))
        } else {
            Err(AnimationError::InvalidValue)
        }
    }

    /// Returns the parent-content rectangle.
    pub fn rectangle(self) -> Rectangle {
        self.0
    }
}

impl AnimationValue for LayoutBounds {
    fn is_valid(&self) -> bool {
        self.0.is_valid()
    }

    fn interpolate(&self, target: &Self, progress: f64) -> Self {
        Self(self.0.interpolate(&target.0, progress))
    }
}

/// Local presentation visibility. Ancestor visibility still applies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PresentedVisibility(pub bool);

impl AnimationValue for PresentedVisibility {
    fn is_valid(&self) -> bool {
        true
    }

    fn interpolate(&self, target: &Self, progress: f64) -> Self {
        if progress < 1.0 { *self } else { *target }
    }
}

/// A viewport origin in the RangeModel domain, independent of contents transforms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportOrigin(pub Point);

impl AnimationValue for ViewportOrigin {
    fn is_valid(&self) -> bool {
        self.0.x().is_finite() && self.0.y().is_finite()
    }

    fn interpolate(&self, target: &Self, progress: f64) -> Self {
        Self(self.0.interpolate(&target.0, progress))
    }
}

impl GeometryBinding<ViewportOrigin> {
    /// Replaces only the viewport's scroll translation. The contents owns its scale.
    pub fn viewport_origin(content_scale: f64) -> Result<Self, AnimationError> {
        if !content_scale.is_finite() || content_scale <= 0.0 {
            return Err(AnimationError::InvalidValue);
        }
        Ok(Self::new(
            move |_, inputs| {
                let offset = inputs.child_transform.transform_point(Point::ZERO);
                Ok(ViewportOrigin(Point::new(
                    -offset.x() / content_scale,
                    -offset.y() / content_scale,
                )))
            },
            move |value| {
                Ok(GeometryPatch {
                    child_transform: Some(Affine2D::from_translation(
                        -value.0.x() * content_scale,
                        -value.0.y() * content_scale,
                    )),
                    ..Default::default()
                })
            },
        ))
    }
}

/// Frozen committed inputs supplied to a geometry descriptor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeometryInputs {
    /// Committed parent-content bounds.
    pub bounds: Rectangle,
    /// Committed local visibility.
    pub visible: bool,
    /// Frozen border/client insets.
    pub insets: Insets,
    /// Container transform without the client-origin translation.
    pub child_transform: Affine2D,
}

/// One descriptor's output. Absent fields leave the corresponding geometry unchanged.
///
/// Descriptors with distinct semantic value types may coexist, but two writers to the same
/// geometry field are rejected. Diagnostic names never participate in identity.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GeometryPatch {
    /// Parent-content bounds, without size scaling.
    pub bounds: Option<Rectangle>,
    /// Local visibility; ancestor conjunction remains mandatory.
    pub visible: Option<bool>,
    /// Explicit additional transform after node placement.
    pub local_transform: Option<Affine2D>,
    /// Replaces the container transform, preserving the frozen client-origin translation.
    pub child_transform: Option<Affine2D>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct GeometryFields {
    bounds: bool,
    visible: bool,
    pub(crate) local_transform: bool,
    pub(crate) child_transform: bool,
}

impl GeometryFields {
    pub(crate) fn union(self, other: Self) -> Self {
        Self {
            bounds: self.bounds || other.bounds,
            visible: self.visible || other.visible,
            local_transform: self.local_transform || other.local_transform,
            child_transform: self.child_transform || other.child_transform,
        }
    }

    pub(crate) fn overlaps(self, other: Self) -> bool {
        (self.bounds && other.bounds)
            || (self.visible && other.visible)
            || (self.local_transform && other.local_transform)
            || (self.child_transform && other.child_transform)
    }
}

impl GeometryPatch {
    pub(crate) fn fields(self) -> GeometryFields {
        GeometryFields {
            bounds: self.bounds.is_some(),
            visible: self.visible.is_some(),
            local_transform: self.local_transform.is_some(),
            child_transform: self.child_transform.is_some(),
        }
    }
    pub(crate) fn validate(self) -> Result<Self, AnimationError> {
        if self.bounds.is_some_and(|value| !value.is_valid())
            || self.local_transform.is_some_and(|value| !value.is_valid())
            || self.child_transform.is_some_and(|value| !value.is_valid())
        {
            return Err(AnimationError::InvalidValue);
        }
        Ok(self)
    }

    pub(crate) fn merge(&mut self, other: Self) -> Result<(), AnimationError> {
        fn field<T: Copy>(into: &mut Option<T>, value: Option<T>) -> Result<(), AnimationError> {
            if into.is_some() && value.is_some() {
                return Err(AnimationError::ChannelConflict);
            }
            if value.is_some() {
                *into = value;
            }
            Ok(())
        }
        field(&mut self.bounds, other.bounds)?;
        field(&mut self.visible, other.visible)?;
        field(&mut self.local_transform, other.local_transform)?;
        field(&mut self.child_transform, other.child_transform)
    }
}

type ReadGeometry<V> =
    dyn Fn(&dyn Figure, GeometryInputs) -> Result<V, AnimationError> + Send + Sync;
type ApplyGeometry<V> = dyn Fn(&V) -> Result<GeometryPatch, AnimationError> + Send + Sync;

/// Typed adapter from committed inputs and cached Motion values to presentation geometry.
///
/// The semantic identity is `V`, shared by registration, channel ownership and queries. External
/// consumers use this same constructor as the standard bounds and visibility descriptors.
/// Values use an owned `Copy` representation so frozen geometry cannot retain opaque heap owners.
/// Readers/adapters capture only fixed `Copy` configuration, charged with each active track.
/// Referenced inputs must remain immutable and must not hide owned heap storage.
pub struct GeometryBinding<V> {
    pub(crate) committed: Arc<ReadGeometry<V>>,
    apply: Arc<ApplyGeometry<V>>,
    callback_bytes: usize,
}

impl<V> Clone for GeometryBinding<V> {
    fn clone(&self) -> Self {
        Self {
            committed: self.committed.clone(),
            apply: self.apply.clone(),
            callback_bytes: self.callback_bytes,
        }
    }
}

impl<V: AnimationValue + Copy> GeometryBinding<V> {
    /// Constructs a descriptor using a pure source reader and sampled-value adapter.
    pub fn new(
        committed: impl Fn(&dyn Figure, GeometryInputs) -> Result<V, AnimationError>
        + Copy
        + Send
        + Sync
        + 'static,
        apply: impl Fn(&V) -> Result<GeometryPatch, AnimationError> + Copy + Send + Sync + 'static,
    ) -> Self {
        let callback_bytes = std::mem::size_of_val(&committed) + std::mem::size_of_val(&apply);
        Self {
            committed: Arc::new(committed),
            apply: Arc::new(apply),
            callback_bytes,
        }
    }

    pub(crate) fn erased(&self) -> Arc<dyn ErasedGeometryBinding> {
        Arc::new(self.clone())
    }
}

impl GeometryBinding<LayoutBounds> {
    /// Standard bounds adapter, using the same protocol as external semantic values.
    pub fn layout_bounds() -> Self {
        Self::new(
            |_, inputs| LayoutBounds::try_new(inputs.bounds),
            |value| {
                Ok(GeometryPatch {
                    bounds: Some(value.rectangle()),
                    ..GeometryPatch::default()
                })
            },
        )
    }
}

impl GeometryBinding<PresentedVisibility> {
    /// Standard local visibility adapter.
    pub fn visibility() -> Self {
        Self::new(
            |_, inputs| Ok(PresentedVisibility(inputs.visible)),
            |value| {
                Ok(GeometryPatch {
                    visible: Some(value.0),
                    ..GeometryPatch::default()
                })
            },
        )
    }
}

pub(crate) trait ErasedGeometryBinding: Send + Sync {
    fn storage_bytes(&self) -> usize;
    fn patch(&self, value: &dyn Any) -> Result<GeometryPatch, AnimationError>;
    fn clone_value(&self, value: &dyn Any) -> Result<Arc<dyn Any + Send + Sync>, AnimationError>;
}

impl<V: AnimationValue + Copy> ErasedGeometryBinding for GeometryBinding<V> {
    fn storage_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.callback_bytes + 6 * std::mem::size_of::<usize>()
    }
    fn patch(&self, value: &dyn Any) -> Result<GeometryPatch, AnimationError> {
        let value = value
            .downcast_ref::<V>()
            .ok_or(AnimationError::InvalidValue)?;
        (self.apply)(value)?.validate()
    }

    fn clone_value(&self, value: &dyn Any) -> Result<Arc<dyn Any + Send + Sync>, AnimationError> {
        Ok(Arc::new(
            *value
                .downcast_ref::<V>()
                .ok_or(AnimationError::InvalidValue)?,
        ))
    }
}

pub(crate) fn inputs(node: &crate::graph::FigureNode) -> GeometryInputs {
    GeometryInputs {
        bounds: node.figure_bounds(),
        visible: node.is_visible,
        insets: node.state().insets,
        child_transform: Affine2D::from_translation(
            -node.state().insets.left,
            -node.state().insets.top,
        ) * node.child_transform().affine(),
    }
}
