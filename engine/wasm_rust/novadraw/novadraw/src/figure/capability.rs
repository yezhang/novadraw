use std::{
    any::{Any, TypeId},
    collections::HashMap,
    error::Error,
    fmt,
    marker::PhantomData,
};

use crate::FigureId;

use super::{
    AccessibleFigure, BorderedFigure, ClickableBehavior, Figure, FigureContainer,
    FigureEventHandler, FigureLifecycle, FigurePreparation,
};
use crate::connection::{ConnectionDecorationBehavior, ConnectionFigureBehavior};

fn container_of<T>(figure: &dyn Figure) -> Option<&dyn FigureContainer>
where
    T: Figure + FigureContainer + 'static,
{
    figure
        .as_any()
        .downcast_ref::<T>()
        .map(|figure| figure as &dyn FigureContainer)
}

fn input_of<T>(figure: &dyn Figure) -> Option<&dyn FigureEventHandler>
where
    T: Figure + FigureEventHandler + 'static,
{
    figure
        .as_any()
        .downcast_ref::<T>()
        .map(|figure| figure as &dyn FigureEventHandler)
}

fn lifecycle_of<T>(figure: &mut dyn Figure) -> Option<&mut dyn FigureLifecycle>
where
    T: Figure + FigureLifecycle + 'static,
{
    figure
        .as_any_mut()
        .downcast_mut::<T>()
        .map(|figure| figure as &mut dyn FigureLifecycle)
}

fn accessibility_of<T>(figure: &dyn Figure) -> Option<&dyn AccessibleFigure>
where
    T: Figure + AccessibleFigure + 'static,
{
    figure
        .as_any()
        .downcast_ref::<T>()
        .map(|figure| figure as &dyn AccessibleFigure)
}

fn preparation_of<T>(figure: &dyn Figure) -> Option<&dyn FigurePreparation>
where
    T: Figure + FigurePreparation + 'static,
{
    figure
        .as_any()
        .downcast_ref::<T>()
        .map(|figure| figure as &dyn FigurePreparation)
}

fn border_of<T>(figure: &dyn Figure) -> Option<&dyn BorderedFigure>
where
    T: Figure + BorderedFigure + 'static,
{
    figure
        .as_any()
        .downcast_ref::<T>()
        .map(|figure| figure as &dyn BorderedFigure)
}

fn border_mut_of<T>(figure: &mut dyn Figure) -> Option<&mut dyn BorderedFigure>
where
    T: Figure + BorderedFigure + 'static,
{
    figure
        .as_any_mut()
        .downcast_mut::<T>()
        .map(|figure| figure as &mut dyn BorderedFigure)
}

fn clickable_of<T>(figure: &dyn Figure) -> Option<&dyn ClickableBehavior>
where
    T: Figure + ClickableBehavior + 'static,
{
    figure
        .as_any()
        .downcast_ref::<T>()
        .map(|figure| figure as &dyn ClickableBehavior)
}

fn clickable_mut_of<T>(figure: &mut dyn Figure) -> Option<&mut dyn ClickableBehavior>
where
    T: Figure + ClickableBehavior + 'static,
{
    figure
        .as_any_mut()
        .downcast_mut::<T>()
        .map(|figure| figure as &mut dyn ClickableBehavior)
}

fn connection_of<T>(figure: &dyn Figure) -> Option<&dyn ConnectionFigureBehavior>
where
    T: Figure + ConnectionFigureBehavior + 'static,
{
    figure
        .as_any()
        .downcast_ref::<T>()
        .map(|figure| figure as &dyn ConnectionFigureBehavior)
}

fn connection_mut_of<T>(figure: &mut dyn Figure) -> Option<&mut dyn ConnectionFigureBehavior>
where
    T: Figure + ConnectionFigureBehavior + 'static,
{
    figure
        .as_any_mut()
        .downcast_mut::<T>()
        .map(|figure| figure as &mut dyn ConnectionFigureBehavior)
}

fn decoration_of<T>(figure: &dyn Figure) -> Option<&dyn ConnectionDecorationBehavior>
where
    T: Figure + ConnectionDecorationBehavior + 'static,
{
    figure
        .as_any()
        .downcast_ref::<T>()
        .map(|figure| figure as &dyn ConnectionDecorationBehavior)
}

fn decoration_mut_of<T>(figure: &mut dyn Figure) -> Option<&mut dyn ConnectionDecorationBehavior>
where
    T: Figure + ConnectionDecorationBehavior + 'static,
{
    figure
        .as_any_mut()
        .downcast_mut::<T>()
        .map(|figure| figure as &mut dyn ConnectionDecorationBehavior)
}

/// Adapter for the container projection and child policy behavior.
#[derive(Clone, Copy)]
pub struct ContainerCapability {
    resolve: for<'a> fn(&'a dyn Figure) -> Option<&'a dyn FigureContainer>,
}

impl ContainerCapability {
    pub fn of<T>() -> Self
    where
        T: Figure + FigureContainer + 'static,
    {
        Self {
            resolve: container_of::<T>,
        }
    }

    pub(crate) fn resolve(self, figure: &dyn Figure) -> Option<&dyn FigureContainer> {
        (self.resolve)(figure)
    }
}

/// Adapter for input behavior.
#[derive(Clone, Copy)]
pub struct InputCapability {
    resolve: for<'a> fn(&'a dyn Figure) -> Option<&'a dyn FigureEventHandler>,
}

impl InputCapability {
    pub fn of<T>() -> Self
    where
        T: Figure + FigureEventHandler + 'static,
    {
        Self {
            resolve: input_of::<T>,
        }
    }

    pub(crate) fn resolve(self, figure: &dyn Figure) -> Option<&dyn FigureEventHandler> {
        (self.resolve)(figure)
    }
}

/// Adapter for attach, detach, validation, and invalidation behavior.
#[derive(Clone, Copy)]
pub struct LifecycleCapability {
    resolve: for<'a> fn(&'a mut dyn Figure) -> Option<&'a mut dyn FigureLifecycle>,
}

impl LifecycleCapability {
    pub fn of<T>() -> Self
    where
        T: Figure + FigureLifecycle + 'static,
    {
        Self {
            resolve: lifecycle_of::<T>,
        }
    }

    pub(crate) fn resolve(self, figure: &mut dyn Figure) -> Option<&mut dyn FigureLifecycle> {
        (self.resolve)(figure)
    }
}

/// Adapter for immutable accessibility projection behavior.
#[derive(Clone, Copy)]
pub struct AccessibilityCapability {
    resolve: for<'a> fn(&'a dyn Figure) -> Option<&'a dyn AccessibleFigure>,
}

impl AccessibilityCapability {
    pub fn of<T>() -> Self
    where
        T: Figure + AccessibleFigure + 'static,
    {
        Self {
            resolve: accessibility_of::<T>,
        }
    }

    pub(crate) fn resolve(self, figure: &dyn Figure) -> Option<&dyn AccessibleFigure> {
        (self.resolve)(figure)
    }
}

/// Adapter for immutable measure/arrange/paint preparation.
#[derive(Clone, Copy)]
pub struct PreparationCapability {
    resolve: for<'a> fn(&'a dyn Figure) -> Option<&'a dyn FigurePreparation>,
}

impl PreparationCapability {
    pub fn of<T>() -> Self
    where
        T: Figure + FigurePreparation + 'static,
    {
        Self {
            resolve: preparation_of::<T>,
        }
    }

    pub(crate) fn resolve(self, figure: &dyn Figure) -> Option<&dyn FigurePreparation> {
        (self.resolve)(figure)
    }
}

/// Adapter for Runtime-controlled Border replacement.
#[derive(Clone, Copy)]
pub struct BorderCapability {
    resolve: for<'a> fn(&'a dyn Figure) -> Option<&'a dyn BorderedFigure>,
    resolve_mut: for<'a> fn(&'a mut dyn Figure) -> Option<&'a mut dyn BorderedFigure>,
}

impl BorderCapability {
    pub fn of<T>() -> Self
    where
        T: Figure + BorderedFigure + 'static,
    {
        Self {
            resolve: border_of::<T>,
            resolve_mut: border_mut_of::<T>,
        }
    }

    pub(crate) fn resolve(self, figure: &dyn Figure) -> Option<&dyn BorderedFigure> {
        (self.resolve)(figure)
    }

    pub(crate) fn resolve_mut(self, figure: &mut dyn Figure) -> Option<&mut dyn BorderedFigure> {
        (self.resolve_mut)(figure)
    }
}

/// Adapter for button-like interaction state.
#[derive(Clone, Copy)]
pub struct ClickableCapability {
    resolve: for<'a> fn(&'a dyn Figure) -> Option<&'a dyn ClickableBehavior>,
    resolve_mut: for<'a> fn(&'a mut dyn Figure) -> Option<&'a mut dyn ClickableBehavior>,
}

impl ClickableCapability {
    pub fn of<T>() -> Self
    where
        T: Figure + ClickableBehavior + 'static,
    {
        Self {
            resolve: clickable_of::<T>,
            resolve_mut: clickable_mut_of::<T>,
        }
    }

    pub(crate) fn resolve(self, figure: &dyn Figure) -> Option<&dyn ClickableBehavior> {
        (self.resolve)(figure)
    }

    pub(crate) fn resolve_mut(self, figure: &mut dyn Figure) -> Option<&mut dyn ClickableBehavior> {
        (self.resolve_mut)(figure)
    }
}

/// Adapter for routed Connection geometry.
#[derive(Clone, Copy)]
pub struct ConnectionCapability {
    resolve: for<'a> fn(&'a dyn Figure) -> Option<&'a dyn ConnectionFigureBehavior>,
    resolve_mut: for<'a> fn(&'a mut dyn Figure) -> Option<&'a mut dyn ConnectionFigureBehavior>,
}

impl ConnectionCapability {
    pub fn of<T>() -> Self
    where
        T: Figure + ConnectionFigureBehavior + 'static,
    {
        Self {
            resolve: connection_of::<T>,
            resolve_mut: connection_mut_of::<T>,
        }
    }

    pub(crate) fn resolve(self, figure: &dyn Figure) -> Option<&dyn ConnectionFigureBehavior> {
        (self.resolve)(figure)
    }

    pub(crate) fn resolve_mut(
        self,
        figure: &mut dyn Figure,
    ) -> Option<&mut dyn ConnectionFigureBehavior> {
        (self.resolve_mut)(figure)
    }
}

/// Adapter for route-oriented Connection decorations.
#[derive(Clone, Copy)]
pub struct ConnectionDecorationCapability {
    resolve: for<'a> fn(&'a dyn Figure) -> Option<&'a dyn ConnectionDecorationBehavior>,
    resolve_mut: for<'a> fn(&'a mut dyn Figure) -> Option<&'a mut dyn ConnectionDecorationBehavior>,
}

impl ConnectionDecorationCapability {
    pub fn of<T>() -> Self
    where
        T: Figure + ConnectionDecorationBehavior + 'static,
    {
        Self {
            resolve: decoration_of::<T>,
            resolve_mut: decoration_mut_of::<T>,
        }
    }

    pub(crate) fn resolve(self, figure: &dyn Figure) -> Option<&dyn ConnectionDecorationBehavior> {
        (self.resolve)(figure)
    }

    pub(crate) fn resolve_mut(
        self,
        figure: &mut dyn Figure,
    ) -> Option<&mut dyn ConnectionDecorationBehavior> {
        (self.resolve_mut)(figure)
    }
}

/// Marker descriptor for Figures accepted by layered containers.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LayerCapability;

/// Marker descriptor for Figures whose extent is derived from descendants.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FreeformCapability;

/// Shared scale model descriptor.
#[derive(Clone, Debug)]
pub struct ScaleCapability {
    model: crate::container::ScaleModel,
}

impl ScaleCapability {
    pub fn new(model: crate::container::ScaleModel) -> Self {
        Self { model }
    }

    pub fn model(&self) -> &crate::container::ScaleModel {
        &self.model
    }
}

pub const LAYER: CapabilityKey<LayerCapability> = CapabilityKey::new("novadraw.layer");
pub const FREEFORM: CapabilityKey<FreeformCapability> = CapabilityKey::new("novadraw.freeform");
pub const SCALE: CapabilityKey<ScaleCapability> = CapabilityKey::new("novadraw.scale");
pub const CONTAINER: CapabilityKey<ContainerCapability> = CapabilityKey::new("novadraw.container");
pub const INPUT: CapabilityKey<InputCapability> = CapabilityKey::new("novadraw.input");
pub const LIFECYCLE: CapabilityKey<LifecycleCapability> = CapabilityKey::new("novadraw.lifecycle");
pub const ACCESSIBILITY: CapabilityKey<AccessibilityCapability> =
    CapabilityKey::new("novadraw.accessibility");
pub const PREPARATION: CapabilityKey<PreparationCapability> =
    CapabilityKey::new("novadraw.preparation");
pub const BORDER: CapabilityKey<BorderCapability> = CapabilityKey::new("novadraw.border");
pub const CLICKABLE: CapabilityKey<ClickableCapability> = CapabilityKey::new("novadraw.clickable");
pub const CONNECTION: CapabilityKey<ConnectionCapability> =
    CapabilityKey::new("novadraw.connection");
pub const CONNECTION_DECORATION: CapabilityKey<ConnectionDecorationCapability> =
    CapabilityKey::new("novadraw.connection-decoration");

/// A typed identity for one Figure capability descriptor.
///
/// Capability identity is the descriptor type `C`. The diagnostic name is
/// stable API text used only in errors and tooling.
pub struct CapabilityKey<C> {
    name: &'static str,
    descriptor: PhantomData<fn() -> C>,
}

impl<C> CapabilityKey<C> {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            descriptor: PhantomData,
        }
    }

    pub const fn name(self) -> &'static str {
        self.name
    }
}

impl<C> Clone for CapabilityKey<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> Copy for CapabilityKey<C> {}

impl<C> fmt::Debug for CapabilityKey<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CapabilityKey")
            .field(&self.name)
            .finish()
    }
}

struct CapabilityEntry {
    name: &'static str,
    descriptor: Box<dyn Any>,
}

/// Collects owned capability descriptors before a Figure is admitted.
#[derive(Default)]
pub struct FigureCapabilityBuilder {
    entries: HashMap<TypeId, CapabilityEntry>,
}

impl FigureCapabilityBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one owned descriptor.
    ///
    /// A descriptor type may occur at most once on a Figure.
    pub fn register<C: 'static>(
        &mut self,
        key: CapabilityKey<C>,
        descriptor: C,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        let type_id = TypeId::of::<C>();
        if let Some(existing) = self.entries.get(&type_id) {
            return Err(FigureCapabilityRegistrationError::DuplicateCapability {
                registered: existing.name,
                attempted: key.name,
            });
        }
        self.entries.insert(
            type_id,
            CapabilityEntry {
                name: key.name,
                descriptor: Box::new(descriptor),
            },
        );
        Ok(())
    }

    pub(crate) fn finish(self) -> FigureCapabilitySet {
        FigureCapabilitySet {
            entries: self.entries,
        }
    }
}

/// A capability registration error rejected before topology publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FigureCapabilityRegistrationError {
    DuplicateCapability {
        registered: &'static str,
        attempted: &'static str,
    },
}

impl fmt::Display for FigureCapabilityRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateCapability {
                registered,
                attempted,
            } => write!(
                formatter,
                "duplicate Figure capability descriptor: registered {registered}, attempted {attempted}"
            ),
        }
    }
}

impl Error for FigureCapabilityRegistrationError {}

/// Failure to query a typed Figure capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityQueryError {
    ForeignRuntime(FigureId),
    UnknownOrDisposedFigure(FigureId),
    DescriptorTypeMismatch { capability: &'static str },
}

impl fmt::Display for CapabilityQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignRuntime(figure) => {
                write!(formatter, "foreign Runtime Figure: {figure:?}")
            }
            Self::UnknownOrDisposedFigure(figure) => {
                write!(formatter, "unknown or disposed Figure ID: {figure:?}")
            }
            Self::DescriptorTypeMismatch { capability } => {
                write!(
                    formatter,
                    "Figure capability registry contains an invalid {capability} descriptor"
                )
            }
        }
    }
}

impl Error for CapabilityQueryError {}

/// The immutable descriptor registry owned by one attached Figure node.
pub(crate) struct FigureCapabilitySet {
    entries: HashMap<TypeId, CapabilityEntry>,
}

impl FigureCapabilitySet {
    pub(crate) fn build(figure: &dyn Figure) -> Result<Self, FigureCapabilityRegistrationError> {
        let mut builder = FigureCapabilityBuilder::new();
        figure.register_capabilities(&mut builder)?;
        Ok(builder.finish())
    }

    pub(crate) fn get<C: 'static>(
        &self,
        key: CapabilityKey<C>,
    ) -> Result<Option<&C>, CapabilityQueryError> {
        let Some(entry) = self.entries.get(&TypeId::of::<C>()) else {
            return Ok(None);
        };
        entry.descriptor.downcast_ref::<C>().map(Some).ok_or(
            CapabilityQueryError::DescriptorTypeMismatch {
                capability: key.name,
            },
        )
    }
}
