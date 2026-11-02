use super::*;

/// Producer-owned immutable analysis. Heap and shared storage must remain within its report.
/// Interior caches may use already reported capacity, but may not grow or change semantics.
pub trait TextAnalysisPayload: Any {
    fn storage(&self, context: &TextStageContext) -> Result<TextAnalysisStorage, TextStageError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextAnalysisCacheLimits {
    pub entries: usize,
    pub retained_bytes: usize,
}

/// A fresh responsibility domain and generation for each independent shared allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextAnalysisBackingIdentity {
    domain: uuid::Uuid,
    generation: uuid::Uuid,
}
impl Default for TextAnalysisBackingIdentity {
    fn default() -> Self {
        Self {
            domain: uuid::Uuid::new_v4(),
            generation: uuid::Uuid::new_v4(),
        }
    }
}
impl TextAnalysisBackingIdentity {
    pub fn next_generation(self) -> Self {
        Self {
            domain: self.domain,
            generation: uuid::Uuid::new_v4(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextAnalysisBacking {
    pub identity: TextAnalysisBackingIdentity,
    pub retained_bytes: usize,
}

/// Own heap capacity excludes the inline payload (Core accounts for it).
/// Shared backing reports include all storage pinned by that backing.
#[derive(Debug)]
pub struct TextAnalysisStorage {
    pub owned_bytes: usize,
    pub shared: Vec<TextAnalysisBacking>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextAnalysisUsage {
    pub entries: usize,
    pub retained_bytes: usize,
    /// Changes when admission, actual release or bookkeeping capacity changes.
    pub generation: u64,
}

/// Bound to the registry observation and complete analysis input which produced it.
#[derive(Clone, Debug)]
pub struct TextAnalysisAdmissionCost {
    binding: TextStageBinding,
    request: Rc<TextAnalysisRequest>,
    generation: u64,
    bytes: usize,
    entries: usize,
}

#[derive(Clone, Copy)]
struct BackingRecord {
    backing: TextAnalysisBacking,
    references: usize,
}

struct Ledger {
    limits: TextAnalysisCacheLimits,
    usage: TextAnalysisUsage,
    backings: Vec<BackingRecord>,
}
impl Ledger {
    fn changed(&mut self) {
        // Once exhausted, observations are never accepted as current.
        self.usage.generation = self.usage.generation.saturating_add(1);
    }
}

struct Entry {
    binding: TextStageBinding,
    request: Rc<TextAnalysisRequest>,
    payload: Rc<dyn Any>,
    storage: TextAnalysisStorage,
    exclusive_bytes: usize,
    ledger: Rc<RefCell<Ledger>>,
}
impl Drop for Entry {
    fn drop(&mut self) {
        let mut ledger = self.ledger.borrow_mut();
        ledger.usage.entries -= 1;
        ledger.usage.retained_bytes -= self.exclusive_bytes;
        for backing in &self.storage.shared {
            if let Some(index) = ledger
                .backings
                .iter()
                .position(|r| r.backing.identity == backing.identity)
            {
                let record = &mut ledger.backings[index];
                record.references -= 1;
                if record.references == 0 {
                    let record = ledger.backings.swap_remove(index);
                    ledger.usage.retained_bytes -= record.backing.retained_bytes;
                }
            }
        }
        // The table capacity remains allocated and charged after logical deletion.
        ledger.changed();
    }
}

/// Holds both immutable payload and its accounting owner after cache eviction.
#[derive(Clone)]
pub struct TextAnalysisLease(Rc<Entry>);
impl fmt::Debug for TextAnalysisLease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextAnalysisLease")
            .field("binding", &self.0.binding)
            .finish_non_exhaustive()
    }
}
impl TextAnalysisLease {
    pub fn binding(&self) -> TextStageBinding {
        self.0.binding
    }
    pub fn request(&self) -> &TextAnalysisRequest {
        &self.0.request
    }
    pub fn usage(&self) -> TextAnalysisUsage {
        self.0.ledger.borrow().usage
    }
}

/// Instance-owned cache. The index is preallocated and its capacity stays charged.
pub struct TextAnalysisRegistry {
    identity: TextAnalysisRegistryIdentity,
    engine: TextLayoutCacheIdentity,
    enabled: bool,
    cache: RefCell<Vec<Option<TextAnalysisLease>>>,
    oldest: RefCell<usize>,
    ledger: Rc<RefCell<Ledger>>,
    index_bytes: usize,
    #[cfg(test)]
    fail_reservation: std::cell::Cell<bool>,
}

fn add(a: usize, b: usize) -> Result<usize, TextStageError> {
    a.checked_add(b).ok_or(TextStageError::AccountingOverflow)
}
fn capacity_bytes<T>(capacity: usize) -> Result<usize, TextStageError> {
    capacity
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(TextStageError::AccountingOverflow)
}

impl TextAnalysisRegistry {
    pub fn new(
        engine: TextLayoutCacheIdentity,
        limits: TextAnalysisCacheLimits,
    ) -> Result<Self, TextStageError> {
        let enabled = limits.entries != 0 && limits.retained_bytes != 0;
        let mut cache = Vec::new();
        let mut used = 0;
        let mut index_bytes = 0;
        if enabled {
            let minimum = add(
                capacity_bytes::<Option<TextAnalysisLease>>(limits.entries)?,
                add(std::mem::size_of::<Ledger>(), RC_COUNTER_BYTES)?,
            )?;
            if minimum > limits.retained_bytes {
                return Err(TextStageError::CacheMetadataLimit);
            }
            cache
                .try_reserve_exact(limits.entries)
                .map_err(|_| TextStageError::StorageReservation)?;
            index_bytes = capacity_bytes::<Option<TextAnalysisLease>>(cache.capacity())?;
            used = add(
                index_bytes,
                add(std::mem::size_of::<Ledger>(), RC_COUNTER_BYTES)?,
            )?;
            if used > limits.retained_bytes {
                return Err(TextStageError::CacheMetadataLimit);
            }
            cache.resize_with(limits.entries, || None);
        }
        Ok(Self {
            identity: TextAnalysisRegistryIdentity(uuid::Uuid::new_v4()),
            engine,
            enabled,
            cache: RefCell::new(cache),
            oldest: RefCell::new(0),
            ledger: Rc::new(RefCell::new(Ledger {
                limits,
                usage: TextAnalysisUsage {
                    retained_bytes: used,
                    ..Default::default()
                },
                backings: Vec::new(),
            })),
            index_bytes,
            #[cfg(test)]
            fail_reservation: std::cell::Cell::new(false),
        })
    }
    pub fn identity(&self) -> TextAnalysisRegistryIdentity {
        self.identity
    }
    pub fn engine(&self) -> TextLayoutCacheIdentity {
        self.engine
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    pub fn usage(&self) -> TextAnalysisUsage {
        self.ledger.borrow().usage
    }
    pub fn limits(&self) -> TextAnalysisCacheLimits {
        self.ledger.borrow().limits
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        let limits = self.limits();
        self.enabled = enabled && limits.entries != 0 && limits.retained_bytes != 0;
    }

    fn call<'a>(
        &self,
        context: &'a TextStageContext,
    ) -> Result<&'a (TextStageBinding, Rc<TextAnalysisRequest>), TextStageError> {
        let call = context
            .call
            .as_ref()
            .ok_or(TextStageError::RegistryIdentityMismatch)?;
        if call.0.engine != self.engine {
            return Err(TextStageError::EngineIdentityMismatch);
        }
        if call.0.registry != self.identity {
            return Err(TextStageError::RegistryIdentityMismatch);
        }
        Ok(call)
    }

    pub fn admission_lower_bound(
        &self,
        context: &TextStageContext,
        additional_bytes: usize,
        entries: usize,
    ) -> Result<TextAnalysisAdmissionCost, TextStageError> {
        let (binding, request) = self.call(context)?;
        if entries == 0 {
            return Err(TextStageError::InvalidStorageDescription);
        }
        // Even an empty payload needs request, entry and reference counters.
        let metadata = add(
            request.storage_bytes()?,
            add(std::mem::size_of::<Entry>(), RC_COUNTER_BYTES)?,
        )?;
        let bytes = add(metadata, additional_bytes)?;
        Ok(TextAnalysisAdmissionCost {
            binding: *binding,
            request: request.clone(),
            generation: self.usage().generation,
            bytes,
            entries,
        })
    }

    pub(super) fn definitely_exceeds(
        &self,
        context: &TextStageContext,
        cost: TextAnalysisAdmissionCost,
    ) -> Result<bool, TextStageError> {
        let (binding, request) = self.call(context)?;
        context.charge(
            "text-analysis-lower-validation",
            TextStageCostPlan {
                work: request.comparison_work()?,
                allocation: 0,
            },
        )?;
        let ledger = self.ledger.borrow();
        if cost.binding != *binding
            || cost.request != *request
            || cost.generation != ledger.usage.generation
            || cost.generation == u64::MAX
        {
            return Ok(false);
        }
        Ok(cost.bytes
            > ledger
                .limits
                .retained_bytes
                .saturating_sub(ledger.usage.retained_bytes)
            || cost.entries > ledger.limits.entries.saturating_sub(ledger.usage.entries))
    }

    pub fn find(
        &self,
        context: &TextStageContext,
    ) -> Result<Option<TextAnalysisLease>, TextStageError> {
        let (binding, request) = self.call(context)?;
        let comparisons = request
            .comparison_work()?
            .checked_mul(
                u64::try_from(self.cache.borrow().len())
                    .map_err(|_| TextStageError::AccountingOverflow)?,
            )
            .ok_or(TextStageError::AccountingOverflow)?;
        context.charge(
            "text-analysis-lookup",
            TextStageCostPlan {
                work: comparisons,
                allocation: 0,
            },
        )?;
        Ok(self
            .cache
            .borrow()
            .iter()
            .flatten()
            .find(|lease| lease.0.binding == *binding && lease.0.request == *request)
            .cloned())
    }

    pub fn validate(
        &self,
        lease: &TextAnalysisLease,
        context: &TextStageContext,
    ) -> Result<(), TextStageError> {
        let (binding, request) = self.call(context)?;
        let captured = lease.binding();
        if captured.registry != binding.registry {
            return Err(TextStageError::RegistryIdentityMismatch);
        }
        if captured.engine != binding.engine {
            return Err(TextStageError::EngineIdentityMismatch);
        }
        if captured.key != binding.key {
            return Err(TextStageError::KeyMismatch);
        }
        if captured.revision != binding.revision {
            return Err(TextStageError::RevisionMismatch);
        }
        context.charge(
            "text-analysis-lease-validation",
            TextStageCostPlan {
                work: request.comparison_work()?,
                allocation: 0,
            },
        )?;
        if lease.request() != request.as_ref() {
            return Err(TextStageError::InputMismatch);
        }
        Ok(())
    }

    pub fn read<'a, A: TextAnalysisPayload>(
        &self,
        key: TextAnalysisKey<A>,
        lease: &'a TextAnalysisLease,
        context: &TextStageContext,
    ) -> Result<&'a A, TextStageError> {
        self.validate(lease, context)?;
        if key.erase() != lease.binding().key {
            return Err(TextStageError::KeyMismatch);
        }
        lease
            .0
            .payload
            .downcast_ref::<A>()
            .ok_or(TextStageError::KeyMismatch)
    }

    /// Eviction removes only cache references; pinned leases remain in the ledger.
    pub fn clear(&self) {
        for slot in self.cache.borrow_mut().iter_mut() {
            *slot = None;
        }
    }

    pub fn insert<A: TextAnalysisPayload>(
        &self,
        key: TextAnalysisKey<A>,
        payload: A,
        request: &TextAnalysisRequest,
        context: &TextStageContext,
    ) -> Result<Option<TextAnalysisLease>, TextStageError> {
        let (binding, captured_request) = self.call(context)?;
        if key.erase() != binding.key {
            return Err(TextStageError::KeyMismatch);
        }
        context.charge(
            "text-analysis-input-validation",
            TextStageCostPlan {
                work: captured_request.comparison_work()?,
                allocation: 0,
            },
        )?;
        if request != captured_request.as_ref() {
            return Err(TextStageError::InputMismatch);
        }
        context.budget.check()?;
        if !self.enabled {
            return Ok(None);
        }
        let storage = payload.storage(context)?;
        context.budget.check()?;
        context.charge("text-analysis-retained-request", request.copy_plan()?)?;
        // An entry owns a distinct request; session clones do not pin its metadata.
        // Complete all fallible storage work before committing any ledger mutation.
        let owned_request = Rc::new(TextAnalysisRequest::new(request.request())?);
        let scan = self
            .ledger
            .borrow()
            .backings
            .len()
            .checked_add(storage.shared.len())
            .and_then(|n| n.checked_mul(storage.shared.len()))
            .and_then(|n| n.checked_mul(self.cache.borrow().len().checked_add(1)?))
            .and_then(|n| n.checked_mul(2))
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(TextStageError::AccountingOverflow)?;
        context.charge(
            "text-analysis-admission",
            TextStageCostPlan {
                work: scan,
                allocation: 0,
            },
        )?;
        for (index, backing) in storage.shared.iter().enumerate() {
            if backing.retained_bytes == 0
                || storage.shared[..index]
                    .iter()
                    .any(|b| b.identity == backing.identity)
            {
                return Err(TextStageError::InvalidStorageDescription);
            }
        }
        let exclusive = add(
            add(owned_request.storage_bytes()?, storage.owned_bytes)?,
            add(
                add(std::mem::size_of::<Entry>(), RC_COUNTER_BYTES)?,
                add(
                    rc_allocation_bytes::<A>()?,
                    capacity_bytes::<TextAnalysisBacking>(storage.shared.capacity())?,
                )?,
            )?,
        )?;
        let mut attempts = self.cache.borrow().len();
        loop {
            let mut ledger = self.ledger.borrow_mut();
            let mut new_bytes = 0;
            let mut new_backings = 0usize;
            for backing in &storage.shared {
                if let Some(record) = ledger
                    .backings
                    .iter()
                    .find(|r| r.backing.identity == backing.identity)
                {
                    if record.backing != *backing {
                        return Err(TextStageError::BackingDescriptionConflict);
                    }
                    record
                        .references
                        .checked_add(1)
                        .ok_or(TextStageError::AccountingOverflow)?;
                } else {
                    new_bytes = add(new_bytes, backing.retained_bytes)?;
                    new_backings = add(new_backings, 1)?;
                }
            }
            let required = add(ledger.backings.len(), new_backings)?;
            let growth = required.saturating_sub(ledger.backings.capacity());
            let minimum = add(
                add(ledger.usage.retained_bytes, exclusive)?,
                add(new_bytes, capacity_bytes::<BackingRecord>(growth)?)?,
            )?;
            if ledger.usage.entries < ledger.limits.entries
                && minimum <= ledger.limits.retained_bytes
            {
                // Build replacement bookkeeping off to the side. Nothing is visible until commit.
                let mut replacement = None;
                let mut table_growth = 0;
                if growth != 0 {
                    let allocation =
                        u64::try_from(required).map_err(|_| TextStageError::AccountingOverflow)?;
                    context.charge(
                        "text-analysis-backing-table",
                        TextStageCostPlan {
                            work: u64::try_from(ledger.backings.len())
                                .map_err(|_| TextStageError::AccountingOverflow)?,
                            allocation,
                        },
                    )?;
                    let mut table = Vec::new();
                    table
                        .try_reserve_exact(required)
                        .map_err(|_| TextStageError::StorageReservation)?;
                    table.extend_from_slice(&ledger.backings);
                    table_growth = capacity_bytes::<BackingRecord>(
                        table.capacity().saturating_sub(ledger.backings.capacity()),
                    )?;
                    replacement = Some(table);
                }
                let total = add(
                    add(ledger.usage.retained_bytes, exclusive)?,
                    add(new_bytes, table_growth)?,
                )?;
                if total > ledger.limits.retained_bytes {
                    return Ok(None);
                }
                context.charge(
                    "text-analysis-entry",
                    TextStageCostPlan {
                        work: 0,
                        allocation: 2,
                    },
                )?;
                #[cfg(test)]
                if self.fail_reservation.replace(false) {
                    return Err(TextStageError::StorageReservation);
                }
                if let Some(table) = replacement {
                    ledger.backings = table;
                }
                for backing in &storage.shared {
                    if let Some(record) = ledger
                        .backings
                        .iter_mut()
                        .find(|r| r.backing.identity == backing.identity)
                    {
                        record.references += 1;
                    } else {
                        ledger.backings.push(BackingRecord {
                            backing: *backing,
                            references: 1,
                        });
                    }
                }
                ledger.usage.retained_bytes = total;
                ledger.usage.entries += 1;
                ledger.changed();
                drop(ledger);
                let lease = TextAnalysisLease(Rc::new(Entry {
                    binding: *binding,
                    request: owned_request,
                    payload: Rc::new(payload),
                    storage,
                    exclusive_bytes: exclusive,
                    ledger: self.ledger.clone(),
                }));
                let mut cache = self.cache.borrow_mut();
                let index = cache
                    .iter()
                    .position(Option::is_none)
                    .unwrap_or(*self.oldest.borrow());
                cache[index] = Some(lease.clone());
                *self.oldest.borrow_mut() = (index + 1) % cache.len();
                return Ok(Some(lease));
            }
            drop(ledger);
            if attempts == 0 {
                return Ok(None);
            }
            attempts -= 1;
            let mut cache = self.cache.borrow_mut();
            let index = *self.oldest.borrow();
            cache[index] = None;
            *self.oldest.borrow_mut() = (index + 1) % cache.len();
        }
    }
}

impl Drop for TextAnalysisRegistry {
    fn drop(&mut self) {
        // Index allocation actually releases here, unlike clear().
        self.cache.get_mut().clear();
        let mut ledger = self.ledger.borrow_mut();
        ledger.usage.retained_bytes -= self.index_bytes;
        ledger.changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::PreparationBudget;
    use crate::graphics::Color;
    use crate::text::{FontDescriptor, TextConstraints, TextStyle};

    struct Asset(TextAnalysisBacking);
    impl TextAnalysisPayload for Asset {
        fn storage(
            &self,
            context: &TextStageContext,
        ) -> Result<TextAnalysisStorage, TextStageError> {
            context.charge(
                "storage-description",
                TextStageCostPlan {
                    work: 0,
                    allocation: 1,
                },
            )?;
            let mut shared = Vec::new();
            shared
                .try_reserve_exact(1)
                .map_err(|_| TextStageError::StorageReservation)?;
            shared.push(self.0);
            Ok(TextAnalysisStorage {
                owned_bytes: 0,
                shared,
            })
        }
    }
    const KEY: TextAnalysisKey<Asset> = TextAnalysisKey::new("private-transaction-test");

    fn setup() -> (TextAnalysisRegistry, TextStageContext, TextAnalysisBacking) {
        let identity = TextLayoutCacheIdentity::default();
        let registry = TextAnalysisRegistry::new(
            identity,
            TextAnalysisCacheLimits {
                entries: 2,
                retained_bytes: 16_384,
            },
        )
        .unwrap();
        let request = StyledTextRequest::new(
            "checked",
            TextStyle::new(FontDescriptor::default(), Color::BLACK),
            Vec::new(),
            Vec::new(),
            TextConstraints::UNBOUNDED,
        )
        .unwrap();
        let context = TextStageContext::new(
            PreparationBudgetHandle::new(PreparationBudget::new(10_000, 10_000)),
            TextLayoutDiagnostics::default(),
        )
        .bound(
            TextStageBinding {
                engine: identity,
                revision: 0,
                registry: registry.identity(),
                key: KEY.erase(),
            },
            TextAnalysisRequest::new(&request).unwrap(),
        );
        let backing = TextAnalysisBacking {
            identity: TextAnalysisBackingIdentity::default(),
            retained_bytes: 128,
        };
        (registry, context, backing)
    }

    #[test]
    fn reservation_failure_after_preparation_leaves_no_backing_or_readable_entry() {
        let (registry, context, backing) = setup();
        let before = registry.usage();
        registry.fail_reservation.set(true);
        assert!(matches!(
            registry.insert(
                KEY,
                Asset(backing),
                context.analysis_request().unwrap(),
                &context
            ),
            Err(TextStageError::StorageReservation)
        ));
        assert_eq!(registry.usage(), before);
        assert!(registry.ledger.borrow().backings.is_empty());
        assert_eq!(registry.ledger.borrow().backings.capacity(), 0);
        assert!(registry.find(&context).unwrap().is_none());
        assert!(context.budget.used().allocation > 0);
        let lease = registry
            .insert(
                KEY,
                Asset(backing),
                context.analysis_request().unwrap(),
                &context,
            )
            .unwrap()
            .unwrap();
        assert_eq!(lease.usage().entries, 1);
        assert_eq!(registry.ledger.borrow().backings[0].references, 1);
    }

    #[test]
    fn stale_lower_bound_cannot_reject_a_new_observation() {
        let (registry, context, backing) = setup();
        let cost = registry
            .admission_lower_bound(&context, usize::MAX / 2, 1)
            .unwrap();
        assert!(registry.definitely_exceeds(&context, cost.clone()).unwrap());
        let lease = registry
            .insert(
                KEY,
                Asset(backing),
                context.analysis_request().unwrap(),
                &context,
            )
            .unwrap()
            .unwrap();
        assert!(!registry.definitely_exceeds(&context, cost).unwrap());
        registry.clear();
        drop(lease);
        assert_eq!(registry.usage().entries, 0);
        assert_eq!(registry.ledger.borrow().backings.len(), 0);
        assert!(registry.ledger.borrow().backings.capacity() > 0);
    }

    #[test]
    fn insufficient_capacity_does_not_commit_partial_shared_backing() {
        let (registry, context, mut backing) = setup();
        backing.retained_bytes = registry.limits().retained_bytes;
        let before = registry.usage();
        assert!(
            registry
                .insert(
                    KEY,
                    Asset(backing),
                    context.analysis_request().unwrap(),
                    &context
                )
                .unwrap()
                .is_none()
        );
        assert_eq!(registry.usage(), before);
        assert!(registry.ledger.borrow().backings.is_empty());
    }

    #[test]
    fn payload_alignment_is_included_in_reference_allocation_storage() {
        #[repr(align(64))]
        struct Aligned;
        assert!(rc_allocation_bytes::<Aligned>().unwrap() >= 64);
    }

    #[test]
    fn external_context_clone_does_not_pin_retained_request_after_last_lease() {
        let (registry, context, backing) = setup();
        let cloned_context = context.clone();
        let lease = registry
            .insert(
                KEY,
                Asset(backing),
                context.analysis_request().unwrap(),
                &context,
            )
            .unwrap()
            .unwrap();
        let request = Rc::downgrade(&lease.0.request);
        registry.clear();
        drop(lease);
        assert!(request.upgrade().is_none());
        assert!(cloned_context.analysis_request().is_some());
        assert_eq!(registry.usage().entries, 0);
    }
}
