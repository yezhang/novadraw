//! Host-owned providers and frozen background inputs. No provider runs in Core.
use crate::{
    advanced::RuntimeNamespace,
    geometry::{Affine2D, Rectangle},
    graphics::Color,
    render::{RecordedDrawing, SurfaceInfo},
};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt,
    marker::PhantomData,
    sync::Arc,
};

/// Independent versions for the surface extent, Canvas placement and DPI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackgroundInputVersions {
    pub surface: u64,
    pub position: u64,
    pub dpi: u64,
}

/// Host coordinates frozen before frame preparation; viewport origin is excluded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackgroundFrameInput {
    pub surface: SurfaceInfo,
    pub versions: BackgroundInputVersions,
    pub host_to_canvas: Affine2D,
}

impl BackgroundFrameInput {
    pub(crate) fn validate(self, previous: Option<Self>) -> Result<(), HostBackgroundError> {
        let v = self.versions;
        if v.surface == 0
            || v.position == 0
            || v.dpi == 0
            || !self.surface.logical_width.is_finite()
            || self.surface.logical_width < 0.0
            || !self.surface.logical_height.is_finite()
            || self.surface.logical_height < 0.0
            || !self.surface.scale_factor.is_finite()
            || self.surface.scale_factor <= 0.0
            || self.host_to_canvas.coeffs().iter().any(|c| !c.is_finite())
            || !matches!(self.host_to_canvas.checked_inverse(), Ok(Some(_)))
        {
            return Err(HostBackgroundError::InvalidInput);
        }
        if let Some(old) = previous
            && (v.surface < old.versions.surface
                || v.position < old.versions.position
                || v.dpi < old.versions.dpi
                || (self.surface != old.surface && v.surface == old.versions.surface)
                || (self.surface.scale_factor != old.surface.scale_factor
                    && v.dpi == old.versions.dpi)
                || (self.host_to_canvas != old.host_to_canvas
                    && v.position == old.versions.position))
        {
            return Err(HostBackgroundError::StaleInput);
        }
        Ok(())
    }
}

/// Fresh provider namespace; it cannot alias another producer's imported resources.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HostBackgroundId(pub(crate) RuntimeNamespace);
impl HostBackgroundId {
    pub fn new() -> Self {
        Self(RuntimeNamespace::new())
    }
}
impl Default for HostBackgroundId {
    fn default() -> Self {
        Self::new()
    }
}

/// Runtime-issued request. Removal, rebinding and a newer request retire old tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostBackgroundRequest {
    pub(crate) destination: RuntimeNamespace,
    pub(crate) source: HostBackgroundId,
    pub(crate) generation: u64,
    pub(crate) request: u64,
    pub(crate) input: BackgroundFrameInput,
}
impl HostBackgroundRequest {
    pub fn source(self) -> HostBackgroundId {
        self.source
    }
    pub fn generation(self) -> u64 {
        self.generation
    }
    pub fn request_generation(self) -> u64 {
        self.request
    }
    pub fn input(self) -> BackgroundFrameInput {
        self.input
    }
}

/// Provider result in host coordinates, with an exact immutable resource closure.
#[derive(Clone, Debug, PartialEq)]
pub struct HostBackgroundContent {
    pub coverage: Rectangle,
    pub drawing: RecordedDrawing,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostBackgroundSnapshot {
    pub(crate) request: HostBackgroundRequest,
    pub(crate) revision: u64,
    pub(crate) content: HostBackgroundContent,
}
impl HostBackgroundSnapshot {
    pub fn new(
        request: HostBackgroundRequest,
        revision: u64,
        content: HostBackgroundContent,
    ) -> Result<Self, HostBackgroundError> {
        let r = content.coverage;
        if revision == 0
            || !r.x.is_finite()
            || !r.y.is_finite()
            || !r.width.is_finite()
            || !r.height.is_finite()
            || r.width < 0.0
            || r.height < 0.0
            || !(r.x + r.width).is_finite()
            || !(r.y + r.height).is_finite()
        {
            return Err(HostBackgroundError::InvalidInput);
        }
        Ok(Self {
            request,
            revision,
            content,
        })
    }
    pub fn request(&self) -> HostBackgroundRequest {
        self.request
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn content(&self) -> &HostBackgroundContent {
        &self.content
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum HostBackgroundError {
    Faulted,
    Exhausted,
    StaleRequest,
    StaleInput,
    InputMismatch,
    InvalidInput,
    SnapshotConflict,
    DuplicateProvider,
    UnknownProvider,
    Provider(String),
    /// Boxed because SceneError can itself contain a frame preparation error.
    Import(Box<crate::scene::SceneError>),
}
impl fmt::Display for HostBackgroundError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host background: {self:?}")
    }
}
impl std::error::Error for HostBackgroundError {}

/// Typed provider identity; built-in and external providers use the same registry.
pub struct HostBackgroundProviderKey<P> {
    namespace: &'static str,
    name: &'static str,
    marker: PhantomData<fn() -> P>,
}
impl<P> HostBackgroundProviderKey<P> {
    pub const fn new(namespace: &'static str, name: &'static str) -> Self {
        assert!(!namespace.is_empty() && !name.is_empty());
        Self {
            namespace,
            name,
            marker: PhantomData,
        }
    }
}
impl<P> Copy for HostBackgroundProviderKey<P> {}
impl<P> Clone for HostBackgroundProviderKey<P> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Called by the host, never by Runtime preparation or an engine timer.
pub trait HostBackgroundProvider<P>: 'static {
    fn provide(
        &self,
        payload: &P,
        request: HostBackgroundRequest,
    ) -> Result<HostBackgroundContent, HostBackgroundError>;
}
type ProviderIdentity = (&'static str, &'static str, TypeId);
type ProviderCallback =
    dyn Fn(&dyn Any, HostBackgroundRequest) -> Result<HostBackgroundContent, HostBackgroundError>;

pub const RECORDED_BACKGROUND: HostBackgroundProviderKey<HostBackgroundContent> =
    HostBackgroundProviderKey::new("novadraw.host", "recorded-background");
struct RecordedProvider;
impl HostBackgroundProvider<HostBackgroundContent> for RecordedProvider {
    fn provide(
        &self,
        payload: &HostBackgroundContent,
        _: HostBackgroundRequest,
    ) -> Result<HostBackgroundContent, HostBackgroundError> {
        Ok(payload.clone())
    }
}

pub struct HostBackgroundRegistry {
    providers: HashMap<ProviderIdentity, Box<ProviderCallback>>,
}
impl Default for HostBackgroundRegistry {
    fn default() -> Self {
        let mut registry = Self {
            providers: HashMap::new(),
        };
        registry
            .register(RECORDED_BACKGROUND, RecordedProvider)
            .expect("empty registry");
        registry
    }
}
impl HostBackgroundRegistry {
    pub fn register<P: 'static>(
        &mut self,
        key: HostBackgroundProviderKey<P>,
        provider: impl HostBackgroundProvider<P>,
    ) -> Result<(), HostBackgroundError> {
        let identity = (key.namespace, key.name, TypeId::of::<P>());
        if self.providers.contains_key(&identity) {
            return Err(HostBackgroundError::DuplicateProvider);
        }
        self.providers.insert(
            identity,
            Box::new(move |payload, request| {
                provider.provide(
                    payload
                        .downcast_ref::<P>()
                        .ok_or(HostBackgroundError::UnknownProvider)?,
                    request,
                )
            }),
        );
        Ok(())
    }
    pub fn provide<P: 'static>(
        &self,
        key: HostBackgroundProviderKey<P>,
        payload: &P,
        request: HostBackgroundRequest,
        revision: u64,
    ) -> Result<HostBackgroundSnapshot, HostBackgroundError> {
        let callback = self
            .providers
            .get(&(key.namespace, key.name, TypeId::of::<P>()))
            .ok_or(HostBackgroundError::UnknownProvider)?;
        HostBackgroundSnapshot::new(request, revision, callback(payload, request)?)
    }
}

pub(crate) struct InstalledBackground {
    pub(crate) snapshot: HostBackgroundSnapshot,
    pub(crate) drawing: RecordedDrawing,
    pub(crate) _leases: Vec<Arc<crate::scene::import::ImportLease>>,
}

#[derive(Default)]
pub(crate) struct BackgroundState {
    pub(crate) generation: u64,
    pub(crate) next_request: u64,
    pub(crate) source: Option<HostBackgroundId>,
    pub(crate) request: Option<HostBackgroundRequest>,
    pub(crate) input: Option<BackgroundFrameInput>,
    pub(crate) base: Option<Color>,
    pub(crate) current: Option<InstalledBackground>,
    pub(crate) error: Option<HostBackgroundError>,
    pub(crate) waker: Option<std::task::Waker>,
}
