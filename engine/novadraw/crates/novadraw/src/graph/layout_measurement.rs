use super::*;
use std::collections::HashMap;

pub(super) fn admit_layout_dependencies(
    container: FigureId,
    manager: &dyn LayoutManager,
    budget: &crate::figure::PreparationBudgetHandle,
    is_child: impl Fn(FigureId) -> bool,
) -> Result<Option<Box<crate::layout::LayoutDependencyTable>>, LayoutError> {
    let mut declarations = crate::layout::LayoutDependencyDeclaration::new(budget.clone());
    manager
        .register_child_layout_dependencies(&mut declarations)
        .map_err(LayoutError::DependencyRegistration)?;
    declarations
        .finish(container, is_child)
        .map_err(LayoutError::DependencyRegistration)
}

pub(crate) struct LayoutMeasurementSession<'a> {
    pub(crate) text: &'a mut dyn TextLayoutEngine,
    pub(crate) budget: &'a crate::figure::PreparationBudgetHandle,
}

pub(super) struct LocalLayoutValidation {
    source: FigureId,
    property: crate::runtime::update::ErasedPropertyKey,
    proof: crate::figure::preparation::InputProof,
    request: crate::runtime::update::LocalLayoutRequest,
}

impl LayoutMeasurementSession<'_> {
    fn validate_guard(
        &self,
        tree: &FigureTree,
        proof: &crate::figure::preparation::InputProof,
    ) -> Result<(), LayoutError> {
        let scene = crate::figure::PreparationScene::with_lazy_styles(
            tree,
            tree.root_id(),
            self.budget.clone(),
        )
        .map_err(|error| LayoutError::Input(Box::new(error)))?;
        proof
            .validate(&scene, "local layout return")
            .map_err(|error| LayoutError::Input(Box::new(error)))
    }

    fn guard_proof(
        &self,
        tree: &FigureTree,
        guard: &crate::layout::LayoutRevalidationGuard,
    ) -> Result<Option<crate::figure::preparation::InputProof>, LayoutError> {
        let scene = crate::figure::PreparationScene::with_lazy_styles(
            tree,
            tree.root_id(),
            self.budget.clone(),
        )
        .map_err(|error| LayoutError::Input(Box::new(error)))?;
        guard
            .admit(&scene)
            .map_err(|error| LayoutError::Input(Box::new(error)))
    }

    fn layout(
        &mut self,
        tree: &FigureTree,
        container: FigureId,
        manager: &mut dyn LayoutManager,
        output: &mut LayoutOutput,
        guard: Option<&crate::figure::preparation::InputProof>,
    ) -> Result<Vec<crate::figure::preparation::InputProof>, LayoutError> {
        if !tree
            .capability_ids(PREPARATION)
            .any(|id| tree.blocks[id].preparation().is_some())
        {
            // Ordinary geometry queries need no style snapshot or subtree input log.
            // The storage outlives the read wrapper, without self-referential ownership.
            let scene = std::cell::OnceCell::new();
            let read = RuntimeLayoutRead {
                tree,
                scene: &scene,
                budget: self.budget,
                failure: RefCell::new(None),
            };
            if let Some(proof) = guard {
                read.checked_scene()?
                    .retain_input_proofs(std::slice::from_ref(proof))
                    .map_err(|error| LayoutError::Input(Box::new(error)))?;
            }
            manager.layout(container, &LayoutSceneQuery::new(&read), output)?;
            if let Some(error) = read.failure.into_inner() {
                return Err(error);
            }
            if let Some(scene) = scene.get() {
                scene
                    .validate_input_proofs("layout consumption")
                    .map_err(|error| LayoutError::Input(Box::new(error)))?;
                return scene
                    .input_proofs()
                    .map_err(|error| LayoutError::Input(Box::new(error)));
            }
            self.budget
                .check()
                .map_err(|error| LayoutError::Input(Box::new(error.into())))?;
            return Ok(Vec::new());
        }
        let preparation_error = |error| {
            LayoutError::Preparation(Box::new(crate::figure::PreparationMeasureError {
                figure: container,
                outer_constraints: MeasureConstraints::UNBOUNDED,
                content_constraints: None,
                kind: crate::figure::PreparationMeasureErrorKind::Text(Box::new(error)),
            }))
        };
        let scene = crate::figure::PreparationScene::with_lazy_styles(
            tree,
            tree.root_id(),
            self.budget.clone(),
        )
        .map_err(preparation_error)?;
        if let Some(proof) = guard {
            scene
                .retain_input_proofs(std::slice::from_ref(proof))
                .map_err(preparation_error)?;
        }
        let mut context =
            crate::text::MeasureContext::new(self.text, crate::text::FontDescriptor::default());
        let read = PreparationLayoutRead {
            tree,
            scene: &scene,
            context: RefCell::new(&mut context),
        };
        manager.layout(container, &LayoutSceneQuery::new(&read), output)?;
        scene
            .validate_input_proofs("layout consumption")
            .map_err(preparation_error)?;
        self.budget
            .check()
            .map_err(|error| preparation_error(error.into()))?;
        scene.input_proofs().map_err(preparation_error)
    }
}

fn prepared_measurement(
    figure: FigureId,
    node: &FigureNode,
    measurement: FigureMeasurement,
) -> Result<FigureMeasurement, LayoutError> {
    let measurement = crate::layout::validate_measurement(figure, measurement)?;
    if node.border_snapshot.is_some() {
        owner_scoped_border_measurement(figure, measurement, node.border_snapshot.as_ref())
    } else if node.figure.get_border().is_some() {
        border_measurement(figure, measurement, node.insets(), node.border_preferred)
    } else {
        Ok(measurement)
    }
}

/// A layout calculation in the same frozen measurement session as its preparation producer.
struct PreparationLayoutRead<'a, 'tree, 'text> {
    tree: &'a FigureTree,
    scene: &'a crate::figure::PreparationScene<'tree>,
    context: RefCell<&'a mut crate::text::MeasureContext<'text>>,
}

impl crate::layout::LayoutSceneRead for PreparationLayoutRead<'_, '_, '_> {
    fn read_input(
        &self,
        _source: FigureId,
        read: &dyn Fn(
            &crate::figure::PreparationScene<'_>,
        )
            -> Result<crate::figure::PreparationInputSnapshot, crate::text::TextError>,
    ) -> Result<crate::figure::PreparationInputSnapshot, LayoutError> {
        read(self.scene).map_err(|error| LayoutError::Input(Box::new(error)))
    }
    fn layout_direction(
        &self,
        figure: FigureId,
    ) -> Result<crate::figure::LayoutDirection, LayoutError> {
        self.scene.check_layout_member(figure)?;
        self.scene
            .observe(crate::connection::DependencySubject::FigureMeasurementSource(figure));
        Ok(self
            .scene
            .style(figure)
            .map_err(|error| {
                LayoutError::Preparation(Box::new(crate::figure::PreparationMeasureError {
                    figure,
                    outer_constraints: MeasureConstraints::UNBOUNDED,
                    content_constraints: None,
                    kind: crate::figure::PreparationMeasureErrorKind::Text(Box::new(error)),
                }))
            })?
            .layout_direction)
    }
    fn get_children(&self, parent: FigureId) -> Result<Vec<(FigureId, Rectangle)>, LayoutError> {
        self.scene.check_layout_member(parent)?;
        let children = crate::layout::LayoutSceneRead::get_children(self.tree, parent)?;
        for (child, _) in &children {
            self.scene
                .observe(crate::connection::DependencySubject::FigureGeometry(*child));
        }
        Ok(children)
    }

    fn get_constraint(
        &self,
        child: FigureId,
    ) -> Result<Option<&dyn LayoutConstraint>, LayoutError> {
        self.scene.check_layout_member(child)?;
        self.scene
            .observe(crate::connection::DependencySubject::FigureMeasurementSource(child));
        crate::layout::LayoutSceneRead::get_constraint(self.tree, child)
    }

    fn preferred_measurement(
        &self,
        figure: FigureId,
        constraints: MeasureConstraints,
    ) -> Result<FigureMeasurement, LayoutError> {
        self.tree.validate_identity(figure)?;
        self.scene
            .measure_selected(
                &mut self.context.borrow_mut(),
                figure,
                constraints,
                crate::figure::preparation::MeasurementSelection::Preferred,
            )
            .map(|measurement| measurement.preferred)
            .map_err(|error| LayoutError::Preparation(Box::new(error)))
    }

    fn minimum_size(
        &self,
        figure: FigureId,
        constraints: MeasureConstraints,
    ) -> Result<Dimension, LayoutError> {
        self.tree.validate_identity(figure)?;
        self.scene
            .measure_selected(
                &mut self.context.borrow_mut(),
                figure,
                constraints,
                crate::figure::preparation::MeasurementSelection::Minimum,
            )
            .map(|measurement| measurement.minimum)
            .map_err(|error| LayoutError::Preparation(Box::new(error)))
    }

    fn maximum_size(&self, figure: FigureId) -> Result<Dimension, LayoutError> {
        self.scene.check_layout_member(figure)?;
        self.scene
            .observe(crate::connection::DependencySubject::FigureMeasurementSource(figure));
        self.tree.maximum_size(figure)
    }

    fn get_container_bounds(&self, figure: FigureId) -> Result<Rectangle, LayoutError> {
        self.scene.check_layout_member(figure)?;
        self.scene
            .observe(crate::connection::DependencySubject::FigureGeometry(figure));
        crate::layout::LayoutSceneRead::get_container_bounds(self.tree, figure)
    }

    fn get_freeform_extent(&self, figure: FigureId) -> Result<Option<Rectangle>, LayoutError> {
        self.scene.check_layout_member(figure)?;
        self.scene
            .observe(crate::connection::DependencySubject::FigureGeometry(figure));
        crate::layout::LayoutSceneRead::get_freeform_extent(self.tree, figure)
    }

    fn get_content_scale(&self, figure: FigureId) -> Result<Option<f64>, LayoutError> {
        self.scene.check_layout_member(figure)?;
        self.scene
            .observe(crate::connection::DependencySubject::FigureMeasurementSource(figure));
        crate::layout::LayoutSceneRead::get_content_scale(self.tree, figure)
    }
}

/// A checked Runtime query for trees without preparation producers. Private input
/// admission is lazy; nested manager measurements retain this same query.
struct RuntimeLayoutRead<'a> {
    tree: &'a FigureTree,
    scene: &'a std::cell::OnceCell<crate::figure::PreparationScene<'a>>,
    budget: &'a crate::figure::PreparationBudgetHandle,
    failure: RefCell<Option<LayoutError>>,
}

impl RuntimeLayoutRead<'_> {
    fn measurement(
        &self,
        figure: FigureId,
        constraints: MeasureConstraints,
        minimum: bool,
    ) -> Result<FigureMeasurement, LayoutError> {
        self.tree.validate_identity(figure)?;
        let cache_key = slotmap::DefaultKey::from(figure.local());
        let node = &self.tree.blocks[figure];
        let slot = usize::from(minimum);
        let cacheable = node.layout.manager.is_some()
            && if minimum {
                node.minimum_size.is_none()
            } else {
                node.preferred_size.is_none()
            };
        let content = if cacheable {
            node.layout_content_constraints(figure, constraints)?
        } else {
            constraints
        };
        let generation = node.layout.generation();
        if cacheable && self.scene.get().is_none() {
            // Borrow the receipt: the checked reader cannot replace this cache.
            let cache = self.tree.input_free_measurements.borrow();
            let cached = cache.get(cache_key).and_then(|records| records[slot]);
            if let Some(record) = cached
                && record.matches(generation, content)
                // Once private inputs are admitted, callbacks remain observable.
                // A cached size cannot certify an unknown callback's side effects.
                && self.scene.get().is_none()
            {
                return Ok(record.measurement);
            }
        }
        let query = LayoutSceneQuery::new(self);
        let measurement = if minimum {
            let size = self
                .tree
                .minimum_size_in(figure, constraints, Some(&query))?;
            FigureMeasurement::new(size.width, size.height, None)
        } else {
            self.tree
                .preferred_measurement_in(figure, constraints, Some(&query))?
        };
        if cacheable && self.scene.get().is_none() {
            let record = Some(CachedMeasurement {
                generation,
                constraints: content,
                measurement,
            });
            let mut cache = self.tree.input_free_measurements.borrow_mut();
            if !cache.contains_key(cache_key) {
                self.budget
                    .allocate(
                        "ordinary measurement cache",
                        (std::mem::size_of::<slotmap::DefaultKey>()
                            + std::mem::size_of::<[Option<CachedMeasurement>; 2]>()
                            + 2 * std::mem::size_of::<usize>()) as u64,
                    )
                    .map_err(|error| LayoutError::Input(Box::new(error.into())))?;
            }
            cache.entry(cache_key).expect("live arena key").or_default()[slot] = record;
        }
        Ok(measurement)
    }

    fn checked_scene(&self) -> Result<&crate::figure::PreparationScene<'_>, LayoutError> {
        if let Some(error) = self.failure.borrow().as_ref() {
            return Err(error.clone());
        }
        let result: Result<_, LayoutError> = (|| {
            if self.scene.get().is_none() {
                let scene = crate::figure::PreparationScene::with_lazy_styles(
                    self.tree,
                    self.tree.root_id(),
                    self.budget.clone(),
                )
                .map_err(|error| LayoutError::Input(Box::new(error)))?;
                let _ = self.scene.set(scene);
            }
            Ok(self.scene.get().expect("scene initialized"))
        })();
        if let Err(error) = &result {
            *self.failure.borrow_mut() = Some(error.clone());
        }
        result
    }
}

impl crate::layout::LayoutSceneRead for RuntimeLayoutRead<'_> {
    fn read_input(
        &self,
        _source: FigureId,
        read: &dyn Fn(
            &crate::figure::PreparationScene<'_>,
        )
            -> Result<crate::figure::PreparationInputSnapshot, crate::text::TextError>,
    ) -> Result<crate::figure::PreparationInputSnapshot, LayoutError> {
        read(self.checked_scene()?).map_err(|error| LayoutError::Input(Box::new(error)))
    }

    fn layout_direction(
        &self,
        figure: FigureId,
    ) -> Result<crate::figure::LayoutDirection, LayoutError> {
        crate::layout::LayoutSceneRead::layout_direction(self.tree, figure)
    }
    fn get_children(&self, parent: FigureId) -> Result<Vec<(FigureId, Rectangle)>, LayoutError> {
        crate::layout::LayoutSceneRead::get_children(self.tree, parent)
    }
    fn get_constraint(
        &self,
        child: FigureId,
    ) -> Result<Option<&dyn LayoutConstraint>, LayoutError> {
        crate::layout::LayoutSceneRead::get_constraint(self.tree, child)
    }
    fn preferred_measurement(
        &self,
        figure: FigureId,
        constraints: MeasureConstraints,
    ) -> Result<FigureMeasurement, LayoutError> {
        self.measurement(figure, constraints, false)
    }
    fn minimum_size(
        &self,
        figure: FigureId,
        constraints: MeasureConstraints,
    ) -> Result<Dimension, LayoutError> {
        self.measurement(figure, constraints, true)
            .map(|measurement| measurement.size())
    }
    fn maximum_size(&self, figure: FigureId) -> Result<Dimension, LayoutError> {
        self.tree.maximum_size(figure)
    }
    fn get_container_bounds(&self, figure: FigureId) -> Result<Rectangle, LayoutError> {
        crate::layout::LayoutSceneRead::get_container_bounds(self.tree, figure)
    }
    fn get_freeform_extent(&self, figure: FigureId) -> Result<Option<Rectangle>, LayoutError> {
        crate::layout::LayoutSceneRead::get_freeform_extent(self.tree, figure)
    }
    fn get_content_scale(&self, figure: FigureId) -> Result<Option<f64>, LayoutError> {
        crate::layout::LayoutSceneRead::get_content_scale(self.tree, figure)
    }
}

impl FigureTree {
    /// Construction validation owns a bounded input session just like Runtime
    /// validation. Standalone read queries still have no publication session.
    pub(super) fn validate_subtree_with_inputs(
        &mut self,
        container: FigureId,
    ) -> Result<(), LayoutError> {
        self.validate_identity(container)?;
        let mut updates = UpdateManager::with_namespace(self.namespace());
        let mut text = crate::text::TextSystem::default();
        let budget = crate::figure::PreparationBudgetHandle::default();
        let mut measurement = LayoutMeasurementSession {
            text: text.parts().0,
            budget: &budget,
        };
        self.begin_derived_preparation(true);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.invalidate_stale_layout_inputs(&mut updates, &budget)
                .map_err(|error| LayoutError::Input(Box::new(error)))?;
            self.revalidate_with_update(&mut updates, container, Some(&mut measurement), None)?;
            self.validate_preparation_inputs(&budget)
                .map_err(|error| LayoutError::Input(Box::new(error)))
        }));
        let success = matches!(&result, Ok(Ok(())));
        let recovery = self.finish_derived_preparation(success);
        match result {
            Ok(result) => {
                recovery?;
                result
            }
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub(crate) fn measure_preparation_figure(
        &self,
        context: &mut crate::text::MeasureContext<'_>,
        figure: FigureId,
        outer: MeasureConstraints,
        scene: &crate::figure::PreparationScene<'_>,
        selection: crate::figure::preparation::MeasurementSelection,
    ) -> Result<crate::figure::PreparationMeasurement, crate::figure::PreparationMeasureError> {
        stacker::maybe_grow(
            RECURSIVE_VALIDATION_STACK_RED_ZONE,
            RECURSIVE_VALIDATION_STACK_GROWTH,
            || self.measure_preparation_figure_inner(context, figure, outer, scene, selection),
        )
    }

    fn measure_preparation_figure_inner(
        &self,
        context: &mut crate::text::MeasureContext<'_>,
        figure: FigureId,
        outer: MeasureConstraints,
        scene: &crate::figure::PreparationScene<'_>,
        selection: crate::figure::preparation::MeasurementSelection,
    ) -> Result<crate::figure::PreparationMeasurement, crate::figure::PreparationMeasureError> {
        use crate::figure::{
            PreparationMeasureError, PreparationMeasureErrorKind as Kind,
            PreparationMeasureStage as Stage, PreparationMeasurement,
        };
        let node = &self.blocks[figure];
        let prepared_source = node.preparation().is_some() || node.prepared.is_some();
        let preferred_selected = selection.preferred();
        let minimum_selected = selection.minimum();
        let preferred_prepared = preferred_selected
            && node.preferred_size.is_none()
            && node.layout.manager.is_none()
            && prepared_source;
        let minimum_prepared = minimum_selected
            && node.minimum_size.is_none()
            && node.layout.manager.is_none()
            && prepared_source;
        let fail = |content, kind| PreparationMeasureError {
            figure,
            outer_constraints: outer,
            content_constraints: content,
            kind,
        };
        let layout_error = |content, stage, error| {
            if let LayoutError::Preparation(error) = error {
                return *error;
            }
            fail(
                content,
                match error {
                    LayoutError::InvalidMeasurement {
                        reason: crate::layout::MeasurementFailure::DerivedOverflow,
                        ..
                    } => Kind::DerivedOverflow { stage },
                    error => Kind::Layout(Box::new(error)),
                },
            )
        };
        let layout_selected = node.layout.manager.is_some()
            && ((preferred_selected && node.preferred_size.is_none())
                || (minimum_selected && node.minimum_size.is_none()));
        let content = if preferred_prepared || minimum_prepared || layout_selected {
            Some(
                node.layout_content_constraints(figure, outer)
                    .map_err(|error| layout_error(None, Stage::ConstraintShrink, error))?,
            )
        } else {
            None
        };
        let prepared = if preferred_prepared || minimum_prepared {
            let content = content.expect("selected preparation has content constraints");
            let preparation = node
                .preparation()
                .ok_or_else(|| fail(Some(content), Kind::UnsupportedConstraintMeasurement))?;
            let measured_scene =
                scene
                    .for_measurement(figure, outer, content)
                    .map_err(|error| {
                        fail(
                            Some(content),
                            match error {
                                TextError::PreparationDependency(error) => Kind::Dependency(error),
                                error => Kind::Text(Box::new(error)),
                            },
                        )
                    })?;
            let style = scene
                .style(figure)
                .map_err(|error| fail(Some(content), Kind::Text(Box::new(error))))?;
            let font = style.font.clone();
            let mut child_context = context.reborrow_with_font(font);
            let measured = preparation
                .measure_scene(&mut child_context, content, &measured_scene)
                .map_err(|mut error| {
                    // Preserve a nested measurement's actual subject and constraints.
                    if error.figure == figure {
                        error.outer_constraints = outer;
                        error.content_constraints = Some(content);
                    }
                    error
                })?;
            scene.budget().check().map_err(|error| {
                fail(
                    Some(content),
                    Kind::Dependency(Box::new(crate::figure::PreparationDependencyError::Budget(
                        error,
                    ))),
                )
            })?;
            let measured = measured.validate(
                figure,
                outer,
                Some(content),
                preferred_prepared,
                minimum_prepared,
            )?;
            Some(measured)
        } else {
            None
        };
        let layout_scene = PreparationLayoutRead {
            tree: self,
            scene,
            context: RefCell::new(context),
        };
        let query = LayoutSceneQuery::new(&layout_scene);
        let preferred = if !preferred_selected {
            FigureMeasurement::default()
        } else if let Some(size) = node.preferred_size {
            let raw = FigureMeasurement::new(size.width, size.height, None);
            PreparationMeasurement {
                preferred: raw,
                minimum: Dimension::ZERO,
            }
            .validate(figure, outer, content, true, false)?;
            validate_projected_measurement(figure, node.project_preferred_measurement(raw))
                .map_err(|error| layout_error(content, Stage::MeasurementProjection, error))?
        } else if preferred_prepared {
            let raw = prepared.expect("selected preparation measured").preferred;
            let expanded = prepared_measurement(figure, node, raw)
                .map_err(|error| layout_error(content, Stage::BorderExpansion, error))?;
            validate_projected_measurement(figure, node.project_preferred_measurement(expanded))
                .map_err(|error| layout_error(content, Stage::MeasurementProjection, error))?
        } else if let Some(layout) = node.layout.manager.as_deref() {
            let raw = layout
                .preferred_measurement(figure, content.unwrap(), &query)
                .and_then(|raw| crate::layout::validate_measurement(figure, raw))
                .map_err(|error| layout_error(content, Stage::MeasurementProjection, error))?;
            node.project_layout_preferred_measurement(figure, raw)
                .map_err(|error| layout_error(content, Stage::MeasurementProjection, error))?
        } else {
            self.preferred_measurement(figure, outer)
                .map_err(|error| layout_error(content, Stage::MeasurementProjection, error))?
        };
        let minimum = if !minimum_selected {
            Dimension::ZERO
        } else if let Some(size) = node.minimum_size {
            PreparationMeasurement {
                preferred: FigureMeasurement::default(),
                minimum: size,
            }
            .validate(figure, outer, content, false, true)?;
            validate_projected_size(figure, node.project_minimum_size(size))
                .map_err(|error| layout_error(content, Stage::MeasurementProjection, error))?
        } else if minimum_prepared {
            let raw = prepared.expect("selected preparation measured").minimum;
            let expanded = prepared_measurement(
                figure,
                node,
                FigureMeasurement::new(raw.width, raw.height, None),
            )
            .map_err(|error| layout_error(content, Stage::BorderExpansion, error))?
            .size();
            validate_projected_size(figure, node.project_minimum_size(expanded))
                .map_err(|error| layout_error(content, Stage::MeasurementProjection, error))?
        } else if let Some(layout) = node.layout.manager.as_deref() {
            let raw = layout
                .minimum_size(figure, content.unwrap(), &query)
                .and_then(|raw| crate::layout::validate_measurement_size(figure, raw, false))
                .map_err(|error| layout_error(content, Stage::MeasurementProjection, error))?;
            node.project_layout_minimum_size(figure, raw)
                .map_err(|error| layout_error(content, Stage::MeasurementProjection, error))?
        } else {
            self.minimum_size(figure, outer)
                .map_err(|error| layout_error(content, Stage::MeasurementProjection, error))?
        };
        PreparationMeasurement { preferred, minimum }.validate(
            figure,
            outer,
            content,
            preferred_selected,
            minimum_selected,
        )
    }

    pub fn freeform_extent(&self, figure_id: FigureId) -> Result<Rectangle, FreeformError> {
        self.validate_identity(figure_id)?;
        let block = self
            .blocks
            .get(figure_id)
            .ok_or(FreeformError::UnknownFigure(figure_id))?;
        let state = block
            .layout
            .freeform
            .as_ref()
            .ok_or(FreeformError::NotFreeform(figure_id))?;
        if state.extent_generation.is_none() {
            return Err(FreeformError::Unvalidated(figure_id));
        }
        Ok(state.cached_extent)
    }

    /// Derives a freeform extent while omitting complete child subtrees.
    ///
    /// This is a read-only projection over the current validated geometry. It is useful when a
    /// transient visual replaces, rather than supplements, an existing subtree.
    pub fn freeform_extent_excluding(
        &self,
        figure_id: FigureId,
        excluded: &[FigureId],
    ) -> Result<Rectangle, FreeformError> {
        self.validate_identity(figure_id)?;
        for &id in excluded {
            self.validate_identity(id)?;
        }
        let block = self
            .blocks
            .get(figure_id)
            .ok_or(FreeformError::UnknownFigure(figure_id))?;
        let state = block
            .layout
            .freeform
            .as_ref()
            .ok_or(FreeformError::NotFreeform(figure_id))?;
        if state.extent_generation.is_none() {
            return Err(FreeformError::Unvalidated(figure_id));
        }
        let projection = self.begin_projection_scope();
        let extent = self
            .derive_freeform_extent_excluding(figure_id, excluded)?
            .unwrap_or(Rectangle::ZERO);
        projection.finish(self, "freeform extent query return")?;
        Ok(extent)
    }

    fn derive_freeform_extent_excluding(
        &self,
        figure_id: FigureId,
        excluded: &[FigureId],
    ) -> Result<Option<Rectangle>, FreeformError> {
        let block = self.node(figure_id)?;
        let mut extent: Option<Rectangle> = None;
        for child_id in block.children.iter().copied() {
            if excluded.contains(&child_id) {
                continue;
            }
            let child = &self.blocks[child_id];
            let contribution = if child.layout.freeform.is_some() {
                let child_extent = self
                    .derive_freeform_extent_excluding(child_id, excluded)?
                    .unwrap_or(Rectangle::ZERO);
                let bounds = child.figure_bounds();
                let transform = Affine2D::from_translation(bounds.x, bounds.y)
                    * self.checked_child_transform(child_id)?;
                transform_rectangle(transform, child_extent)
                    .ok_or(FreeformError::NonFiniteGeometry(child_id))?
            } else {
                child.figure_bounds()
            };
            if !finite_rectangle(contribution) {
                return Err(FreeformError::NonFiniteGeometry(child_id));
            }
            let combined = match extent {
                Some(current) => current.union(contribution),
                None => contribution,
            };
            if !finite_rectangle(combined) {
                return Err(FreeformError::NonFiniteGeometry(figure_id));
            }
            extent = Some(combined);
        }
        Ok(extent)
    }

    fn recompute_freeform_extent(&mut self, figure_id: FigureId) -> Result<(), LayoutError> {
        let dirty = self
            .blocks
            .get(figure_id)
            .and_then(|block| block.layout.freeform.as_ref())
            .is_some_and(|state| state.dirty);
        if !dirty {
            return Ok(());
        }

        let children = self.blocks[figure_id].children.clone();
        let mut extent: Option<Rectangle> = None;
        for child_id in children {
            if self.blocks[child_id].layout.freeform.is_some() {
                self.recompute_freeform_extent(child_id)?;
            }
            let child = &self.blocks[child_id];
            let contribution = if let Some(state) = child.layout.freeform.as_ref() {
                let child_extent = state.cached_extent;
                let bounds = child.figure_bounds();
                let transform = Affine2D::from_translation(bounds.x, bounds.y)
                    * self.checked_child_transform(child_id)?;
                transform_rectangle(transform, child_extent)
                    .ok_or(LayoutError::NonFiniteGeometry { figure: child_id })?
            } else {
                child.figure_bounds()
            };
            if !finite_rectangle(contribution) {
                return Err(LayoutError::NonFiniteGeometry { figure: child_id });
            }
            extent = Some(match extent {
                Some(current) => current.union(contribution),
                None => contribution,
            });
        }

        let new_extent = extent.unwrap_or(Rectangle::ZERO);
        if !finite_rectangle(new_extent) {
            return Err(LayoutError::NonFiniteGeometry { figure: figure_id });
        }
        let (old_extent, changed) = {
            let block = &mut self.blocks[figure_id];
            let generation = block.layout.generation();
            let state = block
                .layout
                .freeform
                .as_mut()
                .expect("dirty freeform state must exist");
            let old_extent = state.cached_extent;
            state.cached_extent = new_extent;
            state.extent_generation = Some(generation);
            state.dirty = false;
            (old_extent, old_extent != new_extent)
        };
        if changed {
            self.record_property_change(
                figure_id,
                FREEFORM_EXTENT_PROPERTY,
                old_extent,
                new_extent,
            );
        }
        Ok(())
    }

    /// Executes the graph-owned validation phase while UpdateManager owns only
    /// the pending work queue and phase trigger.
    #[cfg(test)]
    pub(crate) fn perform_validation_cycle(
        &mut self,
        update_manager: &mut UpdateManager,
    ) -> Result<(), ValidationError> {
        self.perform_validation_cycle_with_budget(update_manager, DEFAULT_VALIDATION_BUDGET)
    }

    #[cfg(test)]
    pub(crate) fn perform_validation_cycle_with_budget(
        &mut self,
        update_manager: &mut UpdateManager,
        budget: usize,
    ) -> Result<(), ValidationError> {
        self.perform_validation_cycle_measured(update_manager, budget, None)
    }

    pub(crate) fn perform_validation_cycle_measured(
        &mut self,
        update_manager: &mut UpdateManager,
        budget: usize,
        mut measurement: Option<&mut LayoutMeasurementSession<'_>>,
    ) -> Result<(), ValidationError> {
        let mut processed = 0;
        let mut invalidation_chain = Vec::new();
        loop {
            let (figure_ids, requests) = update_manager.drain_layout_requests();
            if figure_ids.is_empty() {
                return Ok(());
            }
            let remaining = budget.saturating_sub(processed);
            if figure_ids.len() > remaining {
                invalidation_chain.extend(figure_ids.iter().take(remaining).copied());
                for figure_id in &figure_ids {
                    update_manager.add_invalid_figure(*figure_id);
                }
                return Err(ValidationError::NonConvergingValidation {
                    budget,
                    invalidation_chain,
                });
            }
            processed += figure_ids.len();
            invalidation_chain.extend(figure_ids.iter().copied());

            for figure_id in &figure_ids {
                if !requests.contains_key(figure_id) {
                    self.mark_validation_path_invalid(*figure_id);
                }
            }

            let mut local_requests = HashMap::new();
            for (source, request) in requests {
                if request.guard.source() == source
                    && self.property_layout_chain_permits(source, request.change.property())
                    && measurement.is_some()
                {
                    local_requests.insert(source, request);
                } else {
                    self.mark_validation_path_invalid(source);
                }
            }

            let mut validation_roots: Vec<FigureId> = figure_ids
                .iter()
                .copied()
                .filter_map(|figure_id| self.validation_root(figure_id))
                .collect();
            validation_roots.sort_by_key(|id| self.depth(*id).unwrap_or(usize::MAX));
            validation_roots.dedup();

            for root_id in validation_roots {
                // Qualification occurs immediately before this subtree's first
                // callback, after any earlier root has finished its callbacks.
                let request = local_requests.remove(&root_id);
                let was_local = request.is_some();
                let local = if let Some(request) = request
                    && self.property_layout_chain_permits(root_id, request.change.property())
                    && let Some(session) = measurement.as_deref_mut()
                {
                    match session.guard_proof(self, &request.guard) {
                        Ok(proof) => proof.map(|proof| LocalLayoutValidation {
                            source: root_id,
                            property: request.change.property(),
                            proof,
                            request,
                        }),
                        Err(error) => {
                            for id in &figure_ids {
                                self.mark_invalid(update_manager, *id);
                            }
                            return Err(error.into());
                        }
                    }
                } else {
                    None
                };
                let root_id = if local.is_some() {
                    root_id
                } else {
                    if was_local {
                        self.mark_validation_path_invalid(root_id);
                    }
                    self.validation_root(root_id).unwrap_or(root_id)
                };
                if let Err(error) = self.revalidate_with_update(
                    update_manager,
                    root_id,
                    measurement.as_deref_mut(),
                    local.as_ref(),
                ) {
                    for id in &figure_ids {
                        self.mark_invalid(update_manager, *id);
                    }
                    return Err(error.into());
                }
            }
        }
    }

    fn validation_root(&self, figure_id: FigureId) -> Option<FigureId> {
        let mut current = figure_id;
        let mut root = figure_id;
        loop {
            let block = self.blocks.get(current)?;
            let Some(parent_id) = block.parent else {
                return Some(root);
            };
            let parent = self.blocks.get(parent_id)?;
            if parent.is_valid {
                return Some(root);
            }
            root = parent_id;
            current = parent_id;
        }
    }

    pub(crate) fn request_property_layout(
        &mut self,
        updates: &mut UpdateManager,
        request: crate::runtime::update::LocalLayoutRequest,
    ) {
        let source = request.change.figure_id();
        if request.guard.source() != source
            || self
                .blocks
                .get(source)
                .is_none_or(|node| !node.is_valid && !updates.has_local_layout_request(source))
            || !self.property_layout_chain_permits(source, request.change.property())
            || !updates.add_local_layout_request(request)
        {
            self.mark_invalid(updates, source);
            return;
        }
        self.mark_validation_path_invalid_until(source, LayoutInvalidation::Geometry, Some(source));
    }

    fn property_layout_chain_permits(
        &self,
        source: FigureId,
        property: crate::runtime::update::ErasedPropertyKey,
    ) -> bool {
        let Some(source_node) = self.blocks.get(source) else {
            return false;
        };
        if source_node.layout.freeform.is_some() {
            return false;
        }
        let mut child = source;
        let mut parent = source_node.parent;
        while let Some(id) = parent {
            let Some(node) = self.blocks.get(id) else {
                return false;
            };
            if !node.is_valid
                || node.layout.freeform.is_some()
                || node.capabilities.get(LIFECYCLE).ok().flatten().is_some()
                || node.capabilities.get(PREPARATION).ok().flatten().is_some()
            {
                return false;
            }
            if node.layout.manager.is_some()
                && !node
                    .layout
                    .child_dependencies
                    .as_ref()
                    .is_some_and(|table| table.permits(child, property))
            {
                return false;
            }
            child = id;
            parent = node.parent;
        }
        true
    }

    pub(super) fn mark_internal_layout_geometry(
        &mut self,
        updates: &mut UpdateManager,
        child: FigureId,
        local: &LocalLayoutValidation,
    ) {
        self.mark_validation_path_invalid_until(
            child,
            LayoutInvalidation::Geometry,
            Some(local.source),
        );
        if !updates.add_local_layout_request(local.request.clone()) {
            self.mark_invalid(updates, local.source);
        }
    }

    fn revalidate_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        container_id: FigureId,
        measurement: Option<&mut LayoutMeasurementSession<'_>>,
        local: Option<&LocalLayoutValidation>,
    ) -> Result<(), LayoutError> {
        let ancestors_visible = match self.parent_id(container_id)? {
            Some(parent) => self.is_effectively_visible(parent)?,
            None => true,
        };
        self.revalidate_with_update_inner(
            update_manager,
            container_id,
            ancestors_visible,
            0,
            measurement,
            local,
        )
    }

    fn revalidate_with_update_inner(
        &mut self,
        update_manager: &mut UpdateManager,
        container_id: FigureId,
        ancestors_visible: bool,
        depth: usize,
        measurement: Option<&mut LayoutMeasurementSession<'_>>,
        local: Option<&LocalLayoutValidation>,
    ) -> Result<(), LayoutError> {
        if depth.is_multiple_of(RECURSIVE_STACK_CHECK_INTERVAL) {
            return stacker::maybe_grow(
                RECURSIVE_VALIDATION_STACK_RED_ZONE,
                RECURSIVE_VALIDATION_STACK_GROWTH,
                || {
                    self.revalidate_with_update_body(
                        update_manager,
                        container_id,
                        ancestors_visible,
                        depth,
                        measurement,
                        local,
                    )
                },
            );
        }
        self.revalidate_with_update_body(
            update_manager,
            container_id,
            ancestors_visible,
            depth,
            measurement,
            local,
        )
    }

    fn revalidate_with_update_body(
        &mut self,
        update_manager: &mut UpdateManager,
        container_id: FigureId,
        ancestors_visible: bool,
        depth: usize,
        mut measurement: Option<&mut LayoutMeasurementSession<'_>>,
        local: Option<&LocalLayoutValidation>,
    ) -> Result<(), LayoutError> {
        if self
            .blocks
            .get(container_id)
            .is_none_or(|block| block.is_valid)
        {
            return Ok(());
        }
        if !ancestors_visible || !self.blocks[container_id].is_visible {
            return Ok(());
        }

        let prevalidate_children = self.blocks[container_id]
            .layout
            .manager
            .as_deref()
            .is_some_and(LayoutManager::requires_valid_children_before_layout);
        if prevalidate_children {
            self.revalidate_children_with_update(
                update_manager,
                container_id,
                depth,
                measurement.as_deref_mut(),
                local,
            )?;
        }

        let layout_manager = self
            .blocks
            .get_mut(container_id)
            .and_then(|block| block.layout.manager.take());
        if let Some(mut layout_manager) = layout_manager {
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Started,
                container_id,
                child_id: None,
            });
            let mut output = LayoutOutput::new();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if let Some(session) = measurement.as_deref_mut() {
                    session.layout(
                        self,
                        container_id,
                        layout_manager.as_mut(),
                        &mut output,
                        local.map(|local| &local.proof),
                    )
                } else {
                    let snapshot = LayoutSceneQuery::new(self);
                    layout_manager
                        .layout(container_id, &snapshot, &mut output)
                        .map(|()| Vec::new())
                }
            }));
            if let Some(block) = self.blocks.get_mut(container_id) {
                block.layout.manager = Some(layout_manager);
            }
            let input_proofs = match result {
                Ok(result) => result?,
                Err(payload) => std::panic::resume_unwind(payload),
            };
            self.apply_layout_output_scoped(update_manager, container_id, output, local)?;
            self.blocks[container_id].layout.input_proofs = input_proofs;
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Finished,
                container_id,
                child_id: None,
            });
        }

        self.revalidate_children_with_update(
            update_manager,
            container_id,
            depth,
            measurement.as_deref_mut(),
            local,
        )?;
        self.recompute_freeform_extent(container_id)?;
        if let Some(block) = self.blocks.get_mut(container_id) {
            let bounds = block.figure_bounds();
            if let Some(lifecycle) = block.lifecycle() {
                lifecycle.validate(bounds);
            }
        }
        if let Some(local) = local
            && local.source == container_id
            && let Some(session) = measurement
        {
            session.validate_guard(self, &local.proof)?;
            crate::figure::preparation::merge_proofs(
                &mut self.blocks[container_id].layout.input_proofs,
                std::slice::from_ref(&local.proof),
                session.budget,
            )
            .map_err(|error| LayoutError::Input(Box::new(error)))?;
        }
        if let Some(block) = self.blocks.get_mut(container_id) {
            block.is_valid = true;
            block.layout.mark_validated();
        }
        Ok(())
    }

    fn revalidate_children_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        parent_id: FigureId,
        parent_depth: usize,
        mut measurement: Option<&mut LayoutMeasurementSession<'_>>,
        local: Option<&LocalLayoutValidation>,
    ) -> Result<(), LayoutError> {
        let children = self
            .blocks
            .get(parent_id)
            .map(|block| block.children.clone())
            .unwrap_or_default();
        for child_id in children {
            self.revalidate_with_update_inner(
                update_manager,
                child_id,
                true,
                parent_depth + 1,
                measurement.as_deref_mut(),
                local,
            )?;
        }
        Ok(())
    }

    pub(crate) fn validate_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        container_id: FigureId,
    ) -> Result<(), LayoutError> {
        self.revalidate_with_update(update_manager, container_id, None, None)
    }

    #[cfg(test)]
    pub(crate) fn revalidate(&mut self, container_id: FigureId) {
        self.try_revalidate(container_id)
            .expect("layout validation failed");
    }

    #[cfg(test)]
    pub(crate) fn try_revalidate(&mut self, container_id: FigureId) -> Result<(), LayoutError> {
        let ancestors_visible = match self.parent_id(container_id)? {
            Some(parent) => self.is_effectively_visible(parent)?,
            None => true,
        };
        self.try_revalidate_inner(container_id, ancestors_visible, 0)
    }

    #[cfg(test)]
    fn try_revalidate_inner(
        &mut self,
        container_id: FigureId,
        ancestors_visible: bool,
        depth: usize,
    ) -> Result<(), LayoutError> {
        if depth.is_multiple_of(RECURSIVE_STACK_CHECK_INTERVAL) {
            return stacker::maybe_grow(
                RECURSIVE_VALIDATION_STACK_RED_ZONE,
                RECURSIVE_VALIDATION_STACK_GROWTH,
                || self.try_revalidate_body(container_id, ancestors_visible, depth),
            );
        }
        self.try_revalidate_body(container_id, ancestors_visible, depth)
    }

    #[cfg(test)]
    fn try_revalidate_body(
        &mut self,
        container_id: FigureId,
        ancestors_visible: bool,
        depth: usize,
    ) -> Result<(), LayoutError> {
        if self
            .blocks
            .get(container_id)
            .is_none_or(|block| block.is_valid)
        {
            return Ok(());
        }
        if !ancestors_visible || !self.blocks[container_id].is_visible {
            return Ok(());
        }

        let prevalidate_children = self.blocks[container_id]
            .layout
            .manager
            .as_deref()
            .is_some_and(LayoutManager::requires_valid_children_before_layout);
        if prevalidate_children {
            let children = self.blocks[container_id].children.clone();
            for child_id in children {
                self.try_revalidate_inner(child_id, true, depth + 1)?;
            }
        }

        let layout_manager = self
            .blocks
            .get_mut(container_id)
            .and_then(|block| block.layout.manager.take());
        if let Some(mut layout_manager) = layout_manager {
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Started,
                container_id,
                child_id: None,
            });
            let mut output = LayoutOutput::new();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let snapshot = LayoutSceneQuery::new(self);
                layout_manager.layout(container_id, &snapshot, &mut output)
            }));
            if let Some(block) = self.blocks.get_mut(container_id) {
                block.layout.manager = Some(layout_manager);
            }
            match result {
                Ok(result) => result?,
                Err(payload) => std::panic::resume_unwind(payload),
            }
            self.apply_layout_output_without_update(container_id, output)?;
            self.blocks[container_id].layout.input_proofs.clear();
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Finished,
                container_id,
                child_id: None,
            });
        }

        let children = self
            .blocks
            .get(container_id)
            .map(|block| block.children.clone())
            .unwrap_or_default();
        for child_id in children {
            self.try_revalidate_inner(child_id, true, depth + 1)?;
        }
        self.recompute_freeform_extent(container_id)?;
        if let Some(block) = self.blocks.get_mut(container_id) {
            let bounds = block.figure_bounds();
            if let Some(lifecycle) = block.lifecycle() {
                lifecycle.validate(bounds);
            }
            block.is_valid = true;
            block.layout.mark_validated();
        }
        Ok(())
    }

    pub(crate) fn validate_layout_output(
        &self,
        container_id: FigureId,
        output: &LayoutOutput,
    ) -> Result<(), LayoutError> {
        for change in &output.changes {
            let child_id = match change {
                LayoutChange::Bounds(child_id, bounds) => {
                    if !finite_rectangle(*bounds) || bounds.width < 0.0 || bounds.height < 0.0 {
                        return Err(LayoutError::NonFiniteGeometry { figure: *child_id });
                    }
                    Some(*child_id)
                }
                LayoutChange::Visibility(child_id, _) | LayoutChange::Invalidate(child_id) => {
                    Some(*child_id)
                }
                LayoutChange::Property(event) => {
                    if event.figure_id() != container_id {
                        return Err(LayoutError::InvalidChild {
                            container: container_id,
                            child: event.figure_id(),
                        });
                    }
                    None
                }
                LayoutChange::CoordinateSystemChanged(figure)
                | LayoutChange::Repaint(figure)
                | LayoutChange::RepaintParent(figure) => {
                    if *figure != container_id {
                        return Err(LayoutError::InvalidChild {
                            container: container_id,
                            child: *figure,
                        });
                    }
                    None
                }
                LayoutChange::ViewportEffect(effect) => {
                    if effect.viewport() != container_id {
                        return Err(LayoutError::InvalidChild {
                            container: container_id,
                            child: effect.viewport(),
                        });
                    }
                    None
                }
            };
            if let Some(child_id) = child_id
                && self
                    .blocks
                    .get(child_id)
                    .is_none_or(|child| child.parent != Some(container_id))
            {
                return Err(LayoutError::InvalidChild {
                    container: container_id,
                    child: child_id,
                });
            }
        }
        Ok(())
    }

    pub(crate) fn apply_layout_output(
        &mut self,
        update_manager: &mut UpdateManager,
        container_id: FigureId,
        output: LayoutOutput,
    ) -> Result<(), LayoutError> {
        self.apply_layout_output_scoped(update_manager, container_id, output, None)
    }

    fn apply_layout_output_scoped(
        &mut self,
        update_manager: &mut UpdateManager,
        container_id: FigureId,
        output: LayoutOutput,
        local: Option<&LocalLayoutValidation>,
    ) -> Result<(), LayoutError> {
        self.validate_layout_output(container_id, &output)?;
        // Decide the entire output's range before any geometry is written.
        // Visibility and arbitrary effects cannot certify external measurements.
        let requested_local = local.is_some();
        let local = local.filter(|local| {
            self.property_layout_chain_permits(local.source, local.property)
                && output.changes.iter().all(|change| {
                    match change {
                        LayoutChange::Bounds(_, _)
                        | LayoutChange::Repaint(_)
                        | LayoutChange::RepaintParent(_) => true,
                        // An unchanged visibility write has no invalidation,
                        // notification or measurement effect.
                        LayoutChange::Visibility(id, visible) => {
                            self.blocks[*id].is_visible == *visible
                        }
                        _ => false,
                    }
                })
        });
        if requested_local && local.is_none() {
            self.mark_invalid(update_manager, container_id);
        }
        for change in output.changes {
            match change {
                LayoutChange::Bounds(child_id, bounds) => {
                    // Connection points own their bounds; parent placement must not
                    // move the normalized route away from its anchors.
                    if self.blocks[child_id].connection().is_some() {
                        continue;
                    }
                    self.set_bounds_with_update_scoped(update_manager, child_id, bounds, local);
                }
                LayoutChange::Visibility(child_id, visible) => {
                    self.set_visible_with_update(update_manager, child_id, visible);
                }
                LayoutChange::Invalidate(child_id) => {
                    self.mark_invalid(update_manager, child_id);
                }
                LayoutChange::Property(event) => {
                    self.record_property_event(event);
                }
                LayoutChange::CoordinateSystemChanged(figure) => {
                    self.record_coordinate_system_changed(figure);
                }
                LayoutChange::Repaint(figure) => {
                    self.repaint(update_manager, figure, None);
                }
                LayoutChange::RepaintParent(figure) => {
                    if let Some(parent) = self.parent_id(figure)? {
                        self.repaint(update_manager, parent, None);
                    }
                }
                LayoutChange::ViewportEffect(effect) => {
                    self.record_derived_viewport(&effect);
                    effect.commit()?;
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn apply_layout_output_without_update(
        &mut self,
        container_id: FigureId,
        output: LayoutOutput,
    ) -> Result<(), LayoutError> {
        self.validate_layout_output(container_id, &output)?;
        for change in output.changes {
            match change {
                LayoutChange::Bounds(child_id, bounds) => {
                    // Keep the same points-owned geometry contract without an updater.
                    if self.blocks[child_id].connection().is_some() {
                        continue;
                    }
                    let old_bounds = self.figure_bounds(child_id);
                    self.set_bounds(child_id, bounds.x, bounds.y, bounds.width, bounds.height);
                    if old_bounds
                        .is_ok_and(|old| old.width != bounds.width || old.height != bounds.height)
                        && let Some(child) = self.blocks.get_mut(child_id)
                    {
                        child.is_valid = false;
                    }
                }
                LayoutChange::Visibility(child_id, visible) => {
                    self.set_visible(child_id, visible);
                }
                LayoutChange::Invalidate(child_id) => {
                    self.mark_validation_path_invalid(child_id);
                }
                LayoutChange::Property(event) => {
                    self.record_property_event(event);
                }
                LayoutChange::CoordinateSystemChanged(figure) => {
                    self.record_coordinate_system_changed(figure);
                }
                LayoutChange::Repaint(_) | LayoutChange::RepaintParent(_) => {}
                LayoutChange::ViewportEffect(effect) => effect.commit()?,
            }
        }
        Ok(())
    }

    pub fn is_layout_valid(&self) -> bool {
        self.blocks
            .get(self.contents.unwrap_or(self.root))
            .map(|block| block.is_valid)
            .unwrap_or(true)
    }

    pub fn is_valid(&self, figure_id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.node(figure_id)?.is_valid)
    }

    pub fn preferred_measurement(
        &self,
        figure_id: FigureId,
        constraints: MeasureConstraints,
    ) -> Result<FigureMeasurement, LayoutError> {
        self.preferred_measurement_in(figure_id, constraints, None)
    }

    fn preferred_measurement_in(
        &self,
        figure_id: FigureId,
        constraints: MeasureConstraints,
        query: Option<&LayoutSceneQuery<'_>>,
    ) -> Result<FigureMeasurement, LayoutError> {
        self.validate_identity(figure_id)?;
        let block = &self.blocks[figure_id];
        if let Some(size) = block.preferred_size {
            let raw = crate::layout::validate_measurement(
                figure_id,
                FigureMeasurement::new(size.width, size.height, None),
            )?;
            return validate_projected_measurement(
                figure_id,
                block.project_preferred_measurement(raw),
            );
        }
        if let Some(layout) = block.layout.manager.as_deref() {
            let constraints = block.layout_content_constraints(figure_id, constraints)?;
            let generation = block.layout.generation();
            if query.is_none()
                && let Some(cached) = block.layout.cache.borrow().preferred
                && cached.matches(generation, constraints)
            {
                return Ok(cached.measurement);
            }
            let snapshot = LayoutSceneQuery::new(self);
            let raw = crate::layout::validate_measurement(
                figure_id,
                layout.preferred_measurement(figure_id, constraints, query.unwrap_or(&snapshot))?,
            )?;
            let measurement = block.project_layout_preferred_measurement(figure_id, raw)?;
            if query.is_none() {
                block.layout.cache.borrow_mut().preferred = Some(CachedMeasurement {
                    generation,
                    constraints,
                    measurement,
                });
            }
            return Ok(measurement);
        }
        let constraints = block.layout_constraints(constraints);
        if let Some(prepared) = &block.prepared {
            return validate_projected_measurement(
                figure_id,
                prepared_measurement(figure_id, block, prepared.presentation.measurement())?,
            );
        }
        let measurement = if block.figure.get_border().is_some() {
            block.figure.intrinsic_content_measurement(constraints)
        } else {
            block.figure.intrinsic_measurement(constraints)
        };
        let measurement = crate::layout::validate_measurement(figure_id, measurement)?;
        validate_projected_measurement(
            figure_id,
            prepared_measurement(figure_id, block, measurement)?,
        )
    }

    pub fn minimum_size(
        &self,
        figure_id: FigureId,
        constraints: MeasureConstraints,
    ) -> Result<Dimension, LayoutError> {
        self.minimum_size_in(figure_id, constraints, None)
    }

    fn minimum_size_in(
        &self,
        figure_id: FigureId,
        constraints: MeasureConstraints,
        query: Option<&LayoutSceneQuery<'_>>,
    ) -> Result<Dimension, LayoutError> {
        self.validate_identity(figure_id)?;
        let block = &self.blocks[figure_id];
        if let Some(size) = block.minimum_size {
            crate::layout::validate_measurement_size(figure_id, size, false)?;
            return validate_projected_size(figure_id, block.project_minimum_size(size));
        }
        if let Some(layout) = block.layout.manager.as_deref() {
            let constraints = block.layout_content_constraints(figure_id, constraints)?;
            let generation = block.layout.generation();
            if query.is_none()
                && let Some(cached) = block.layout.cache.borrow().minimum
                && cached.matches(generation, constraints)
            {
                return Ok(cached.measurement.size());
            }
            let snapshot = LayoutSceneQuery::new(self);
            let raw = layout.minimum_size(figure_id, constraints, query.unwrap_or(&snapshot))?;
            crate::layout::validate_measurement_size(figure_id, raw, false)?;
            let size = block.project_layout_minimum_size(figure_id, raw)?;
            if query.is_none() {
                block.layout.cache.borrow_mut().minimum = Some(CachedMeasurement {
                    generation,
                    constraints,
                    measurement: FigureMeasurement::new(size.width, size.height, None),
                });
            }
            return Ok(size);
        }
        let constraints = block.layout_constraints(constraints);
        if let Some(prepared) = &block.prepared {
            let size = prepared.presentation.minimum_size();
            return Ok(validate_projected_measurement(
                figure_id,
                prepared_measurement(
                    figure_id,
                    block,
                    FigureMeasurement::new(size.width, size.height, None),
                )?,
            )?
            .size());
        }
        let measurement = if block.figure.get_border().is_some() {
            block
                .figure
                .intrinsic_content_minimum_measurement(constraints)
        } else {
            block.figure.intrinsic_minimum_measurement(constraints)
        };
        let measurement = crate::layout::validate_measurement(figure_id, measurement)?;
        let projected = validate_projected_measurement(
            figure_id,
            prepared_measurement(figure_id, block, measurement)?,
        )?;
        Ok(projected.size())
    }

    pub fn maximum_size(&self, figure_id: FigureId) -> Result<Dimension, LayoutError> {
        self.validate_identity(figure_id)?;
        let block = &self.blocks[figure_id];
        crate::layout::validate_measurement_size(
            figure_id,
            block
                .maximum_size
                .unwrap_or(Dimension::new(f64::INFINITY, f64::INFINITY)),
            true,
        )
    }

    pub(crate) fn set_preferred_size(
        &mut self,
        figure_id: FigureId,
        size: Option<Dimension>,
    ) -> bool {
        let Some(block) = self.blocks.get_mut(figure_id) else {
            return false;
        };
        if block.preferred_size == size {
            return false;
        }
        block.preferred_size = size;
        self.mark_validation_path_invalid_for(figure_id, LayoutInvalidation::ExplicitSize);
        true
    }

    pub(crate) fn set_minimum_size(
        &mut self,
        figure_id: FigureId,
        size: Option<Dimension>,
    ) -> bool {
        let Some(block) = self.blocks.get_mut(figure_id) else {
            return false;
        };
        if block.minimum_size == size {
            return false;
        }
        block.minimum_size = size;
        self.mark_validation_path_invalid_for(figure_id, LayoutInvalidation::ExplicitSize);
        true
    }

    pub(crate) fn set_maximum_size(
        &mut self,
        figure_id: FigureId,
        size: Option<Dimension>,
    ) -> bool {
        let Some(block) = self.blocks.get_mut(figure_id) else {
            return false;
        };
        if block.maximum_size == size {
            return false;
        }
        block.maximum_size = size;
        self.mark_validation_path_invalid_for(figure_id, LayoutInvalidation::ExplicitSize);
        true
    }

    pub(crate) fn replace_layout_manager(
        &mut self,
        container: FigureId,
        layout_manager: Option<Box<dyn LayoutManager>>,
    ) -> bool {
        self.try_replace_layout_manager(
            container,
            layout_manager,
            &crate::figure::PreparationBudgetHandle::default(),
        )
        .expect("internal layout manager admission")
    }

    pub(crate) fn try_replace_layout_manager(
        &mut self,
        container: FigureId,
        layout_manager: Option<Box<dyn LayoutManager>>,
        budget: &crate::figure::PreparationBudgetHandle,
    ) -> Result<bool, LayoutError> {
        self.validate_identity(container)?;
        let dependencies = if let Some(manager) = layout_manager.as_deref() {
            self.validate_layout_manager_constraints(container, manager)?;
            admit_layout_dependencies(container, manager, budget, |child| {
                self.blocks
                    .get(child)
                    .is_some_and(|node| node.parent == Some(container))
            })?
        } else {
            None
        };
        let Some(block) = self.blocks.get_mut(container) else {
            unreachable!("validated layout owner");
        };
        if block.layout.manager.is_none() && layout_manager.is_none() {
            return Ok(false);
        }
        block.layout.manager = layout_manager;
        block.layout.child_dependencies = dependencies;
        // A replacement owns a new layout computation. In particular, clearing
        // the manager changes the container to freeform and retires old reads.
        block.layout.input_proofs.clear();
        block.layout.source_generation = block.layout.source_generation.wrapping_add(1);
        self.mark_validation_path_invalid_for(container, LayoutInvalidation::Structure);
        Ok(true)
    }

    pub(crate) fn validate_layout_manager_constraints(
        &self,
        container: FigureId,
        layout_manager: &dyn LayoutManager,
    ) -> Result<(), LayoutError> {
        self.validate_identity(container)?;
        let block = &self.blocks[container];
        for (child, constraint) in &block.layout.constraints {
            layout_manager.validate_constraint(container, *child, constraint.as_ref())?;
        }
        Ok(())
    }

    pub fn layout_manager(
        &self,
        container: FigureId,
    ) -> Result<Option<&dyn LayoutManager>, TreeQueryError> {
        Ok(self.node(container)?.layout.manager.as_deref())
    }

    pub(super) fn validate_layout_child(&self, child: FigureId) -> Result<FigureId, LayoutError> {
        self.validate_identity(child)?;
        let child_node = &self.blocks[child];
        child_node.parent.ok_or(LayoutError::InvalidChild {
            container: self.root,
            child,
        })
    }

    pub(super) fn validate_layout_constraint(
        &self,
        child: FigureId,
        constraint: &dyn LayoutConstraint,
    ) -> Result<(), LayoutError> {
        let parent = self.validate_layout_child(child)?;
        if let Some(manager) = self.layout_manager(parent)? {
            manager.validate_constraint(parent, child, constraint)?;
        }
        Ok(())
    }

    pub(crate) fn set_boxed_constraint(
        &mut self,
        child_id: FigureId,
        constraint: Box<dyn LayoutConstraint>,
    ) -> bool {
        let Some(parent_id) = self.blocks.get(child_id).and_then(|child| child.parent) else {
            return false;
        };
        let Some(parent) = self.blocks.get_mut(parent_id) else {
            return false;
        };
        parent.layout.constraints.insert(child_id, constraint);
        parent.layout.source_generation = parent.layout.source_generation.wrapping_add(1);
        self.mark_validation_path_invalid_for(parent_id, LayoutInvalidation::Constraint);
        self.emit_layout_event(LayoutEvent {
            kind: LayoutEventKind::ConstraintChanged,
            container_id: parent_id,
            child_id: Some(child_id),
        });
        true
    }

    pub fn layout_constraint<C>(&self, child_id: FigureId) -> Result<Option<&C>, LayoutError>
    where
        C: LayoutConstraint,
    {
        self.validate_identity(child_id)?;
        let Some(constraint) = self.constraint(child_id) else {
            return Ok(None);
        };
        constraint.as_any().downcast_ref::<C>().map(Some).ok_or(
            LayoutError::ConstraintTypeMismatch {
                container: self.blocks[child_id]
                    .parent
                    .expect("stored constraint has a parent"),
                child: child_id,
                expected: std::any::type_name::<C>(),
                actual: constraint.type_name(),
            },
        )
    }

    pub(crate) fn remove_constraint(&mut self, child_id: FigureId) -> bool {
        let Some(parent_id) = self.blocks.get(child_id).and_then(|child| child.parent) else {
            return false;
        };
        let removed = self
            .blocks
            .get_mut(parent_id)
            .and_then(|parent| parent.layout.constraints.remove(&child_id))
            .is_some();
        if removed {
            let layout = &mut self.blocks[parent_id].layout;
            layout.source_generation = layout.source_generation.wrapping_add(1);
            self.mark_validation_path_invalid_for(parent_id, LayoutInvalidation::Constraint);
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::ConstraintChanged,
                container_id: parent_id,
                child_id: Some(child_id),
            });
        }
        removed
    }

    fn constraint(&self, child_id: FigureId) -> Option<&dyn LayoutConstraint> {
        let parent_id = self.blocks.get(child_id)?.parent?;
        self.blocks
            .get(parent_id)?
            .layout
            .constraints
            .get(&child_id)
            .map(Box::as_ref)
    }

    pub(super) fn nearest_freeform_ancestor(&self, figure_id: FigureId) -> Option<FigureId> {
        let parent = self.blocks.get(figure_id)?.parent?;
        self.blocks[parent].layout.freeform.as_ref()?;
        Some(parent)
    }

    pub(super) fn mark_freeform_ancestor_extents_dirty(&mut self, figure_id: FigureId) {
        let mut current = self.blocks.get(figure_id).and_then(|block| block.parent);
        while let Some(id) = current {
            let Some(block) = self.blocks.get_mut(id) else {
                break;
            };
            if block.layout.freeform.is_none() {
                break;
            }
            block.is_valid = false;
            block.layout.invalidate(LayoutInvalidation::Geometry);
            current = block.parent;
        }
    }

    pub(super) fn mark_validation_path_invalid(&mut self, figure_id: FigureId) {
        self.mark_validation_path_invalid_for(figure_id, LayoutInvalidation::Geometry);
    }

    pub(super) fn mark_validation_path_invalid_for(
        &mut self,
        figure_id: FigureId,
        reason: LayoutInvalidation,
    ) {
        self.mark_validation_path_invalid_until(figure_id, reason, None);
    }

    fn mark_validation_path_invalid_until(
        &mut self,
        mut figure_id: FigureId,
        reason: LayoutInvalidation,
        boundary: Option<FigureId>,
    ) {
        let mut invalidated = Vec::new();
        loop {
            let measurements = self.input_free_measurements.get_mut();
            if !measurements.is_empty() {
                measurements.remove(slotmap::DefaultKey::from(figure_id.local()));
            }
            let (parent, was_valid) = if let Some(block) = self.blocks.get_mut(figure_id) {
                let was_valid = block.is_valid;
                block.is_valid = false;
                block.layout.invalidate(reason);
                if was_valid && let Some(lifecycle) = block.lifecycle() {
                    lifecycle.invalidate();
                }
                (block.parent, was_valid)
            } else {
                (None, false)
            };

            if was_valid {
                invalidated.push(figure_id);
            }
            if boundary == Some(figure_id) {
                break;
            }
            match parent {
                Some(parent_id) => figure_id = parent_id,
                None => break,
            }
        }
        for container_id in invalidated {
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Invalidated,
                container_id,
                child_id: None,
            });
        }
    }

    pub(crate) fn invalid_figure_ids(&self) -> Vec<FigureId> {
        self.blocks
            .iter()
            .filter_map(|(id, block)| (!block.is_valid).then_some(id))
            .collect()
    }

    pub(crate) fn invalid_validation_roots(&self) -> Vec<FigureId> {
        let mut roots = self
            .blocks
            .iter()
            .filter_map(|(id, block)| {
                if block.is_valid {
                    return None;
                }
                let parent_is_valid = block
                    .parent
                    .and_then(|parent| self.blocks.get(parent))
                    .is_some_and(|parent| parent.is_valid);
                (block.parent.is_none() || parent_is_valid).then_some(id)
            })
            .collect::<Vec<_>>();
        roots.sort_by_key(|id| self.depth(*id).unwrap_or(usize::MAX));
        roots
    }
}

impl crate::layout::LayoutSceneRead for FigureTree {
    fn layout_direction(
        &self,
        figure: FigureId,
    ) -> Result<crate::figure::LayoutDirection, LayoutError> {
        self.validate_identity(figure)?;
        Ok(self
            .resolved_style(figure)
            .expect("validated live Figure")
            .layout_direction)
    }
    fn get_children(&self, parent_id: FigureId) -> Result<Vec<(FigureId, Rectangle)>, LayoutError> {
        self.validate_identity(parent_id)?;
        let block = &self.blocks[parent_id];
        Ok(block
            .children
            .iter()
            .map(|&child_id| (child_id, self.blocks[child_id].figure_bounds()))
            .collect())
    }

    fn get_constraint(
        &self,
        child_id: FigureId,
    ) -> Result<Option<&dyn LayoutConstraint>, LayoutError> {
        self.validate_identity(child_id)?;
        Ok(self.constraint(child_id))
    }

    fn preferred_measurement(
        &self,
        figure_id: FigureId,
        constraints: MeasureConstraints,
    ) -> Result<FigureMeasurement, LayoutError> {
        self.preferred_measurement(figure_id, constraints)
    }

    fn minimum_size(
        &self,
        figure_id: FigureId,
        constraints: MeasureConstraints,
    ) -> Result<Dimension, LayoutError> {
        self.minimum_size(figure_id, constraints)
    }

    fn maximum_size(&self, figure_id: FigureId) -> Result<Dimension, LayoutError> {
        self.maximum_size(figure_id)
    }

    fn get_container_bounds(&self, container_id: FigureId) -> Result<Rectangle, LayoutError> {
        self.validate_identity(container_id)?;
        let block = &self.blocks[container_id];
        // Layout output lives in child-content coordinates, just like child measurement.
        // Apply the same open container projection after removing the physical border.
        let bounds = block.figure_bounds();
        let constraints = block.layout_content_constraints(
            container_id,
            MeasureConstraints::bounded(bounds.width, bounds.height).map_err(|_| {
                LayoutError::NonFiniteGeometry {
                    figure: container_id,
                }
            })?,
        )?;
        let area = Rectangle::new(
            0.0,
            0.0,
            constraints
                .max_width()
                .ok_or(LayoutError::NonFiniteGeometry {
                    figure: container_id,
                })?,
            constraints
                .max_height()
                .ok_or(LayoutError::NonFiniteGeometry {
                    figure: container_id,
                })?,
        );
        if !finite_rectangle(area) {
            return Err(LayoutError::NonFiniteGeometry {
                figure: container_id,
            });
        }
        Ok(area)
    }

    fn get_freeform_extent(&self, figure_id: FigureId) -> Result<Option<Rectangle>, LayoutError> {
        self.validate_identity(figure_id)?;
        match self.freeform_extent(figure_id) {
            Ok(extent) => Ok(Some(extent)),
            Err(FreeformError::NotFreeform(_)) => Ok(None),
            Err(error) => Err(LayoutError::Freeform(error)),
        }
    }

    fn get_content_scale(&self, figure_id: FigureId) -> Result<Option<f64>, LayoutError> {
        self.validate_identity(figure_id)?;
        Ok(self
            .capability(figure_id, SCALE)
            .map_err(LayoutError::Capability)?
            .map(|capability| capability.model().scale()))
    }
}

pub(super) fn validate_projected_measurement(
    figure: FigureId,
    measurement: FigureMeasurement,
) -> Result<FigureMeasurement, LayoutError> {
    if !measurement.width.is_finite()
        || !measurement.height.is_finite()
        || measurement
            .baseline
            .is_some_and(|baseline| !baseline.is_finite())
    {
        return Err(LayoutError::InvalidMeasurement {
            figure,
            reason: crate::layout::MeasurementFailure::DerivedOverflow,
        });
    }
    crate::layout::validate_measurement(figure, measurement)
}

pub(super) fn validate_projected_size(
    figure: FigureId,
    size: Dimension,
) -> Result<Dimension, LayoutError> {
    validate_projected_measurement(
        figure,
        FigureMeasurement::new(size.width, size.height, None),
    )
    .map(FigureMeasurement::size)
}

#[cfg(test)]
mod connection_placement_tests {
    use super::*;

    #[test]
    fn parent_placement_preserves_connection_geometry_in_both_commit_paths() {
        for with_update in [false, true] {
            let mut tree = FigureTree::new();
            let root = tree.set_contents(Box::new(
                crate::RectangleFigure::new(0.0, 0.0, 300.0, 200.0).expect("valid figure bounds"),
            ));
            let ordinary = tree.add_child_to(
                root,
                Box::new(
                    crate::RectangleFigure::new(0.0, 0.0, 10.0, 10.0).expect("valid figure bounds"),
                ),
            );
            let connection =
                tree.add_child_to(root, Box::new(crate::connection::ConnectionFigure::new()));
            let route_bounds = Rectangle::new(50.0, 25.0, 150.0, 75.0);
            tree.set_bounds(connection, 50.0, 25.0, 150.0, 75.0);
            let placement = Rectangle::new(240.0, 100.0, 0.0, 0.0);
            let mut output = LayoutOutput::default();
            output.set_child_bounds(ordinary, placement);
            output.set_child_bounds(connection, placement);
            output.set_child_visible(connection, false);
            if with_update {
                let mut updates = UpdateManager::with_namespace(tree.namespace());
                tree.apply_layout_output(&mut updates, root, output)
                    .unwrap();
            } else {
                tree.apply_layout_output_without_update(root, output)
                    .unwrap();
            }
            assert_eq!(tree.figure_bounds(ordinary).unwrap(), placement);
            assert_eq!(tree.figure_bounds(connection).unwrap(), route_bounds);
            assert!(!tree.is_visible(connection).unwrap());
        }
    }
}

#[cfg(test)]
mod property_layout_tests {
    use super::*;
    use crate::figure::{
        PreparationInputKey, PreparationInputPayload, PreparationInputRegistry, PreparationScene,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    };

    const VALUE: PropertyKey<f64> = PropertyKey::new("external.local", "value");
    const OTHER: PropertyKey<f64> = PropertyKey::new("external.local", "other");
    #[derive(Eq, PartialEq)]
    struct NonTarget(u64);
    impl PreparationInputPayload for NonTarget {
        fn retained_units(&self) -> u64 {
            0
        }
    }
    const INPUT: PreparationInputKey<NonTarget> = PreparationInputKey::new("external.non-target");

    struct Placement {
        independent: bool,
        calls: Arc<AtomicUsize>,
        width: Arc<AtomicU64>,
        visibility: bool,
    }
    impl LayoutManager for Placement {
        fn register_child_layout_dependencies(
            &self,
            out: &mut crate::layout::LayoutDependencyDeclaration,
        ) -> Result<(), crate::layout::LayoutDependencyRegistrationError> {
            if self.independent {
                out.independent_child_property(VALUE)?;
            }
            Ok(())
        }
        fn preferred_measurement(
            &self,
            _: FigureId,
            _: MeasureConstraints,
            _: &LayoutSceneQuery<'_>,
        ) -> Result<FigureMeasurement, LayoutError> {
            Ok(FigureMeasurement::new(100.0, 100.0, None))
        }
        fn minimum_size(
            &self,
            _: FigureId,
            _: MeasureConstraints,
            _: &LayoutSceneQuery<'_>,
        ) -> Result<Dimension, LayoutError> {
            Ok(Dimension::ZERO)
        }
        fn layout(
            &mut self,
            owner: FigureId,
            query: &LayoutSceneQuery<'_>,
            out: &mut LayoutOutput,
        ) -> Result<(), LayoutError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            for (child, _) in query.children(owner)? {
                out.set_child_bounds(
                    child,
                    Rectangle::new(0.0, 0.0, self.width.load(Ordering::Relaxed) as f64, 10.0),
                );
                if self.visibility {
                    out.set_child_visible(child, false);
                }
            }
            Ok(())
        }
    }
    struct Fixture {
        tree: FigureTree,
        updates: UpdateManager,
        owner: FigureId,
        source: FigureId,
        child: FigureId,
        inputs: PreparationInputRegistry,
        non_target: Arc<AtomicU64>,
        width: Arc<AtomicU64>,
        ancestor_calls: Arc<AtomicUsize>,
    }
    impl Fixture {
        fn new(independent: bool, visibility: bool) -> Self {
            let mut tree = FigureTree::new();
            let rectangle =
                || Box::new(crate::RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap());
            let owner = tree.set_contents(rectangle());
            let source = tree.add_child_to(owner, rectangle());
            let child = tree.add_child_to(source, rectangle());
            let ancestor_calls = Arc::new(AtomicUsize::new(0));
            let width = Arc::new(AtomicU64::new(10));
            tree.try_replace_layout_manager(
                owner,
                Some(Box::new(Placement {
                    independent,
                    calls: ancestor_calls.clone(),
                    width: Arc::new(AtomicU64::new(100)),
                    visibility: false,
                })),
                &crate::figure::PreparationBudgetHandle::new(Default::default()),
            )
            .unwrap();
            tree.try_replace_layout_manager(
                source,
                Some(Box::new(Placement {
                    independent: false,
                    calls: Arc::new(AtomicUsize::new(0)),
                    width: width.clone(),
                    visibility,
                })),
                &crate::figure::PreparationBudgetHandle::new(Default::default()),
            )
            .unwrap();
            let updates = UpdateManager::with_namespace(tree.namespace());
            let non_target = Arc::new(AtomicU64::new(1));
            let observed = non_target.clone();
            let mut inputs = PreparationInputRegistry::new();
            inputs
                .register(INPUT, move |_, _: &PreparationScene<'_>| {
                    Ok(NonTarget(observed.load(Ordering::Relaxed)))
                })
                .unwrap();
            let mut fixture = Self {
                tree,
                updates,
                owner,
                source,
                child,
                inputs,
                non_target,
                width,
                ancestor_calls,
            };
            fixture.tree.mark_invalid(&mut fixture.updates, owner);
            fixture.validate().unwrap();
            fixture.ancestor_calls.store(0, Ordering::Relaxed);
            fixture
        }
        fn request(&mut self, property: PropertyKey<f64>, first: u64) {
            let guard = crate::layout::LayoutRevalidationGuard::new(
                &self.inputs,
                INPUT,
                self.source,
                NonTarget(first),
            )
            .unwrap();
            self.tree.request_property_layout(
                &mut self.updates,
                crate::runtime::update::LocalLayoutRequest {
                    change: crate::TypedPropertyChange::new(self.source, property, 10.0, 20.0)
                        .erase(),
                    guard,
                },
            );
        }
        fn validate(&mut self) -> Result<(), ValidationError> {
            let mut text = crate::text::TextSystem::default();
            let budget = crate::figure::PreparationBudgetHandle::new(Default::default());
            let mut session = LayoutMeasurementSession {
                text: text.parts().0,
                budget: &budget,
            };
            self.tree
                .perform_validation_cycle_measured(&mut self.updates, 100, Some(&mut session))
        }
    }

    #[test]
    fn local_child_resize_converges_without_reinvalidating_declared_ancestors() {
        let mut f = Fixture::new(true, false);
        f.width.store(20, Ordering::Relaxed);
        f.request(VALUE, 1);
        assert!(f.tree.blocks[f.owner].is_valid);
        assert!(!f.tree.blocks[f.source].is_valid);
        f.validate().unwrap();
        assert_eq!(f.ancestor_calls.load(Ordering::Relaxed), 0);
        assert_eq!(f.tree.figure_bounds(f.child).unwrap().width, 20.0);
        assert!(f.tree.blocks[f.source].is_valid);
        assert!(!f.updates.has_pending_layout());
    }
    #[test]
    fn unknown_ancestor_and_changed_guard_preserve_full_invalidation() {
        for independent in [false, true] {
            let mut f = Fixture::new(independent, false);
            f.request(VALUE, 1);
            f.non_target.store(2, Ordering::Relaxed);
            f.validate().unwrap();
            assert!(f.ancestor_calls.load(Ordering::Relaxed) > 0);
        }
    }
    #[test]
    fn ordinary_request_and_different_typed_property_both_win() {
        for ordinary in [false, true] {
            let mut f = Fixture::new(true, false);
            f.request(VALUE, 1);
            if ordinary {
                f.tree.mark_invalid(&mut f.updates, f.source);
            } else {
                f.request(OTHER, 1);
            }
            f.validate().unwrap();
            assert!(f.ancestor_calls.load(Ordering::Relaxed) > 0);
        }
    }
    #[test]
    fn same_property_keeps_the_original_non_target_value() {
        let mut f = Fixture::new(true, false);
        f.request(VALUE, 1);
        f.non_target.store(2, Ordering::Relaxed);
        f.request(VALUE, 2);
        f.validate().unwrap();
        assert!(f.ancestor_calls.load(Ordering::Relaxed) > 0);
    }
    #[test]
    fn visibility_effect_upgrades_the_complete_output_before_geometry() {
        let mut f = Fixture::new(true, true);
        f.tree.set_visible(f.child, true);
        f.request(VALUE, 1);
        f.validate().unwrap();
        assert!(f.ancestor_calls.load(Ordering::Relaxed) > 0);
    }
    #[test]
    fn unchanged_visibility_output_has_no_external_layout_effect() {
        let mut f = Fixture::new(true, true);
        f.request(VALUE, 1);
        f.validate().unwrap();
        assert_eq!(f.ancestor_calls.load(Ordering::Relaxed), 0);
        assert!(!f.tree.is_visible(f.child).unwrap());
    }
    #[test]
    fn retired_registration_is_an_error_and_pending_work_survives() {
        let mut f = Fixture::new(true, false);
        f.request(VALUE, 1);
        f.inputs = PreparationInputRegistry::new();
        assert!(f.validate().is_err());
        assert!(f.updates.has_pending_layout());
        assert!(!f.tree.blocks[f.owner].is_valid);
        f.validate().unwrap();
        assert!(f.ancestor_calls.load(Ordering::Relaxed) > 0);
    }
    #[test]
    fn cleared_drained_and_deleted_queues_do_not_leave_local_metadata() {
        for operation in 0..3 {
            let mut f = Fixture::new(true, false);
            f.request(VALUE, 1);
            match operation {
                0 => f.updates.clear(),
                1 => {
                    assert_eq!(f.updates.drain_invalid_figures(), vec![f.source]);
                }
                _ => f
                    .updates
                    .forget_figures(&std::collections::HashSet::from([f.source])),
            }
            assert!(!f.updates.has_local_layout_request(f.source));
            assert!(!f.updates.has_pending_layout());
            f.updates.add_invalid_figure(f.source);
            f.validate().unwrap();
            assert!(f.ancestor_calls.load(Ordering::Relaxed) > 0);
        }
    }
    #[test]
    fn checkpoint_restore_conservatively_requeues_local_work_as_ordinary() {
        let mut f = Fixture::new(true, false);
        f.request(VALUE, 1);
        let checkpoint = f.updates.preparation_checkpoint();
        f.updates
            .restore_preparation_checkpoint(checkpoint, &f.tree);
        assert!(!f.updates.has_local_layout_request(f.source));
        f.validate().unwrap();
        assert!(f.ancestor_calls.load(Ordering::Relaxed) > 0);
    }
    #[test]
    fn declaration_replacement_is_rechecked_at_dequeue() {
        let mut f = Fixture::new(true, false);
        f.request(VALUE, 1);
        f.tree
            .try_replace_layout_manager(
                f.owner,
                Some(Box::new(Placement {
                    independent: false,
                    calls: f.ancestor_calls.clone(),
                    width: Arc::new(AtomicU64::new(100)),
                    visibility: false,
                })),
                &crate::figure::PreparationBudgetHandle::new(Default::default()),
            )
            .unwrap();
        f.validate().unwrap();
        assert!(f.ancestor_calls.load(Ordering::Relaxed) > 0);
    }
    #[test]
    fn exhausted_validation_budget_preserves_requests_as_ordinary() {
        let mut f = Fixture::new(true, false);
        f.request(VALUE, 1);
        assert!(
            f.tree
                .perform_validation_cycle_with_budget(&mut f.updates, 0)
                .is_err()
        );
        assert!(f.updates.has_pending_layout());
        assert!(!f.updates.has_local_layout_request(f.source));
        f.validate().unwrap();
        assert!(f.ancestor_calls.load(Ordering::Relaxed) > 0);
    }
}

#[cfg(test)]
mod s07_query_failure_tests {
    use super::*;
    use crate::RectangleFigure;
    use crate::identity::RuntimeHandle;
    use crate::layout::MeasurementFailure;

    fn subject() -> (FigureTree, FigureId) {
        let mut tree = FigureTree::new();
        let id = tree.set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 0.0, 0.0).expect("valid figure bounds"),
        ));
        (tree, id)
    }

    #[test]
    fn ordinary_geometry_layout_does_not_allocate_an_input_session() {
        let (mut tree, root) = subject();
        tree.add_child_to(
            root,
            Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0).unwrap()),
        );
        let mut text = crate::text::TextSystem::default();
        let budget = crate::figure::PreparationBudgetHandle::new(
            crate::figure::PreparationBudget::new(0, 0),
        );
        let mut session = LayoutMeasurementSession {
            text: text.parts().0,
            budget: &budget,
        };
        let mut output = LayoutOutput::new();
        let proofs = session
            .layout(
                &tree,
                root,
                &mut crate::layout::FillLayout,
                &mut output,
                None,
            )
            .unwrap();
        assert!(proofs.is_empty());
        assert!(!output.is_empty());
        assert_eq!(budget.used(), crate::figure::PreparationBudget::new(0, 0));
    }

    #[test]
    fn lazy_layout_input_admission_failure_is_latched_before_the_reader() {
        #[derive(Eq, PartialEq)]
        struct Value;
        impl crate::figure::PreparationInputPayload for Value {
            fn retained_units(&self) -> u64 {
                0
            }
        }
        let key = crate::figure::PreparationInputKey::<Value>::new("lazy-admission");
        let mut inputs = crate::figure::PreparationInputRegistry::new();
        inputs
            .register(
                key,
                |_: FigureId,
                 _: &crate::figure::PreparationScene<'_>|
                 -> Result<Value, crate::text::TextError> {
                    panic!("reader must not run after rejected session admission");
                },
            )
            .unwrap();
        let (tree, root) = subject();
        let scene = std::cell::OnceCell::new();
        let budget = crate::figure::PreparationBudgetHandle::new(
            crate::figure::PreparationBudget::new(0, 0),
        );
        let read = RuntimeLayoutRead {
            tree: &tree,
            scene: &scene,
            budget: &budget,
            failure: RefCell::new(None),
        };
        let query = LayoutSceneQuery::new(&read);
        let first = query.input(&inputs, key, root).unwrap_err();
        assert!(matches!(&first, LayoutError::Input(error)
            if matches!(error.as_ref(), crate::text::TextError::PreparationDependency(dependency)
                if matches!(dependency.as_ref(), crate::figure::PreparationDependencyError::Budget(_)))));
        assert_eq!(query.input(&inputs, key, root).unwrap_err(), first);
        assert_eq!(read.failure.into_inner(), Some(first));
        assert!(scene.get().is_none());
    }

    #[test]
    fn ordinary_cache_does_not_skip_callbacks_after_private_input_admission() {
        use crate::figure::{
            PreparationInputKey, PreparationInputPayload, PreparationInputRegistry,
        };
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        struct Count(Arc<AtomicUsize>);
        impl LayoutManager for Count {
            fn preferred_measurement(
                &self,
                _: FigureId,
                _: MeasureConstraints,
                _: &LayoutSceneQuery<'_>,
            ) -> Result<FigureMeasurement, LayoutError> {
                self.0.fetch_add(1, Ordering::Relaxed);
                Ok(FigureMeasurement::default())
            }
            fn minimum_size(
                &self,
                _: FigureId,
                _: MeasureConstraints,
                _: &LayoutSceneQuery<'_>,
            ) -> Result<Dimension, LayoutError> {
                Ok(Dimension::ZERO)
            }
            fn layout(
                &mut self,
                _: FigureId,
                _: &LayoutSceneQuery<'_>,
                _: &mut LayoutOutput,
            ) -> Result<(), LayoutError> {
                Ok(())
            }
        }
        #[derive(Eq, PartialEq)]
        struct Value;
        impl PreparationInputPayload for Value {
            fn retained_units(&self) -> u64 {
                0
            }
        }
        let (mut tree, root) = subject();
        let calls = Arc::new(AtomicUsize::new(0));
        tree.replace_layout_manager(root, Some(Box::new(Count(calls.clone()))));
        let scene = std::cell::OnceCell::new();
        let budget = crate::figure::PreparationBudgetHandle::default();
        let read = RuntimeLayoutRead {
            tree: &tree,
            scene: &scene,
            budget: &budget,
            failure: RefCell::new(None),
        };
        let query = LayoutSceneQuery::new(&read);
        for _ in 0..2 {
            query
                .preferred_measurement(root, MeasureConstraints::UNBOUNDED)
                .unwrap();
        }
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert!(scene.get().is_none());
        let key = PreparationInputKey::<Value>::new("cache.admission");
        let mut inputs = PreparationInputRegistry::new();
        inputs
            .register(
                key,
                |_: FigureId, _: &crate::figure::PreparationScene<'_>| Ok(Value),
            )
            .unwrap();
        query.input(&inputs, key, root).unwrap();
        query
            .preferred_measurement(root, MeasureConstraints::UNBOUNDED)
            .unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        // Admitted input sessions never overwrite the earlier input-free receipt.
        assert!(
            tree.input_free_measurements.borrow()[slotmap::DefaultKey::from(root.local())][0]
                .is_some()
        );
        tree.mark_validation_path_invalid(root);
        assert!(
            !tree
                .input_free_measurements
                .borrow()
                .contains_key(slotmap::DefaultKey::from(root.local()))
        );
    }

    #[test]
    #[ignore = "manual no-trace ordinary nested-measurement cost evidence"]
    fn ordinary_layout_input_query_cost() {
        struct Probe;
        impl LayoutManager for Probe {
            fn preferred_measurement(
                &self,
                _: FigureId,
                _: MeasureConstraints,
                _: &LayoutSceneQuery<'_>,
            ) -> Result<FigureMeasurement, LayoutError> {
                Ok(FigureMeasurement::default())
            }
            fn minimum_size(
                &self,
                _: FigureId,
                _: MeasureConstraints,
                _: &LayoutSceneQuery<'_>,
            ) -> Result<Dimension, LayoutError> {
                Ok(Dimension::ZERO)
            }
            fn layout(
                &mut self,
                owner: FigureId,
                query: &LayoutSceneQuery<'_>,
                _: &mut LayoutOutput,
            ) -> Result<(), LayoutError> {
                for (child, _) in query.children(owner)? {
                    std::hint::black_box(
                        query.preferred_measurement(child, MeasureConstraints::UNBOUNDED)?,
                    );
                    std::hint::black_box(query.minimum_size(child, MeasureConstraints::UNBOUNDED)?);
                }
                Ok(())
            }
        }
        for count in [1, 100] {
            let (mut tree, root) = subject();
            for _ in 0..count {
                let child = tree.add_child_to(
                    root,
                    Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0).unwrap()),
                );
                tree.add_child_to(
                    child,
                    Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0).unwrap()),
                );
                tree.replace_layout_manager(child, Some(Box::new(crate::layout::FillLayout)));
            }
            let mut text = crate::text::TextSystem::default();
            let budget = crate::figure::PreparationBudgetHandle::default();
            let mut session = LayoutMeasurementSession {
                text: text.parts().0,
                budget: &budget,
            };
            let mut probe = Probe;
            let mut samples = [Vec::new(), Vec::new()];
            for iteration in 0..220 {
                let order = if iteration % 2 == 0 { [0, 1] } else { [1, 0] };
                for side in order {
                    let mut output = LayoutOutput::new();
                    let start = std::time::Instant::now();
                    if side == 0 {
                        probe
                            .layout(root, &LayoutSceneQuery::new(&tree), &mut output)
                            .unwrap();
                    } else {
                        session
                            .layout(&tree, root, &mut probe, &mut output, None)
                            .unwrap();
                    }
                    if iteration >= 20 {
                        samples[side].push(start.elapsed().as_nanos());
                    }
                }
            }
            for (side, mut values) in samples.into_iter().enumerate() {
                values.sort_unstable();
                println!(
                    "ordinary children={count} side={side} n={} p50_ns={} p95_ns={} p99_ns={}",
                    values.len(),
                    values[values.len() / 2],
                    values[values.len() * 95 / 100],
                    values[values.len() * 99 / 100],
                );
            }
        }
    }

    #[test]
    fn constrained_measurement_source_ignores_cache_invalidation_but_observes_source_changes() {
        use crate::connection::{DependencySubject, FigureTreeRouteSceneRead, RouteSceneRead};
        let (mut tree, root) = subject();
        let geometries = std::collections::HashMap::new();
        let source = DependencySubject::FigureMeasurementSource(root);
        let committed = DependencySubject::FigureMeasurement(root);
        let before_source =
            FigureTreeRouteSceneRead::new(&tree, &geometries).dependency_generation(&source);
        let before_committed =
            FigureTreeRouteSceneRead::new(&tree, &geometries).dependency_generation(&committed);
        tree.blocks[root]
            .layout
            .invalidate(LayoutInvalidation::Geometry);
        let read = FigureTreeRouteSceneRead::new(&tree, &geometries);
        assert_eq!(read.dependency_generation(&source), before_source);
        assert_ne!(read.dependency_generation(&committed), before_committed);
        tree.set_preferred_size(root, Some(Dimension::new(20.0, 30.0)));
        assert_ne!(
            FigureTreeRouteSceneRead::new(&tree, &geometries).dependency_generation(&source),
            before_source
        );
        let before_style =
            FigureTreeRouteSceneRead::new(&tree, &geometries).dependency_generation(&source);
        tree.set_figure_style(
            root,
            crate::FigureStyle {
                cursor: Some(crate::CursorIcon::Text),
                ..crate::FigureStyle::default()
            },
        );
        let after_cursor =
            FigureTreeRouteSceneRead::new(&tree, &geometries).dependency_generation(&source);
        assert_ne!(after_cursor, before_style);
        tree.set_figure_style(
            root,
            crate::FigureStyle {
                cursor: Some(crate::CursorIcon::Text),
                tooltip: Some(Some("measurement input".into())),
                ..crate::FigureStyle::default()
            },
        );
        assert_ne!(
            FigureTreeRouteSceneRead::new(&tree, &geometries).dependency_generation(&source),
            after_cursor
        );
    }

    #[test]
    fn same_namespace_missing_identity_is_rejected_by_every_layout_query() {
        let (tree, root) = subject();
        let missing = FigureId::from_local(root.namespace(), slotmap::KeyData::from_ffi(u64::MAX));
        let error = LayoutError::Identity(TreeQueryError::UnknownFigure(missing));
        let query = LayoutSceneQuery::new(&tree);
        assert_eq!(query.children(missing), Err(error.clone()));
        assert_eq!(query.constraint(missing).err(), Some(error.clone()));
        assert_eq!(query.container_bounds(missing), Err(error.clone()));
        assert_eq!(
            query.preferred_measurement(missing, MeasureConstraints::UNBOUNDED),
            Err(error.clone())
        );
        assert_eq!(
            query.minimum_size(missing, MeasureConstraints::UNBOUNDED),
            Err(error.clone())
        );
        assert_eq!(query.maximum_size(missing), Err(error.clone()));
        assert_eq!(query.freeform_extent(missing), Err(error.clone()));
        assert_eq!(query.content_scale(missing), Err(error));
    }

    #[test]
    fn override_admission_distinguishes_bounded_and_unbounded_measurement() {
        let (mut tree, root) = subject();
        for axis in [f64::NAN, -1.0, f64::INFINITY, f64::NEG_INFINITY] {
            for size in [Dimension::new(axis, 0.0), Dimension::new(0.0, axis)] {
                tree.set_preferred_size(root, Some(size));
                tree.set_minimum_size(root, Some(size));
                let error = LayoutError::InvalidMeasurement {
                    figure: root,
                    reason: MeasurementFailure::InvalidSize,
                };
                assert_eq!(
                    tree.preferred_measurement(root, MeasureConstraints::UNBOUNDED),
                    Err(error.clone())
                );
                assert_eq!(
                    tree.minimum_size(root, MeasureConstraints::UNBOUNDED),
                    Err(error.clone())
                );
                tree.set_maximum_size(root, Some(size));
                if axis == f64::INFINITY {
                    assert_eq!(tree.maximum_size(root), Ok(size));
                } else {
                    assert_eq!(tree.maximum_size(root), Err(error));
                }
            }
        }
        tree.set_preferred_size(root, Some(Dimension::ZERO));
        tree.set_minimum_size(root, Some(Dimension::ZERO));
        tree.set_maximum_size(root, Some(Dimension::ZERO));
        assert_eq!(
            tree.preferred_measurement(root, MeasureConstraints::UNBOUNDED),
            Ok(FigureMeasurement::default())
        );
        assert_eq!(
            tree.minimum_size(root, MeasureConstraints::UNBOUNDED),
            Ok(Dimension::ZERO)
        );
        assert_eq!(tree.maximum_size(root), Ok(Dimension::ZERO));
    }

    #[test]
    fn layout_relationship_and_optional_absence_have_separate_results() {
        let (mut tree, root) = subject();
        let child = tree.add_child_to(
            root,
            Box::new(RectangleFigure::new(0.0, 0.0, 0.0, 0.0).expect("valid figure bounds")),
        );
        let query = LayoutSceneQuery::new(&tree);
        assert_eq!(query.children(child), Ok(Vec::new()));
        assert!(query.constraint(child).unwrap().is_none());
        assert_eq!(query.constraint_as::<Rectangle>(root, child), Ok(None));
        assert_eq!(
            query.constraint_as::<Rectangle>(child, root),
            Err(LayoutError::InvalidChild {
                container: child,
                child: root
            })
        );
        assert_eq!(query.freeform_extent(child), Ok(None));
        assert_eq!(query.content_scale(child), Ok(None));
    }

    #[test]
    fn layout_constraint_type_and_unvalidated_freeform_are_not_absence() {
        let (mut tree, root) = subject();
        let child = tree.add_child_to(
            root,
            Box::new(RectangleFigure::new(0.0, 0.0, 0.0, 0.0).expect("valid figure bounds")),
        );
        tree.set_boxed_constraint(child, Box::new(Rectangle::ZERO));
        let query = LayoutSceneQuery::new(&tree);
        assert_eq!(
            query.constraint_as::<Rectangle>(root, child),
            Ok(Some(&Rectangle::ZERO))
        );
        assert!(matches!(
            query.constraint_as::<crate::layout::XYConstraint>(root, child),
            Err(LayoutError::ConstraintTypeMismatch { container, child: subject, .. })
                if container == root && subject == child
        ));
        let mut freeform_tree = FigureTree::new();
        let freeform = freeform_tree.set_contents(Box::new(
            crate::container::FreeformLayerFigure::new(0.0, 0.0, 10.0, 10.0)
                .expect("valid detached figure input"),
        ));
        assert_eq!(
            LayoutSceneQuery::new(&freeform_tree).freeform_extent(freeform),
            Err(LayoutError::Freeform(FreeformError::Unvalidated(freeform)))
        );
        let mut other = FigureTree::new();
        let foreign = other.set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 0.0, 0.0).expect("valid figure bounds"),
        ));
        assert_eq!(
            query.constraint_as::<Rectangle>(foreign, child),
            Err(LayoutError::Identity(TreeQueryError::ForeignFigure(
                foreign
            )))
        );
    }

    #[test]
    fn inset_overflow_fails_before_constraint_reduction_and_retries_after_invalidation() {
        let (mut tree, root) = subject();
        tree.replace_layout_manager(root, Some(Box::new(crate::layout::StackLayout::new())));
        tree.blocks[root].state.insets = Insets::uniform(f64::MAX);
        let error = LayoutError::InvalidMeasurement {
            figure: root,
            reason: MeasurementFailure::DerivedOverflow,
        };
        assert_eq!(
            tree.preferred_measurement(root, MeasureConstraints::UNBOUNDED),
            Err(error.clone())
        );
        assert_eq!(
            tree.minimum_size(root, MeasureConstraints::UNBOUNDED),
            Err(error)
        );
        assert!(tree.blocks[root].layout.cache.borrow().preferred.is_none());
        assert!(tree.blocks[root].layout.cache.borrow().minimum.is_none());
        tree.blocks[root].state.insets = Insets::uniform(1.0);
        tree.mark_validation_path_invalid(root);
        assert_eq!(
            tree.preferred_measurement(root, MeasureConstraints::UNBOUNDED)
                .unwrap()
                .size(),
            Dimension::new(2.0, 2.0)
        );
        assert_eq!(
            tree.minimum_size(root, MeasureConstraints::UNBOUNDED),
            Ok(Dimension::new(2.0, 2.0))
        );
    }

    #[test]
    fn prepared_and_border_composition_checks_each_selected_stage() {
        use crate::figure::preparation::{PreparationKey, PreparedFigure};
        struct Drawing;
        impl crate::figure::FigureDrawing for Drawing {
            fn paint(
                &self,
                _: &mut crate::graphics::PaintContext<'_>,
            ) -> Result<(), crate::graphics::GraphicsError> {
                Ok(())
            }
        }
        let drawing = std::sync::Arc::new(Drawing);
        for measurement in [
            FigureMeasurement::new(f64::NAN, 1.0, None),
            FigureMeasurement::new(1.0, 1.0, Some(2.0)),
            FigureMeasurement::new(1.0, 1.0, Some(f64::INFINITY)),
        ] {
            assert!(matches!(
                crate::figure::FigurePresentation::new(
                    measurement,
                    Dimension::ZERO,
                    Rectangle::ZERO,
                    drawing.clone()
                ),
                Err(TextError::InvalidMetric)
            ));
        }
        let (mut tree, root) = subject();
        tree.blocks[root].prepared = Some(PreparedFigure {
            owner: root,
            valid: true,
            identity: crate::figure::PreparedSceneIdentity::new(root, 1),
            source: crate::figure::preparation::PreparationSource::default(),
            result_version: None,
            observations: Vec::new().into(),
            input_proofs: Vec::new().into(),
            required_routes: Vec::new(),
            key: PreparationKey {
                component: 0,
                provider: 0,
                text: 0,
                font: crate::text::FontDescriptor::default(),
                bounds: Rectangle::ZERO,
            },
            presentation: crate::figure::FigurePresentation::new(
                FigureMeasurement::new(4.0, 8.0, Some(6.0)),
                Dimension::new(2.0, 3.0),
                Rectangle::ZERO,
                drawing,
            )
            .unwrap(),
        });
        tree.blocks[root].border_snapshot = Some(BorderSnapshot::compound(
            None,
            None,
            Insets::uniform(1.0),
            Dimension::ZERO,
        ));
        assert_eq!(
            tree.preferred_measurement(root, MeasureConstraints::UNBOUNDED),
            Ok(FigureMeasurement::new(6.0, 10.0, Some(7.0)))
        );
        assert_eq!(
            tree.minimum_size(root, MeasureConstraints::UNBOUNDED),
            Ok(Dimension::new(4.0, 5.0))
        );
        // A finite negative top inset makes the composed baseline invalid.
        tree.blocks[root].border_snapshot = Some(BorderSnapshot::compound(
            None,
            None,
            Insets::new(-7.0, 0.0, 7.0, 0.0),
            Dimension::ZERO,
        ));
        assert_eq!(
            tree.preferred_measurement(root, MeasureConstraints::UNBOUNDED),
            Err(LayoutError::InvalidMeasurement {
                figure: root,
                reason: MeasurementFailure::InvalidBaseline
            })
        );
        tree.blocks[root].border_snapshot = Some(BorderSnapshot::compound(
            None,
            None,
            Insets::uniform(f64::MAX),
            Dimension::ZERO,
        ));
        let overflow = LayoutError::InvalidMeasurement {
            figure: root,
            reason: MeasurementFailure::DerivedOverflow,
        };
        assert_eq!(
            tree.preferred_measurement(root, MeasureConstraints::UNBOUNDED),
            Err(overflow.clone())
        );
        assert_eq!(
            tree.minimum_size(root, MeasureConstraints::UNBOUNDED),
            Err(overflow)
        );
    }

    #[test]
    fn failed_candidate_publishes_no_geometry_notifications_or_damage() {
        struct FailingLayout {
            survivor: FigureId,
            missing: FigureId,
        }
        impl LayoutManager for FailingLayout {
            fn preferred_measurement(
                &self,
                _: FigureId,
                _: MeasureConstraints,
                _: &LayoutSceneQuery<'_>,
            ) -> Result<FigureMeasurement, LayoutError> {
                Ok(FigureMeasurement::default())
            }
            fn minimum_size(
                &self,
                _: FigureId,
                _: MeasureConstraints,
                _: &LayoutSceneQuery<'_>,
            ) -> Result<Dimension, LayoutError> {
                Ok(Dimension::ZERO)
            }
            fn layout(
                &mut self,
                _: FigureId,
                query: &LayoutSceneQuery<'_>,
                output: &mut LayoutOutput,
            ) -> Result<(), LayoutError> {
                output.set_child_bounds(self.survivor, Rectangle::new(100.0, 200.0, 30.0, 40.0));
                output.repaint(self.survivor);
                query.container_bounds(self.missing)?;
                Ok(())
            }
        }
        let (mut tree, root) = subject();
        let survivor = tree.add_child_to(
            root,
            Box::new(RectangleFigure::new(1.0, 2.0, 3.0, 4.0).expect("valid figure bounds")),
        );
        let missing = FigureId::from_local(root.namespace(), slotmap::KeyData::from_ffi(u64::MAX));
        tree.replace_layout_manager(root, Some(Box::new(FailingLayout { survivor, missing })));
        tree.drain_notification_effects();
        let before = tree.figure_bounds(survivor);
        let mut updates = UpdateManager::with_namespace(tree.namespace());
        updates.add_invalid_figure(root);
        assert_eq!(
            tree.perform_validation_cycle(&mut updates),
            Err(ValidationError::Layout(LayoutError::Identity(
                TreeQueryError::UnknownFigure(missing)
            )))
        );
        assert_eq!(tree.figure_bounds(survivor), before);
        assert!(updates.dirty_regions.is_empty());
        assert!(tree.notification_effects().iter().all(|effect| matches!(
            effect,
            NotificationEffect::EmitLayout(LayoutEvent {
                kind: LayoutEventKind::Started,
                ..
            })
        )));
    }
}
