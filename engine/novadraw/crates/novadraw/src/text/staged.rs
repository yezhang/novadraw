//! Optional staged shaping shared by built-in and external engines.
use std::{
    any::{Any, TypeId},
    cell::RefCell,
    error::Error,
    fmt,
    marker::PhantomData,
    rc::Rc,
    sync::Arc,
};

use super::shaping::{TextLayoutCacheIdentity, TextLayoutEngine};
use super::{StyledTextLayout, StyledTextRequest, TextLayoutDiagnostics};
use crate::figure::{PreparationBudgetError, PreparationBudgetHandle};

mod registry;
pub use registry::{
    TextAnalysisAdmissionCost, TextAnalysisBacking, TextAnalysisBackingIdentity,
    TextAnalysisCacheLimits, TextAnalysisLease, TextAnalysisPayload, TextAnalysisRegistry,
    TextAnalysisStorage, TextAnalysisUsage,
};

/// Complete checked input with only the width constraint removed.
#[derive(Debug, PartialEq)]
pub struct TextAnalysisRequest(StyledTextRequest);

impl TextAnalysisRequest {
    pub fn new(request: &StyledTextRequest) -> Result<Self, TextStageError> {
        Ok(Self(request.try_analysis_copy()?))
    }

    pub fn request(&self) -> &StyledTextRequest {
        &self.0
    }

    pub(super) fn storage_bytes(&self) -> Result<usize, TextStageError> {
        self.0
            .analysis_storage_bytes()
            .and_then(|bytes| bytes.checked_add(RC_COUNTER_BYTES))
            .ok_or(TextStageError::AccountingOverflow)
    }

    pub(super) fn comparison_work(&self) -> Result<u64, TextStageError> {
        // Font size/weight/style and RGBA components; source range endpoints;
        // inline id/offset/width/height; five paragraph/constraint values.
        const STYLE_SCALARS: usize = 7;
        const SPAN_RANGE_SCALARS: usize = 2;
        const INLINE_SCALARS: usize = 4;
        const REQUEST_SCALARS: usize = 5;
        let request = self.request();
        let mut work = request
            .text()
            .len()
            .checked_add(request.default_style().font().family().len())
            .and_then(|n| n.checked_add(STYLE_SCALARS + REQUEST_SCALARS))
            .and_then(|n| {
                n.checked_add(
                    request
                        .spans()
                        .len()
                        .checked_mul(STYLE_SCALARS + SPAN_RANGE_SCALARS)?,
                )
            })
            .and_then(|n| n.checked_add(request.inline_boxes().len().checked_mul(INLINE_SCALARS)?))
            .ok_or(TextStageError::AccountingOverflow)?;
        for span in request.spans() {
            work = work
                .checked_add(span.style.font().family().len())
                .ok_or(TextStageError::AccountingOverflow)?;
        }
        u64::try_from(work).map_err(|_| TextStageError::AccountingOverflow)
    }

    pub(super) fn copy_plan(&self) -> Result<TextStageCostPlan, TextStageError> {
        request_copy_plan(self.request())
    }
}

fn request_copy_plan(request: &StyledTextRequest) -> Result<TextStageCostPlan, TextStageError> {
    let mut allocation = request
        .default_style()
        .font()
        .family()
        .len()
        .checked_add(request.spans().len())
        .and_then(|n| n.checked_add(request.inline_boxes().len()))
        .and_then(|n| n.checked_add(1))
        .ok_or(TextStageError::AccountingOverflow)?;
    for span in request.spans() {
        allocation = allocation
            .checked_add(span.style.font().family().len())
            .ok_or(TextStageError::AccountingOverflow)?;
    }
    let work = request
        .text()
        .len()
        .checked_add(allocation)
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(TextStageError::AccountingOverflow)?;
    Ok(TextStageCostPlan {
        work,
        allocation: u64::try_from(allocation).map_err(|_| TextStageError::AccountingOverflow)?,
    })
}

// Rc's strong and weak counters; allocator overhead is outside logical storage accounting.
const RC_COUNTER_BYTES: usize = 2 * std::mem::size_of::<usize>();

fn rc_allocation_bytes<T>() -> Result<usize, TextStageError> {
    std::alloc::Layout::new::<[usize; 2]>()
        .extend(std::alloc::Layout::new::<T>())
        .map(|(layout, _)| layout.pad_to_align().size())
        .map_err(|_| TextStageError::AccountingOverflow)
}

/// Type identity determines behavior; the name is only diagnostic.
pub struct TextAnalysisKey<A: TextAnalysisPayload> {
    name: &'static str,
    marker: PhantomData<fn() -> A>,
}

impl<A: TextAnalysisPayload> Copy for TextAnalysisKey<A> {}
impl<A: TextAnalysisPayload> Clone for TextAnalysisKey<A> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<A: TextAnalysisPayload> TextAnalysisKey<A> {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            marker: PhantomData,
        }
    }
    pub fn erase(self) -> ErasedTextAnalysisKey {
        ErasedTextAnalysisKey {
            type_id: TypeId::of::<A>(),
            name: self.name,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ErasedTextAnalysisKey {
    type_id: TypeId,
    name: &'static str,
}
impl PartialEq for ErasedTextAnalysisKey {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id
    }
}
impl Eq for ErasedTextAnalysisKey {}
impl ErasedTextAnalysisKey {
    pub fn diagnostic_name(self) -> &'static str {
        self.name
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextAnalysisRegistryIdentity(uuid::Uuid);

/// Core captures this value; extensions can read but cannot construct it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextStageBinding {
    engine: TextLayoutCacheIdentity,
    revision: u64,
    registry: TextAnalysisRegistryIdentity,
    key: ErasedTextAnalysisKey,
}
impl TextStageBinding {
    pub fn engine(self) -> TextLayoutCacheIdentity {
        self.engine
    }
    pub fn revision(self) -> u64 {
        self.revision
    }
    pub fn registry(self) -> TextAnalysisRegistryIdentity {
        self.registry
    }
    pub fn key(self) -> ErasedTextAnalysisKey {
        self.key
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextStageBindingChange {
    pub captured: TextStageBinding,
    /// None means that cache identity or stage capability disappeared.
    pub returned: Option<TextStageBinding>,
}
impl TextStageBindingChange {
    pub fn capability_disappeared(&self) -> bool {
        self.returned.is_none()
    }
    pub fn engine_changed(&self) -> bool {
        self.returned
            .is_some_and(|value| value.engine != self.captured.engine)
    }
    pub fn revision_changed(&self) -> bool {
        self.returned
            .is_some_and(|value| value.revision != self.captured.revision)
    }
    pub fn registry_changed(&self) -> bool {
        self.returned
            .is_some_and(|value| value.registry != self.captured.registry)
    }
    pub fn key_changed(&self) -> bool {
        self.returned
            .is_some_and(|value| value.key != self.captured.key)
    }
}
impl fmt::Display for TextStageBindingChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "text stage binding changed: {:?} -> {:?}",
            self.captured, self.returned
        )
    }
}
impl Error for TextStageBindingChange {}

/// Owned original extension error. Equality means the same captured error object.
#[derive(Clone)]
pub struct TextStageExtensionError(Arc<dyn Error + Send + Sync>);
impl TextStageExtensionError {
    pub fn new(error: impl Error + Send + Sync + 'static) -> Self {
        Self(Arc::new(error))
    }
    pub fn error(&self) -> &(dyn Error + 'static) {
        self.0.as_ref()
    }
}
impl PartialEq for TextStageExtensionError {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for TextStageExtensionError {}
impl fmt::Debug for TextStageExtensionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::Display for TextStageExtensionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Error for TextStageExtensionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.error())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextStageError {
    EngineIdentityMismatch,
    RegistryIdentityMismatch,
    KeyMismatch,
    RevisionMismatch,
    InputMismatch,
    InvalidOutput,
    InvalidStorageDescription,
    BackingDescriptionConflict,
    AccountingOverflow,
    StorageReservation,
    CacheMetadataLimit,
    ReflowAccountingUnavailable,
    Budget(PreparationBudgetError),
    Extension(TextStageExtensionError),
}
impl TextStageError {
    pub fn extension(error: impl Error + Send + Sync + 'static) -> Self {
        Self::Extension(TextStageExtensionError::new(error))
    }
}
impl fmt::Display for TextStageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Budget(error) => error.fmt(f),
            Self::Extension(error) => error.fmt(f),
            _ => write!(f, "text stage error: {self:?}"),
        }
    }
}
impl Error for TextStageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Budget(error) => Some(error),
            Self::Extension(error) => Some(error.error()),
            _ => None,
        }
    }
}
impl From<PreparationBudgetError> for TextStageError {
    fn from(error: PreparationBudgetError) -> Self {
        Self::Budget(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextStageFailureKind {
    Budget,
    Binding,
    Stage,
    Validation,
}

/// All observed failures survive independently of diagnostic settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextStageFailure {
    pub primary: TextStageFailureKind,
    pub budget_failure: Option<PreparationBudgetError>,
    pub binding_failure: Option<TextStageBindingChange>,
    pub stage_error: Option<TextStageError>,
    pub validation_error: Option<TextStageError>,
}
impl fmt::Display for TextStageFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "text stage failed ({:?})", self.primary)
    }
}
impl Error for TextStageFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Some(error) = &self.stage_error {
            return Some(error);
        }
        match self.primary {
            TextStageFailureKind::Budget => self.budget_failure.as_ref().map(|e| e as &dyn Error),
            TextStageFailureKind::Binding => self.binding_failure.as_ref().map(|e| e as &dyn Error),
            TextStageFailureKind::Validation => {
                self.validation_error.as_ref().map(|e| e as &dyn Error)
            }
            TextStageFailureKind::Stage => None,
        }
    }
}

/// Logical operation units. Allocation units also incur work in the shared budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextStageCostPlan {
    pub work: u64,
    pub allocation: u64,
}

/// A session handle can be reused for standalone calls; binding is supplied by Core.
#[derive(Clone)]
pub struct TextStageContext {
    budget: PreparationBudgetHandle,
    diagnostics: TextLayoutDiagnostics,
    call: Option<(TextStageBinding, Rc<TextAnalysisRequest>)>,
}
impl TextStageContext {
    pub fn new(budget: PreparationBudgetHandle, diagnostics: TextLayoutDiagnostics) -> Self {
        Self {
            budget,
            diagnostics,
            call: None,
        }
    }
    pub fn budget(&self) -> &PreparationBudgetHandle {
        &self.budget
    }
    pub fn diagnostics(&self) -> &TextLayoutDiagnostics {
        &self.diagnostics
    }
    pub fn binding(&self) -> Option<TextStageBinding> {
        self.call.as_ref().map(|call| call.0)
    }
    pub fn analysis_request(&self) -> Option<&TextAnalysisRequest> {
        self.call.as_ref().map(|call| call.1.as_ref())
    }
    pub fn charge(
        &self,
        stage: &'static str,
        plan: TextStageCostPlan,
    ) -> Result<(), TextStageError> {
        self.budget.consume(stage, plan.work)?;
        self.budget.allocate(stage, plan.allocation)?;
        Ok(())
    }
    fn bound(&self, binding: TextStageBinding, request: TextAnalysisRequest) -> Self {
        Self {
            call: Some((binding, Rc::new(request))),
            ..self.clone()
        }
    }
}

pub trait StagedTextLayoutEngine {
    fn analysis_key(&self) -> ErasedTextAnalysisKey;
    fn analysis_registry(&self) -> &TextAnalysisRegistry;
    fn analysis_admission_lower_bound(
        &self,
        _request: &TextAnalysisRequest,
        _context: &TextStageContext,
    ) -> Result<Option<TextAnalysisAdmissionCost>, TextStageError> {
        Ok(None)
    }
    fn analyze(
        &mut self,
        request: &TextAnalysisRequest,
        context: &TextStageContext,
    ) -> Result<Option<TextAnalysisLease>, TextStageError>;
    fn reflow(
        &mut self,
        analysis: &TextAnalysisLease,
        request: &StyledTextRequest,
        context: &TextStageContext,
    ) -> Result<StyledTextLayout, TextStageError>;
}

fn capture(engine: &mut dyn TextLayoutEngine) -> Option<TextStageBinding> {
    let identity = engine.cache_identity()?;
    let revision = engine.revision();
    let staged = engine.staged_layout()?;
    Some(TextStageBinding {
        engine: identity,
        revision,
        registry: staged.analysis_registry().identity(),
        key: staged.analysis_key(),
    })
}

fn checked<T>(
    engine: &mut dyn TextLayoutEngine,
    context: &TextStageContext,
    captured: TextStageBinding,
    result: Result<T, TextStageError>,
) -> Result<T, super::TextError> {
    let returned = capture(engine);
    let budget_failure = context.budget.check().err();
    let binding_failure =
        (returned != Some(captured)).then_some(TextStageBindingChange { captured, returned });
    let stage_error = result.as_ref().err().cloned();
    let primary = if budget_failure.is_some() {
        Some(TextStageFailureKind::Budget)
    } else if binding_failure.is_some() {
        Some(TextStageFailureKind::Binding)
    } else if stage_error.is_some() {
        Some(TextStageFailureKind::Stage)
    } else {
        None
    };
    if let Some(primary) = primary {
        return Err(super::TextError::Stage(Box::new(TextStageFailure {
            primary,
            budget_failure,
            binding_failure,
            stage_error,
            validation_error: None,
        })));
    }
    result.map_err(validation)
}

fn validation(error: TextStageError) -> super::TextError {
    super::TextError::Stage(Box::new(TextStageFailure {
        primary: TextStageFailureKind::Validation,
        budget_failure: None,
        binding_failure: None,
        stage_error: None,
        validation_error: Some(error),
    }))
}

fn rejected(session: &TextStageContext, error: TextStageError) -> super::TextError {
    let budget_failure = session.budget.check().err();
    super::TextError::Stage(Box::new(TextStageFailure {
        primary: if budget_failure.is_some() {
            TextStageFailureKind::Budget
        } else {
            TextStageFailureKind::Validation
        },
        budget_failure,
        binding_failure: None,
        stage_error: None,
        validation_error: Some(error),
    }))
}

pub(super) fn layout(
    engine: &mut dyn TextLayoutEngine,
    request: &StyledTextRequest,
    session: &TextStageContext,
) -> Result<StyledTextLayout, super::TextError> {
    session
        .budget
        .check()
        .map_err(|error| rejected(session, error.into()))?;
    let Some(binding) = capture(engine) else {
        return full(engine, request, session);
    };
    let staged = engine
        .staged_layout()
        .ok_or_else(|| validation(TextStageError::RegistryIdentityMismatch))?;
    let registry = staged.analysis_registry();
    if registry.engine() != binding.engine {
        return Err(validation(TextStageError::EngineIdentityMismatch));
    }
    if !registry.is_enabled() {
        return full(engine, request, session);
    }
    // Projection scans and copies all request-owned collections before any stage callback.
    session
        .charge(
            "text-analysis-request",
            request_copy_plan(request).map_err(validation)?,
        )
        .map_err(|error| rejected(session, error))?;
    let analysis = TextAnalysisRequest::new(request).map_err(validation)?;
    let context = session.bound(binding, analysis);
    let analysis = context
        .analysis_request()
        .ok_or_else(|| validation(TextStageError::InputMismatch))?;
    let hit = registry
        .find(&context)
        .map_err(|error| rejected(&context, error))?;
    let lease = if let Some(hit) = hit {
        hit
    } else {
        let result = staged.analysis_admission_lower_bound(analysis, &context);
        let lower = checked(engine, &context, binding, result)?;
        let staged = engine
            .staged_layout()
            .ok_or_else(|| validation(TextStageError::RegistryIdentityMismatch))?;
        let refused = lower
            .map(|cost| {
                staged
                    .analysis_registry()
                    .definitely_exceeds(&context, cost)
            })
            .transpose()
            .map_err(|error| rejected(&context, error))?
            .unwrap_or(false);
        if refused {
            return full(engine, request, &context);
        }
        let result = staged.analyze(analysis, &context);
        let Some(lease) = checked(engine, &context, binding, result)? else {
            return full(engine, request, &context);
        };
        lease
    };
    let staged = engine
        .staged_layout()
        .ok_or_else(|| validation(TextStageError::RegistryIdentityMismatch))?;
    staged
        .analysis_registry()
        .validate(&lease, &context)
        .map_err(|error| rejected(&context, error))?;
    let result = staged.reflow(&lease, request, &context);
    let output = checked(engine, &context, binding, result)?;
    if output.request() != request {
        return Err(validation(TextStageError::InvalidOutput));
    }
    if output.layout().key().engine_revision() != binding.revision {
        return Err(validation(TextStageError::RevisionMismatch));
    }
    // Final binding check precedes returning a candidate to its output owner.
    checked(engine, &context, binding, Ok(output))
}

fn full(
    engine: &mut dyn TextLayoutEngine,
    request: &StyledTextRequest,
    session: &TextStageContext,
) -> Result<StyledTextLayout, super::TextError> {
    let result = engine.layout_styled_with_diagnostics(request, &session.diagnostics);
    if let Some(binding) = session.binding() {
        // Preserve a full fallback error through the same structured extension chain.
        checked(
            engine,
            session,
            binding,
            result.map_err(TextStageError::extension),
        )
    } else {
        if let Err(error) = session.budget.check() {
            return Err(super::TextError::Stage(Box::new(TextStageFailure {
                primary: TextStageFailureKind::Budget,
                budget_failure: Some(error),
                binding_failure: None,
                stage_error: result.err().map(TextStageError::extension),
                validation_error: None,
            })));
        }
        result
    }
}
