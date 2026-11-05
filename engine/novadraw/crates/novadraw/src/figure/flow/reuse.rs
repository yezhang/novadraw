use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

use crate::{
    FigureId,
    figure::{
        PreparationInputKey, PreparationInputPayload, PreparationInputRegistry,
        PreparationInputSnapshot, PreparationScene,
    },
    text::TextError,
};

use super::{
    ContributionKey, FLOW, FlowContributionContext, FlowContributionHandler,
    FlowContributionPayload, FlowContributionRegistry, FlowError,
};
use crate::figure::preparation::PreparationSource;

/// Owned identity of an extension instance. Clones keep identity; replacements do not.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowReuseIdentity(uuid::Uuid);
impl FlowReuseIdentity {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
    pub fn declaration(self, generation: u64) -> FlowReuseDeclaration {
        FlowReuseDeclaration {
            identity: self,
            generation,
            contributions: Vec::new(),
            inputs: Vec::new(),
        }
    }
}
impl Default for FlowReuseIdentity {
    fn default() -> Self {
        Self::new()
    }
}

/// Promise that omitting this callback preserves its full output and failure behavior.
///
/// The callback must have no required side effects. Every mutable private input must
/// be covered by generation or a registered input snapshot. Generation must never
/// wrap: replace the identity on exhaustion. This first contract is conservative:
/// source inputs are compared in full, and constraints must not affect assembly.
/// Derived reads must be represented by registered inputs; merely reading the same
/// live object twice does not establish an immutable input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowReuseDeclaration {
    identity: FlowReuseIdentity,
    generation: u64,
    contributions: Vec<TypeId>,
    inputs: Vec<PreparationInputSnapshot>,
}
impl FlowReuseDeclaration {
    /// Complete ordered payload-type sequence emitted by a contributor.
    pub fn with_contribution<P: FlowContributionPayload>(mut self, _: ContributionKey<P>) -> Self {
        self.contributions.reserve_exact(1);
        self.contributions.push(TypeId::of::<P>());
        self
    }
    /// An owned first value captured with `PreparationScene::input`.
    pub fn with_input(mut self, input: PreparationInputSnapshot) -> Self {
        self.inputs.reserve_exact(1);
        self.inputs.push(input);
        self
    }
    pub(crate) fn matches_contributions(&self, sequence: impl Iterator<Item = TypeId>) -> bool {
        self.contributions.iter().copied().eq(sequence)
    }
}

/// Adds the same optional declaration to a closure handler as to a struct handler.
pub struct DeclaredFlowHandler<H, D> {
    handler: H,
    declaration: D,
}
impl<H, D> DeclaredFlowHandler<H, D> {
    pub fn new(handler: H, declaration: D) -> Self {
        Self {
            handler,
            declaration,
        }
    }
}
impl<P, H, D> FlowContributionHandler<P> for DeclaredFlowHandler<H, D>
where
    P: FlowContributionPayload,
    H: FlowContributionHandler<P>,
    D: Fn(&FlowContributionContext<'_>) -> Result<Option<FlowReuseDeclaration>, FlowError>
        + Send
        + Sync,
{
    fn process(
        &self,
        payload: &P,
        context: &FlowContributionContext<'_>,
        output: &mut super::FlowAssembly<'_>,
    ) -> Result<(), FlowError> {
        self.handler.process(payload, context, output)
    }
    fn assembly_reuse(
        &self,
        context: &FlowContributionContext<'_>,
    ) -> Result<Option<FlowReuseDeclaration>, FlowError> {
        (self.declaration)(context)
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct AssemblyInput {
    source: Arc<AssemblySource>,
    participants: Arc<Vec<Participant>>,
}
struct AssemblySource {
    value: PreparationSource,
    retention: crate::text::TextContentLease,
    participants: Mutex<Weak<Vec<Participant>>>,
}
impl PartialEq for AssemblySource {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.value.0, &other.value.0) || self.value == other.value
    }
}
type SourceImports = Mutex<HashMap<usize, Weak<AssemblySource>>>;
type Participant = (
    FigureId,
    FlowReuseDeclaration,
    Vec<(FlowReuseIdentity, FlowReuseDeclaration)>,
);

fn declaration_units(declaration: &FlowReuseDeclaration) -> usize {
    declaration.contributions.capacity() * std::mem::size_of::<TypeId>()
        + declaration.inputs.capacity() * std::mem::size_of::<PreparationInputSnapshot>()
        + declaration
            .inputs
            .iter()
            .map(|input| input.retained_units() as usize)
            .sum::<usize>()
}

// Source geometry is validated finite by the tree; source values are immutable.
impl Eq for AssemblyInput {}
impl PreparationInputPayload for AssemblyInput {
    fn retained_units(&self) -> u64 {
        // This is the temporary Preparation domain, not the cross-call ledger.
        let nested = self
            .participants
            .iter()
            .map(|(_, d, handlers)| {
                declaration_units(d)
                    + handlers.capacity()
                        * std::mem::size_of::<(FlowReuseIdentity, FlowReuseDeclaration)>()
                    + handlers
                        .iter()
                        .map(|(_, d)| declaration_units(d))
                        .sum::<usize>()
            })
            .sum::<usize>();
        (self.participants.capacity() * std::mem::size_of::<Participant>()
            + nested
            + 4 * std::mem::size_of::<usize>()) as u64
    }
}
impl AssemblyInput {
    pub(crate) fn source_units(&self) -> usize {
        self.source.retention.units()
    }
}
pub(crate) fn input_units(input: &PreparationInputSnapshot) -> usize {
    (input.retained_units() as usize).saturating_add(
        input
            .value(ASSEMBLY_INPUT)
            .and_then(Option::as_ref)
            .map_or(0, AssemblyInput::source_units),
    )
}

pub(crate) fn declared_handler<P: FlowContributionPayload>(
    handler: impl FlowContributionHandler<P>,
) -> impl FlowContributionHandler<P> {
    let identity = FlowReuseIdentity::new();
    DeclaredFlowHandler::new(handler, move |_: &FlowContributionContext<'_>| {
        Ok(Some(identity.declaration(0)))
    })
}
pub(crate) const ASSEMBLY_INPUT: PreparationInputKey<Option<AssemblyInput>> =
    PreparationInputKey::new("flow.assembly-input");

impl PreparationInputPayload for Option<AssemblyInput> {
    fn retained_units(&self) -> u64 {
        self.as_ref()
            .map_or(0, PreparationInputPayload::retained_units)
    }
}

pub(crate) fn input_registry(
    registry: FlowContributionRegistry,
    store: crate::text::TextContentStore,
) -> PreparationInputRegistry {
    let mut inputs = PreparationInputRegistry::new();
    let imports = Arc::new(SourceImports::default());
    let source_store = store.clone();
    inputs
        .register_with_store(
            ASSEMBLY_INPUT,
            move |_: FigureId, scene: &PreparationScene<'_>| {
                read_input(&registry, scene, &source_store, &imports).map_err(FlowError::into_text)
            },
            Some(store),
        )
        .expect("one assembly input reader");
    inputs
}

fn read_input(
    registry: &FlowContributionRegistry,
    scene: &PreparationScene<'_>,
    store: &crate::text::TextContentStore,
    imports: &SourceImports,
) -> Result<Option<AssemblyInput>, FlowError> {
    let budget = scene.budget();
    let _timing = budget.time("flow-qualification");
    // Root qualification precedes source-prefix capture.
    let root = contributor_declaration(scene.root(), scene)?;
    let Some(root) = root else { return Ok(None) };
    let mut root = Some(root);
    let source = scene.source_prefix()?;
    let key = source.0.as_ptr() as usize;
    let existing = imports
        .lock()
        .map_err(|_| FlowError::InvalidStructure)?
        .get(&key)
        .and_then(Weak::upgrade);
    let expected = existing
        .as_ref()
        .map(|source| {
            source
                .participants
                .lock()
                .map_err(|_| FlowError::InvalidStructure)
                .map(|participants| participants.upgrade())
        })
        .transpose()?
        .flatten();
    // Call every declaration. Only its materialized immutable metadata may
    // be shared; no callback result or private generation is memoized.
    let mut participants: Option<Vec<Participant>> = None;
    for (index, node) in source.0.iter().enumerate() {
        budget
            .consume("flow qualification visit", 1)
            .map_err(TextError::from)?;
        let declaration = if node.id == scene.root() {
            root.take()
        } else {
            contributor_declaration(node.id, scene)?
        };
        let Some(declaration) = declaration else {
            return Ok(None);
        };
        budget
            .allocate(
                "flow qualification temporary declaration",
                declaration_units(&declaration) as u64,
            )
            .map_err(TextError::from)?;
        let previous = expected
            .as_ref()
            .and_then(|participants| participants.get(index));
        let same_declaration =
            previous.is_some_and(|(id, old, _)| *id == node.id && *old == declaration);
        let context = FlowContributionContext {
            source: node.id,
            scene,
        };
        let mut handlers = None;
        for (handler_index, &payload) in declaration.contributions.iter().enumerate() {
            let Some((token, handler)) = registry.handler_declaration(payload, &context)? else {
                return Ok(None);
            };
            budget
                .allocate(
                    "flow qualification temporary handler",
                    declaration_units(&handler) as u64,
                )
                .map_err(TextError::from)?;
            let old_handlers = previous
                .filter(|_| same_declaration)
                .map(|(_, _, handlers)| handlers);
            if handlers.is_none()
                && old_handlers
                    .and_then(|old| old.get(handler_index))
                    .is_some_and(|old| old.0 == token && old.1 == handler)
            {
                continue;
            }
            if handlers.is_none() {
                budget
                    .allocate(
                        "flow qualification handlers",
                        (declaration.contributions.len()
                            * std::mem::size_of::<(FlowReuseIdentity, FlowReuseDeclaration)>())
                            as u64,
                    )
                    .map_err(TextError::from)?;
                let mut next = Vec::with_capacity(declaration.contributions.len());
                if let Some(old) = old_handlers {
                    for (token, declaration) in &old[..handler_index] {
                        budget
                            .allocate(
                                "flow qualification copied handler",
                                declaration_units(declaration) as u64,
                            )
                            .map_err(TextError::from)?;
                        next.push((*token, declaration.clone()));
                    }
                }
                handlers = Some(next);
            }
            handlers
                .as_mut()
                .expect("initialized handlers")
                .push((token, handler));
        }
        if same_declaration && handlers.is_none() {
            if let Some(participants) = &mut participants {
                let previous = previous.expect("matching participant");
                budget
                    .allocate(
                        "flow qualification copied participant",
                        (declaration_units(&previous.1)
                            + previous.2.capacity()
                                * std::mem::size_of::<(FlowReuseIdentity, FlowReuseDeclaration)>()
                            + previous
                                .2
                                .iter()
                                .map(|(_, d)| declaration_units(d))
                                .sum::<usize>()) as u64,
                    )
                    .map_err(TextError::from)?;
                participants.push(previous.clone());
            }
            continue;
        }
        if participants.is_none() {
            budget
                .allocate(
                    "flow qualification participants",
                    (source.0.len() * std::mem::size_of::<Participant>()) as u64,
                )
                .map_err(TextError::from)?;
            let mut next = Vec::with_capacity(source.0.len());
            if let Some(old) = &expected {
                for previous in &old[..index] {
                    budget.allocate("flow qualification copied participant",
                        (declaration_units(&previous.1)
                            + previous.2.capacity() * std::mem::size_of::<(FlowReuseIdentity, FlowReuseDeclaration)>()
                            + previous.2.iter().map(|(_, d)| declaration_units(d)).sum::<usize>()) as u64)
                        .map_err(TextError::from)?;
                    next.push(previous.clone());
                }
            }
            participants = Some(next);
        }
        let handlers = handlers.unwrap_or_default();
        budget
            .allocate(
                "flow qualification records",
                (std::mem::size_of_val(&declaration)
                    + handlers.len()
                        * std::mem::size_of::<(FlowReuseIdentity, FlowReuseDeclaration)>())
                    as u64,
            )
            .map_err(TextError::from)?;
        participants
            .as_mut()
            .expect("initialized participants")
            .push((node.id, declaration, handlers));
    }
    let participants = if let Some(participants) = participants {
        budget
            .allocate(
                "flow qualification shared participants",
                (2 * std::mem::size_of::<usize>()) as u64,
            )
            .map_err(TextError::from)?;
        Arc::new(participants)
    } else {
        expected.expect("all participants matched")
    };
    // The frozen prefix is already immutable. Import its backing into this
    // family once; repeated reads retain the same independent accounting lease.
    // Weak entries neither keep source evidence alive nor trust pointer reuse.
    let mut imports = imports.lock().map_err(|_| FlowError::InvalidStructure)?;
    if let Some(existing) = existing {
        *existing
            .participants
            .lock()
            .map_err(|_| FlowError::InvalidStructure)? = Arc::downgrade(&participants);
        return Ok(Some(AssemblyInput {
            source: existing,
            participants,
        }));
    }
    imports.retain(|_, entry| entry.strong_count() > 0);
    let source_units =
        source
            .0
            .iter()
            .map(|n| {
                std::mem::size_of_val(n)
                    + n.children.capacity() * std::mem::size_of::<FigureId>()
                    + n.resources.capacity()
                        * std::mem::size_of::<(
                            crate::render::ResourceId,
                            Option<crate::ResourceStatus>,
                        )>()
                    + n.style.font.family_capacity()
                    + n.style.tooltip.as_ref().map_or(0, String::capacity)
            })
            .sum::<usize>()
            + std::mem::size_of::<AssemblySource>()
            + 2 * std::mem::size_of::<usize>();
    let retention = store.reserve(source_units).map_err(TextError::from)?;
    budget
        .allocate("flow qualification source import", 1)
        .map_err(TextError::from)?;
    let source = Arc::new(AssemblySource {
        value: source,
        retention,
        participants: Mutex::new(Arc::downgrade(&participants)),
    });
    imports.insert(key, Arc::downgrade(&source));
    Ok(Some(AssemblyInput {
        source,
        participants,
    }))
}

fn contributor_declaration(
    source: FigureId,
    scene: &PreparationScene<'_>,
) -> Result<Option<FlowReuseDeclaration>, FlowError> {
    let Some((capability, figure)) = scene.capability_figure(source, FLOW)? else {
        return Ok(None);
    };
    let Some(contributor) = capability.resolve(figure) else {
        return Ok(None);
    };
    contributor.assembly_reuse(&FlowContributionContext { source, scene })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Figure, MeasureConstraints, Rectangle,
        figure::{
            CONTAINER, ContainerCapability, FigureCapabilityBuilder,
            FigureCapabilityRegistrationError, FigureContainer, ResolvedStyle,
            flow::{
                FlowCapability, FlowContributionSink, FlowContributor, FlowLayoutService,
                FlowPageFigure, StructuredTextFigure, TEXT_CONTRIBUTION, TextContribution,
            },
        },
        text::{BuiltinFont, FontDescriptor, TextSystem},
        tree::FigureTree,
    };
    use std::{
        collections::HashMap,
        sync::{
            Arc,
            atomic::{AtomicU64, AtomicUsize, Ordering},
        },
    };

    #[test]
    fn declaration_keeps_repeated_payloads_in_order_after_first_identity() {
        use crate::figure::flow::{BLOCK_CONTRIBUTION, GROUP_CONTRIBUTION};
        let first = FlowReuseIdentity::new()
            .declaration(7)
            .with_contribution(TEXT_CONTRIBUTION);
        let expanded = first
            .clone()
            .with_contribution(BLOCK_CONTRIBUTION)
            .with_contribution(TEXT_CONTRIBUTION)
            .with_contribution(GROUP_CONTRIBUTION);
        let text = TypeId::of::<TextContribution>();
        let block = TypeId::of::<super::super::BlockContribution>();
        let group = TypeId::of::<super::super::GroupContribution>();
        assert!(expanded.matches_contributions([text, block, text, group].into_iter()));
        assert!(!expanded.matches_contributions([text, text, block, group].into_iter()));
        assert!(!expanded.matches_contributions([text, block, text].into_iter()));
        assert!(first.matches_contributions([text].into_iter()));
        assert_eq!(expanded.contributions.len(), 4);
        assert_eq!(first.contributions.len(), 1);
        assert_eq!(first.generation, expanded.generation);
        assert_eq!(first.identity, expanded.identity);
    }

    #[test]
    fn payload_arrays_are_charged_and_old_declarations_stay_immutable() {
        let empty = FlowReuseIdentity::new().declaration(0);
        assert!(empty.matches_contributions(std::iter::empty()));
        let first = empty.with_contribution(TEXT_CONTRIBUTION);
        let first_units = declaration_units(&first);
        assert!(first_units >= std::mem::size_of::<TypeId>());
        let expanded = first.clone().with_contribution(TEXT_CONTRIBUTION);
        assert!(declaration_units(&expanded) >= 2 * std::mem::size_of::<TypeId>());
        assert_eq!(declaration_units(&first), first_units);
        assert_ne!(first, expanded);
    }

    struct DynamicText {
        identity: FlowReuseIdentity,
        version: Arc<AtomicU64>,
        calls: Arc<AtomicUsize>,
        change_during_contribute: bool,
    }
    impl Figure for DynamicText {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::ZERO
        }
        fn name(&self) -> &'static str {
            "DynamicText"
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(FLOW, FlowCapability::of::<Self>())?;
            out.register(CONTAINER, ContainerCapability::of::<Self>())
        }
    }
    impl FigureContainer for DynamicText {}
    impl FlowContributor for DynamicText {
        fn assembly_reuse(
            &self,
            _: &FlowContributionContext<'_>,
        ) -> Result<Option<FlowReuseDeclaration>, FlowError> {
            Ok(Some(
                self.identity
                    .declaration(self.version.load(Ordering::Relaxed))
                    .with_contribution(TEXT_CONTRIBUTION),
            ))
        }
        fn contribute(
            &self,
            _: &FlowContributionContext<'_>,
            out: &mut FlowContributionSink,
        ) -> Result<(), FlowError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            out.append(
                TEXT_CONTRIBUTION,
                TextContribution(format!(
                    "version {} alpha 中 beta wrap wrap",
                    self.version.load(Ordering::Relaxed)
                )),
            );
            if self.change_during_contribute {
                self.version.fetch_add(1, Ordering::Relaxed);
            }
            Ok(())
        }
    }

    fn fixture(
        change: bool,
    ) -> (
        FigureTree,
        FigureId,
        HashMap<FigureId, ResolvedStyle>,
        Arc<AtomicU64>,
        Arc<AtomicUsize>,
    ) {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(FlowPageFigure::new(Rectangle::ZERO).unwrap()))
            .unwrap();
        let version = Arc::new(AtomicU64::new(0));
        let calls = Arc::new(AtomicUsize::new(0));
        let child = tree
            .builder()
            .add_child(
                root,
                Box::new(DynamicText {
                    identity: FlowReuseIdentity::new(),
                    version: version.clone(),
                    calls: calls.clone(),
                    change_during_contribute: change,
                }),
            )
            .unwrap();
        let styles = [root, child]
            .into_iter()
            .map(|id| (id, tree.resolved_style(id).unwrap()))
            .collect();
        (tree, root, styles, version, calls)
    }
    fn measure(
        service: &FlowLayoutService,
        tree: &FigureTree,
        root: FigureId,
        styles: &HashMap<FigureId, ResolvedStyle>,
        width: f64,
    ) -> Result<crate::figure::PreparationMeasurement, crate::figure::PreparationMeasureError> {
        let mut system = TextSystem::new();
        system.register_builtin_font(BuiltinFont::Inter).unwrap();
        service.measure(
            &mut system.measure(FontDescriptor::default()),
            MeasureConstraints::new(Some(width), None).unwrap(),
            &PreparationScene::new(tree, root, styles).unwrap(),
        )
    }

    #[test]
    fn repeated_proof_reads_share_one_charged_source_backing() {
        let (tree, root, styles, _, _) = fixture(false);
        let store = crate::text::TextContentStore::new(usize::MAX);
        let registry = input_registry(FlowContributionRegistry::standard(), store.clone());
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let first = scene.input(&registry, ASSEMBLY_INPUT, root).unwrap();
        let next = scene.input(&registry, ASSEMBLY_INPUT, root).unwrap();
        let a = first.value(ASSEMBLY_INPUT).unwrap().as_ref().unwrap();
        let b = next.value(ASSEMBLY_INPUT).unwrap().as_ref().unwrap();
        assert!(Arc::ptr_eq(&a.source, &b.source));
        assert!(Arc::ptr_eq(&a.participants, &b.participants));
        assert_eq!(a.source_units(), b.source_units());
        assert!(a == b);
        assert!(store.usage().unwrap().retained_units > a.source_units());
        drop(first);
        drop(next);
        drop(scene);
        drop(registry);
        assert_eq!(store.usage().unwrap().retained_units, 0);
    }

    #[test]
    fn private_generation_changes_replace_metadata_without_overwriting_first_proof() {
        let (tree, root, styles, version, _) = fixture(false);
        let store = crate::text::TextContentStore::new(usize::MAX);
        let registry = input_registry(FlowContributionRegistry::standard(), store);
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let first = scene.input(&registry, ASSEMBLY_INPUT, root).unwrap();
        version.store(1, Ordering::Relaxed);
        let independent = scene.input_scope(root, root, None).unwrap();
        let current = independent.input(&registry, ASSEMBLY_INPUT, root).unwrap();
        let a = first.value(ASSEMBLY_INPUT).unwrap().as_ref().unwrap();
        let b = current.value(ASSEMBLY_INPUT).unwrap().as_ref().unwrap();
        assert!(Arc::ptr_eq(&a.source, &b.source));
        assert!(!Arc::ptr_eq(&a.participants, &b.participants));
        assert!(a != b);
        assert!(scene.validate_input_proofs("metadata return").is_err());
        assert_eq!(a.participants[1].1.generation, 0);
        assert_eq!(b.participants[1].1.generation, 1);
    }

    #[test]
    fn declared_page_skips_callbacks_and_reuses_assembly_across_measurement_widths() {
        let (tree, root, styles, _, calls) = fixture(false);
        let service = FlowLayoutService::default().with_assembly_reuse(true);
        let wide = measure(&service, &tree, root, &styles, 400.0).unwrap();
        let narrow = measure(&service, &tree, root, &styles, 80.0).unwrap();
        assert!(narrow.preferred.height > wide.preferred.height);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        let stats = service.work_statistics().unwrap();
        assert_eq!(stats.assembly_builds, 1);
        assert_eq!(stats.assembly_hits, 1);
        // New services do not inherit cache or grant reuse across bindings.
        let other = FlowLayoutService::default().with_assembly_reuse(true);
        measure(&other, &tree, root, &styles, 80.0).unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn same_engine_measurement_cache_rechecks_private_generation_and_first_evidence() {
        let (tree, root, styles, version, calls) = fixture(false);
        let service = FlowLayoutService::default().with_assembly_reuse(true);
        let mut system = TextSystem::new();
        system.register_builtin_font(BuiltinFont::Inter).unwrap();
        let mut context = system.measure(FontDescriptor::default());
        let constraints = MeasureConstraints::new(Some(80.0), None).unwrap();
        let first = PreparationScene::new(&tree, root, &styles).unwrap();
        service.measure(&mut context, constraints, &first).unwrap();
        let original_builds = service.work_statistics().unwrap().builds;
        let unchanged = PreparationScene::new(&tree, root, &styles).unwrap();
        service
            .measure(&mut context, constraints, &unchanged)
            .unwrap();
        assert_eq!(service.work_statistics().unwrap().builds, original_builds);
        version.store(12345, Ordering::Relaxed);
        // An earlier consumer may not replace its first value with the new generation.
        assert!(service.measure(&mut context, constraints, &first).is_err());
        let updated = PreparationScene::new(&tree, root, &styles).unwrap();
        service
            .measure(&mut context, constraints, &updated)
            .unwrap();
        assert_eq!(
            service.work_statistics().unwrap().builds,
            original_builds + 1
        );
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn private_generation_changes_between_calls_are_misses() {
        let (tree, root, styles, version, calls) = fixture(false);
        let service = FlowLayoutService::default().with_assembly_reuse(true);
        measure(&service, &tree, root, &styles, 400.0).unwrap();
        version.store(7, Ordering::Relaxed);
        measure(&service, &tree, root, &styles, 400.0).unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        assert_eq!(service.work_statistics().unwrap().assembly_builds, 2);
        assert_eq!(service.work_statistics().unwrap().assembly_hits, 0);
    }

    #[test]
    fn private_generation_changes_during_build_are_errors_and_do_not_install() {
        let (tree, root, styles, _, _) = fixture(true);
        let service = FlowLayoutService::default().with_assembly_reuse(true);
        let error = measure(&service, &tree, root, &styles, 400.0).unwrap_err();
        assert!(
            matches!(error.kind, crate::figure::PreparationMeasureErrorKind::Text(error)
            if matches!(*error, TextError::PreparationInput(_)))
        );
        assert_eq!(service.assembly_usage().unwrap().retained_units, 0);
        assert_eq!(service.work_statistics().unwrap().assembly_hits, 0);
    }

    #[test]
    fn missing_handler_declaration_keeps_the_existing_path() {
        let mut registry = FlowContributionRegistry::new();
        registry
            .register(
                super::super::BLOCK_CONTRIBUTION,
                |p: &super::super::BlockContribution,
                 c: &FlowContributionContext<'_>,
                 out: &mut super::super::FlowAssembly<'_>| {
                    out.block(c.source, p.0)
                },
            )
            .unwrap();
        registry
            .register(
                TEXT_CONTRIBUTION,
                |p: &TextContribution,
                 c: &FlowContributionContext<'_>,
                 out: &mut super::super::FlowAssembly<'_>| {
                    out.text(c.source, &p.0, c.text_style()?)
                },
            )
            .unwrap();
        let (tree, root, styles, _, calls) = fixture(false);
        let service = FlowLayoutService::new(registry).with_assembly_reuse(true);
        for _ in 0..2 {
            measure(&service, &tree, root, &styles, 400.0).unwrap();
        }
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        assert_eq!(service.work_statistics().unwrap().assembly_ineligible, 2);
    }

    #[test]
    fn shared_family_limits_preserve_old_content_and_refuse_new_assets() {
        let (tree, root, styles, version, _) = fixture(false);
        let service = FlowLayoutService::default().with_assembly_reuse(true);
        measure(&service, &tree, root, &styles, 400.0).unwrap();
        let peer = service.clone();
        let old_usage = service.assembly_usage().unwrap();
        assert!(old_usage.retained_units > 0);
        // Paragraph layout keeps its content lease after the cache reference is evicted.
        peer.set_assembly_limits(0, 0).unwrap();
        assert!(service.assembly_usage().unwrap().is_over_limit());
        version.store(1, Ordering::Relaxed);
        assert!(measure(&service, &tree, root, &styles, 400.0).is_err());
        assert_eq!(service.work_statistics().unwrap().assembly_hits, 0);
        peer.set_assembly_limits(usize::MAX, usize::MAX).unwrap();
        measure(&service, &tree, root, &styles, 400.0).unwrap();
    }

    #[test]
    fn standard_text_has_the_same_optional_contract_as_extensions() {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(FlowPageFigure::new(Rectangle::ZERO).unwrap()))
            .unwrap();
        let child = tree
            .builder()
            .add_child(root, Box::new(StructuredTextFigure::new("alpha 中 beta")))
            .unwrap();
        let styles = [root, child]
            .into_iter()
            .map(|id| (id, tree.resolved_style(id).unwrap()))
            .collect();
        let service = FlowLayoutService::default().with_assembly_reuse(true);
        for _ in 0..2 {
            measure(&service, &tree, root, &styles, 400.0).unwrap();
        }
        assert_eq!(service.work_statistics().unwrap().assembly_builds, 1);
        assert_eq!(service.work_statistics().unwrap().assembly_hits, 1);
    }
}
