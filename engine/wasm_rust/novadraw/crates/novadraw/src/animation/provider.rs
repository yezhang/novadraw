use std::{
    any::{Any, TypeId},
    cell::Cell,
    collections::{HashMap, HashSet},
    marker::PhantomData,
    sync::Arc,
};

use crate::FigureId;

use super::{AnimationError, AnimationValue, GeometryPatch, PresentationGeometry};

/// Typed identity of one derived value. Names and the underlying Rust representation do not alias
/// distinct semantic types; a Figure's generation remains part of the identity.
pub struct GeometryProviderKey<V> {
    figure: FigureId,
    marker: PhantomData<fn() -> V>,
}

impl<V> Copy for GeometryProviderKey<V> {}
impl<V> Clone for GeometryProviderKey<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: AnimationValue> GeometryProviderKey<V> {
    /// Declares a semantic output at a Figure.
    pub fn new(figure: FigureId) -> Self {
        Self {
            figure,
            marker: PhantomData,
        }
    }

    /// Erases this key for an explicit dependency declaration.
    pub fn dependency(self) -> GeometryProviderDependency {
        GeometryProviderDependency {
            figure: self.figure,
            value_type: TypeId::of::<V>(),
        }
    }
}

/// Explicit typed provider dependency. Constructed from a typed key.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GeometryProviderDependency {
    figure: FigureId,
    value_type: TypeId,
}

/// Read-only inputs to a derived provider.
///
/// Geometry outputs query the complete cached Motion geometry. Drawing outputs query the
/// geometry rebuilt with all accepted geometry patches. Within either phase, preceding typed
/// values are available only through declared dependencies; queries never trigger evaluation.
pub struct GeometryProviderContext<'a> {
    geometry: &'a PresentationGeometry,
    figures: &'a HashSet<FigureId>,
    dependencies: &'a HashSet<GeometryProviderDependency>,
    values: &'a HashMap<GeometryProviderDependency, Arc<dyn Any + Send + Sync>>,
    key: GeometryProviderDependency,
    operations: Cell<usize>,
    operation_limit: usize,
}

impl GeometryProviderContext<'_> {
    fn spend(&self, count: usize) -> Result<(), AnimationError> {
        let actual = self.operations.get().saturating_add(count);
        self.operations.set(actual);
        if actual > self.operation_limit {
            return Err(AnimationError::ProviderOutputExceeded {
                kind: super::AnimationBudgetKind::GeometryOperations,
                declared: self.operation_limit,
                actual,
                figure: self.key.figure,
                value_type: self.key.value_type,
            });
        }
        Ok(())
    }

    /// Queries only a declared Figure dependency.
    pub fn figure(
        &self,
        figure: FigureId,
    ) -> Result<&super::PresentedFigureGeometry, AnimationError> {
        self.spend(1)?;
        if !self.figures.contains(&figure) {
            return Err(AnimationError::UndeclaredGeometryDependency { figure });
        }
        self.geometry.figure(figure)
    }

    /// Reads a preceding provider's typed value.
    pub fn value<V: AnimationValue>(
        &self,
        key: GeometryProviderKey<V>,
    ) -> Result<&V, AnimationError> {
        self.spend(1)?;
        let dependency = key.dependency();
        if !self.dependencies.contains(&dependency) {
            return Err(AnimationError::UndeclaredProviderDependency);
        }
        self.values
            .get(&dependency)
            .and_then(|value| value.downcast_ref())
            .ok_or(AnimationError::UndeclaredProviderDependency)
    }

    /// Maps between two explicitly declared local domains.
    pub fn relative_transform(
        &self,
        from: FigureId,
        to: FigureId,
    ) -> Result<Option<crate::Affine2D>, AnimationError> {
        self.spend(3)?;
        self.figure(from)?;
        self.figure(to)?;
        self.geometry.relative_transform(from, to)
    }

    /// Maps into a declared container's child-content domain.
    pub fn to_content_transform(
        &self,
        from: FigureId,
        container: FigureId,
    ) -> Result<Option<crate::Affine2D>, AnimationError> {
        self.spend(4)?;
        self.figure(from)?;
        self.figure(container)?;
        self.geometry.to_content_transform(from, container)
    }
}

type Derive<V> = dyn Fn(&GeometryProviderContext<'_>) -> Result<V, AnimationError> + Send + Sync;
type Apply<V> = dyn Fn(&V) -> Result<GeometryPatch, AnimationError> + Send + Sync;
type Draw<V> = dyn Fn(
        &V,
        &GeometryProviderContext<'_>,
        &mut super::GeometryDrawingBuilder,
    ) -> Result<(), AnimationError>
    + Send
    + Sync;

enum Output<V> {
    Geometry(Arc<Apply<V>>),
    Drawing { lines: usize, draw: Arc<Draw<V>> },
}

/// Pure bounded derived-value descriptor. Internal and external consumers use this constructor.
///
/// One evaluation publishes one typed value and at most four geometry fields. The dependency
/// graph and unique output fields are validated before channel ownership is transferred.
/// Callback captures must be fixed `Copy` data. References/handles may identify immutable
/// external data but must not retain hidden mutable inputs or unaccounted owned allocations.
/// Each evaluation permits eight query/coordinate operations per declared dependency plus
/// eight for the output. Repeated queries consume this same finite allowance.
pub struct GeometryProvider<V> {
    key: GeometryProviderKey<V>,
    figures: HashSet<FigureId>,
    dependencies: Vec<GeometryProviderDependency>,
    dependency_lookup: HashSet<GeometryProviderDependency>,
    derive: Arc<Derive<V>>,
    output: Output<V>,
    callback_bytes: usize,
}

impl<V: AnimationValue + Copy> GeometryProvider<V> {
    /// Creates a provider with explicit Figure and provider dependencies.
    pub fn new(
        key: GeometryProviderKey<V>,
        figures: impl IntoIterator<Item = FigureId>,
        dependencies: Vec<GeometryProviderDependency>,
        derive: impl Fn(&GeometryProviderContext<'_>) -> Result<V, AnimationError>
        + Copy
        + Send
        + Sync
        + 'static,
        apply: impl Fn(&V) -> Result<GeometryPatch, AnimationError> + Copy + Send + Sync + 'static,
    ) -> Self {
        let callback_bytes = std::mem::size_of_val(&derive) + std::mem::size_of_val(&apply);
        Self {
            key,
            figures: figures.into_iter().collect(),
            dependency_lookup: dependencies.iter().copied().collect(),
            dependencies,
            derive: Arc::new(derive),
            output: Output::Geometry(Arc::new(apply)),
            callback_bytes,
        }
    }

    /// Derives bounded immutable lines after every geometry provider has published its output.
    ///
    /// The value is inline and copied; `max_lines` reserves both output items and storage.
    /// Drawing providers can depend on geometry values or preceding drawing values, but cannot
    /// write geometry. Geometry providers cannot depend on drawing values.
    pub fn drawing(
        key: GeometryProviderKey<V>,
        figures: impl IntoIterator<Item = FigureId>,
        dependencies: Vec<GeometryProviderDependency>,
        max_lines: usize,
        derive: impl Fn(&GeometryProviderContext<'_>) -> Result<V, AnimationError>
        + Copy
        + Send
        + Sync
        + 'static,
        draw: impl Fn(
            &V,
            &GeometryProviderContext<'_>,
            &mut super::GeometryDrawingBuilder,
        ) -> Result<(), AnimationError>
        + Copy
        + Send
        + Sync
        + 'static,
    ) -> Self {
        let callback_bytes = std::mem::size_of_val(&derive) + std::mem::size_of_val(&draw);
        Self {
            key,
            figures: figures.into_iter().collect(),
            dependency_lookup: dependencies.iter().copied().collect(),
            dependencies,
            derive: Arc::new(derive),
            output: Output::Drawing {
                lines: max_lines,
                draw: Arc::new(draw),
            },
            callback_bytes,
        }
    }

    pub(crate) fn erased(self) -> Arc<dyn ErasedGeometryProvider> {
        Arc::new(self)
    }
}

pub(crate) trait ErasedGeometryProvider: Send + Sync {
    fn key(&self) -> GeometryProviderDependency;
    fn figures(&self) -> &HashSet<FigureId>;
    fn dependencies(&self) -> &[GeometryProviderDependency];
    fn dependency_lookup(&self) -> &HashSet<GeometryProviderDependency>;
    fn is_drawing(&self) -> bool;
    fn operation_limit(&self) -> Result<usize, AnimationError>;
    fn work(&self) -> Result<super::AnimationWork, AnimationError>;
    fn evaluate(
        &self,
        context: &GeometryProviderContext<'_>,
    ) -> Result<DerivedValue, AnimationError>;
}

pub(crate) struct DerivedValue {
    pub(crate) value: Arc<dyn Any + Send + Sync>,
    pub(crate) patch: GeometryPatch,
    pub(crate) content: Option<crate::figure::FigurePresentation>,
}

impl<V: AnimationValue + Copy> ErasedGeometryProvider for GeometryProvider<V> {
    fn key(&self) -> GeometryProviderDependency {
        self.key.dependency()
    }
    fn figures(&self) -> &HashSet<FigureId> {
        &self.figures
    }
    fn dependencies(&self) -> &[GeometryProviderDependency] {
        &self.dependencies
    }
    fn dependency_lookup(&self) -> &HashSet<GeometryProviderDependency> {
        &self.dependency_lookup
    }
    fn is_drawing(&self) -> bool {
        matches!(self.output, Output::Drawing { .. })
    }
    fn operation_limit(&self) -> Result<usize, AnimationError> {
        self.figures
            .len()
            .checked_add(self.dependencies.len())
            .and_then(|n| n.checked_add(1))
            .and_then(|n| n.checked_mul(8))
            .ok_or(AnimationError::BudgetExceeded(
                super::AnimationBudgetKind::GeometryOperations,
            ))
    }
    fn work(&self) -> Result<super::AnimationWork, AnimationError> {
        let items = match self.output {
            Output::Geometry(_) => 1,
            Output::Drawing { lines, .. } => lines,
        };
        let bytes = items
            .checked_mul(std::mem::size_of::<super::GeometryLine>())
            .and_then(|n| n.checked_add(std::mem::size_of::<V>()))
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>() + self.callback_bytes))
            .and_then(|n| n.checked_add(6 * std::mem::size_of::<usize>()))
            .and_then(|n| {
                self.figures
                    .capacity()
                    .checked_mul(2 * (std::mem::size_of::<FigureId>() + 1))
                    .and_then(|figures| n.checked_add(figures))
            })
            .and_then(|n| {
                self.dependencies
                    .capacity()
                    .checked_mul(std::mem::size_of::<GeometryProviderDependency>())
                    .and_then(|dependencies| n.checked_add(dependencies))
            })
            .and_then(|n| {
                self.dependency_lookup
                    .capacity()
                    .checked_mul(2 * (std::mem::size_of::<GeometryProviderDependency>() + 1))
                    .and_then(|lookup| n.checked_add(lookup))
            })
            .ok_or(AnimationError::BudgetExceeded(
                super::AnimationBudgetKind::SnapshotBytes,
            ))?;
        Ok(super::AnimationWork {
            items,
            bytes,
            operations: items.checked_add(self.operation_limit()?).ok_or(
                AnimationError::BudgetExceeded(super::AnimationBudgetKind::GeometryOperations),
            )?,
            ..Default::default()
        })
    }
    fn evaluate(
        &self,
        context: &GeometryProviderContext<'_>,
    ) -> Result<DerivedValue, AnimationError> {
        let value = (self.derive)(context)?;
        if !value.is_valid() {
            return Err(AnimationError::InvalidValue);
        }
        let (patch, content) = match &self.output {
            Output::Geometry(apply) => (apply(&value)?.validate()?, None),
            Output::Drawing { lines, draw } => {
                let mut builder =
                    super::GeometryDrawingBuilder::new(self.key.figure, TypeId::of::<V>(), *lines)?;
                draw(&value, context, &mut builder)?;
                (GeometryPatch::default(), Some(builder.finish()?))
            }
        };
        Ok(DerivedValue {
            value: Arc::new(value),
            patch,
            content,
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct ProviderGraph {
    providers: Vec<Arc<dyn ErasedGeometryProvider>>,
    schedule: Option<Vec<usize>>,
}

#[derive(Clone, Default)]
pub(crate) struct ProviderEvaluation {
    pub(crate) patches: HashMap<FigureId, GeometryPatch>,
    pub(crate) values: super::geometry_view::GeometryValues,
    pub(crate) content: HashMap<FigureId, crate::figure::FigurePresentation>,
}

impl ProviderGraph {
    pub(crate) fn push<V: AnimationValue + Copy>(&mut self, provider: GeometryProvider<V>) {
        self.providers.push(provider.erased());
        self.schedule = None;
    }

    pub(crate) fn append(&mut self, mut other: Self) {
        self.providers.append(&mut other.providers);
        self.schedule = None;
    }

    pub(crate) fn figures(&self) -> impl Iterator<Item = FigureId> + '_ {
        self.providers.iter().flat_map(|provider| {
            provider
                .figures()
                .iter()
                .copied()
                .chain(std::iter::once(provider.key().figure))
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.providers.len()
    }
    pub(crate) fn drawing_figures(&self) -> impl Iterator<Item = FigureId> + '_ {
        self.providers
            .iter()
            .filter(|provider| provider.is_drawing())
            .map(|provider| provider.key().figure)
    }
    pub(crate) fn validate(&mut self) -> Result<(), AnimationError> {
        let mut drawings = HashSet::new();
        if self
            .drawing_figures()
            .any(|figure| !drawings.insert(figure))
        {
            return Err(AnimationError::ChannelConflict);
        }
        self.schedule = Some(self.order()?);
        Ok(())
    }
    pub(crate) fn edges(&self) -> Result<usize, AnimationError> {
        self.providers.iter().try_fold(0usize, |count, provider| {
            count
                .checked_add(provider.dependencies().len())
                .and_then(|n| n.checked_add(provider.figures().len()))
                .ok_or(AnimationError::BudgetExceeded(
                    super::AnimationBudgetKind::DependencyEdges,
                ))
        })
    }

    pub(crate) fn work(&self) -> Result<super::AnimationWork, AnimationError> {
        let work = self
            .providers
            .iter()
            .try_fold(super::AnimationWork::default(), |work, provider| {
                work.combine(super::AnimationWork::default(), provider.work()?)
            })?;
        // Reserve scheduling scratch, stable schedule and both evaluation tables. Four
        // slots per entry cover geometric Vec growth and hash-table control/load overhead.
        let slots = std::mem::size_of::<(GeometryProviderDependency, usize)>()
            + std::mem::size_of::<Vec<usize>>()
            + 4 * std::mem::size_of::<usize>()
            + std::mem::size_of::<(FigureId, GeometryPatch)>()
            + std::mem::size_of::<(GeometryProviderDependency, Arc<dyn Any + Send + Sync>)>()
            + std::mem::size_of::<(FigureId, crate::figure::FigurePresentation)>();
        let scratch = self
            .len()
            .checked_mul(slots)
            .and_then(|n| n.checked_mul(4))
            .and_then(|n| {
                self.edges()
                    .ok()?
                    .checked_mul(
                        4 * (std::mem::size_of::<GeometryProviderDependency>()
                            + std::mem::size_of::<usize>()
                            + 1),
                    )
                    .and_then(|edges| n.checked_add(edges))
            })
            .and_then(|n| {
                self.providers
                    .capacity()
                    .checked_mul(std::mem::size_of::<Arc<dyn ErasedGeometryProvider>>())
                    .and_then(|providers| n.checked_add(providers))
            })
            .ok_or(AnimationError::BudgetExceeded(
                super::AnimationBudgetKind::SnapshotBytes,
            ))?;
        work.combine(
            super::AnimationWork::default(),
            super::AnimationWork {
                bytes: scratch,
                ..Default::default()
            },
        )
    }

    /// Linear Kahn scheduling with stable discovery order.
    fn order(&self) -> Result<Vec<usize>, AnimationError> {
        let mut keys = HashMap::new();
        for (index, provider) in self.providers.iter().enumerate() {
            if keys.insert(provider.key(), index).is_some() {
                return Err(AnimationError::ChannelConflict);
            }
        }
        let mut pending = vec![0_usize; self.len()];
        let mut consumers = vec![Vec::new(); self.len()];
        for (index, provider) in self.providers.iter().enumerate() {
            let mut seen = HashSet::new();
            for dependency in provider.dependencies() {
                if !seen.insert(dependency) {
                    continue;
                }
                let source = *keys
                    .get(dependency)
                    .ok_or(AnimationError::UndeclaredProviderDependency)?;
                if !provider.is_drawing() && self.providers[source].is_drawing() {
                    return Err(AnimationError::InvalidComposition);
                }
                pending[index] += 1;
                consumers[source].push(index);
            }
        }
        let mut ready: std::collections::VecDeque<_> = pending
            .iter()
            .enumerate()
            .filter_map(|(index, count)| (*count == 0).then_some(index))
            .collect();
        let mut order = Vec::with_capacity(self.len());
        while let Some(index) = ready.pop_front() {
            order.push(index);
            for consumer in &consumers[index] {
                pending[*consumer] -= 1;
                if pending[*consumer] == 0 {
                    ready.push_back(*consumer);
                }
            }
        }
        if order.len() != self.len() {
            return Err(AnimationError::GeometryDependencyCycle);
        }
        Ok(order)
    }

    pub(crate) fn evaluate(
        &self,
        geometry: &PresentationGeometry,
        rebuild: impl FnOnce(
            &HashMap<FigureId, GeometryPatch>,
        ) -> Result<PresentationGeometry, AnimationError>,
    ) -> Result<ProviderEvaluation, AnimationError> {
        let mut values = HashMap::new();
        let mut patches: HashMap<FigureId, GeometryPatch> = HashMap::new();
        let mut content = HashMap::new();
        let mut rebuild = Some(rebuild);
        let schedule = self
            .schedule
            .as_ref()
            .ok_or(AnimationError::GeometryNotPrepared)?;
        let mut rebuilt = None;
        for drawing in [false, true] {
            if drawing && self.providers.iter().any(|provider| provider.is_drawing()) {
                rebuilt = Some(rebuild.take().expect("one drawing phase")(&patches)?);
            }
            for index in schedule
                .iter()
                .filter(|index| self.providers[**index].is_drawing() == drawing)
            {
                let provider = &self.providers[*index];
                let geometry = rebuilt.as_ref().unwrap_or(geometry);
                geometry.figure(provider.key().figure)?;
                for figure in provider.figures() {
                    geometry.figure(*figure)?;
                }
                let context = GeometryProviderContext {
                    geometry,
                    figures: provider.figures(),
                    dependencies: provider.dependency_lookup(),
                    values: &values,
                    key: provider.key(),
                    operations: Cell::new(0),
                    operation_limit: provider.operation_limit()?,
                };
                let result = provider.evaluate(&context)?;
                context.spend(0)?;
                patches
                    .entry(provider.key().figure)
                    .or_default()
                    .merge(result.patch)?;
                if let Some(drawing) = result.content
                    && content.insert(provider.key().figure, drawing).is_some()
                {
                    return Err(AnimationError::ChannelConflict);
                }
                values.insert(provider.key(), result.value);
            }
        }
        Ok(ProviderEvaluation {
            patches,
            values: values
                .into_iter()
                .map(|(key, value)| ((key.figure, key.value_type), value))
                .collect(),
            content,
        })
    }
}
