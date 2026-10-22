use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    rc::Rc,
};

use crate::figure::{CapabilityKey, ResolvedStyle};
use crate::graph::FigureTree;
use crate::layout::{LayoutError, LayoutOutput};
use crate::text::TextError;
use crate::{Figure, FigureId, FigureMeasurement, MeasureConstraints, Rectangle};

use super::{
    FigurePresentation, PreparationDependencyError, PreparationMeasureError,
    PreparationMeasureErrorKind, PreparationMeasurement,
};

/// Frozen read access restricted to the preparing Figure's subtree.
pub struct PreparationScene<'a> {
    tree: &'a FigureTree,
    root: FigureId,
    styles: &'a HashMap<FigureId, ResolvedStyle>,
    members: HashSet<FigureId>,
    measurement_path: Rc<RefCell<Vec<FigureId>>>,
    measurement_constraints: Option<(MeasureConstraints, MeasureConstraints)>,
}

impl<'a> PreparationScene<'a> {
    pub(crate) fn new(
        tree: &'a FigureTree,
        root: FigureId,
        styles: &'a HashMap<FigureId, ResolvedStyle>,
    ) -> Self {
        let mut members = HashSet::new();
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            members.insert(id);
            if let Ok(node) = tree.node(id) {
                pending.extend(node.children.iter().copied());
            }
        }
        Self {
            tree,
            root,
            styles,
            members,
            measurement_path: Rc::new(RefCell::new(Vec::new())),
            measurement_constraints: None,
        }
    }

    pub fn root(&self) -> FigureId {
        self.root
    }

    /// Outer and content constraints of the current measurement callback.
    pub fn measurement_constraints(&self) -> Option<(MeasureConstraints, MeasureConstraints)> {
        self.measurement_constraints
    }

    pub(crate) fn for_measurement(
        &self,
        root: FigureId,
        outer: MeasureConstraints,
        content: MeasureConstraints,
    ) -> PreparationScene<'a> {
        let mut scene = Self::new(self.tree, root, self.styles);
        scene.measurement_path = self.measurement_path.clone();
        scene.measurement_constraints = Some((outer, content));
        scene
    }

    /// Measures the selected Figure through the authoritative Runtime measurement pipeline.
    /// A registered preparation capability is explicitly selected when no override/layout wins.
    pub fn measure_figure(
        &self,
        context: &mut crate::text::MeasureContext<'_>,
        id: FigureId,
        constraints: MeasureConstraints,
    ) -> Result<PreparationMeasurement, PreparationMeasureError> {
        let failure = |kind| PreparationMeasureError {
            figure: id,
            outer_constraints: constraints,
            content_constraints: None,
            kind,
        };
        self.tree.validate_identity(id).map_err(|error| {
            failure(PreparationMeasureErrorKind::Layout(Box::new(
                LayoutError::Identity(error),
            )))
        })?;
        if !self.members.contains(&id) {
            return Err(failure(PreparationMeasureErrorKind::Layout(Box::new(
                LayoutError::InvalidChild {
                    container: self.root,
                    child: id,
                },
            ))));
        }
        {
            let path = self.measurement_path.borrow();
            if path.contains(&id) {
                let mut path = path.clone();
                path.push(id);
                return Err(failure(PreparationMeasureErrorKind::Dependency(Box::new(
                    PreparationDependencyError::MeasurementCycle { path },
                ))));
            }
            if path.len() >= crate::graph::MAX_TREE_DEPTH {
                return Err(failure(PreparationMeasureErrorKind::Dependency(Box::new(
                    PreparationDependencyError::DepthExceeded {
                        limit: crate::graph::MAX_TREE_DEPTH,
                    },
                ))));
            }
        }
        struct MeasurementGuard(Rc<RefCell<Vec<FigureId>>>);
        impl Drop for MeasurementGuard {
            fn drop(&mut self) {
                self.0.borrow_mut().pop();
            }
        }
        self.measurement_path.borrow_mut().push(id);
        let _guard = MeasurementGuard(self.measurement_path.clone());
        self.tree
            .measure_preparation_figure(context, id, constraints, self)
    }

    fn check(&self, id: FigureId) -> Result<(), TextError> {
        if self.members.contains(&id) {
            return Ok(());
        }
        Err(TextError::InvalidPreparedScene(
            "source is outside preparation subtree".into(),
        ))
    }

    pub fn children(&self, id: FigureId) -> Result<Vec<FigureId>, TextError> {
        self.check(id)?;
        self.tree.child_order(id).map_err(scene_error)
    }

    pub fn parent(&self, id: FigureId) -> Result<Option<FigureId>, TextError> {
        self.check(id)?;
        if id == self.root {
            return Ok(None);
        }
        self.tree.parent_id(id).map_err(scene_error)
    }

    pub fn figure(&self, id: FigureId) -> Result<&dyn Figure, TextError> {
        self.check(id)?;
        Ok(self.tree.node(id).map_err(scene_error)?.figure.as_ref())
    }

    pub fn capability<C: 'static>(
        &self,
        id: FigureId,
        key: CapabilityKey<C>,
    ) -> Result<Option<&C>, TextError> {
        self.check(id)?;
        self.tree.capability(id, key).map_err(scene_error)
    }

    pub fn style(&self, id: FigureId) -> Result<&ResolvedStyle, TextError> {
        self.check(id)?;
        self.styles
            .get(&id)
            .ok_or_else(|| TextError::InvalidPreparedScene("missing resolved style".into()))
    }

    pub fn client_area(&self, id: FigureId) -> Result<Rectangle, TextError> {
        self.check(id)?;
        Ok(self.tree.node(id).map_err(scene_error)?.client_area())
    }

    /// Resolved ordinary Border metrics, including owner-dependent snapshots.
    pub fn insets(&self, id: FigureId) -> Result<crate::Insets, TextError> {
        self.check(id)?;
        Ok(self.tree.node(id).map_err(scene_error)?.insets())
    }

    pub fn preferred_measurement(
        &self,
        id: FigureId,
        constraints: MeasureConstraints,
    ) -> Result<FigureMeasurement, TextError> {
        self.check(id)?;
        self.tree
            .preferred_measurement(id, constraints)
            .map_err(scene_error)
    }

    /// Input revision includes structure, component state, styles and measurement generation.
    pub fn revision(&self) -> Result<u64, TextError> {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        let mut stack = vec![self.root];
        while let Some(id) = stack.pop() {
            id.hash(&mut hash);
            let node = self.tree.node(id).map_err(scene_error)?;
            node.component_revision.hash(&mut hash);
            if let Some(preparation) = node.preparation() {
                preparation.revision().hash(&mut hash);
            }
            node.layout.generation().hash(&mut hash);
            for value in [
                node.bounds.x,
                node.bounds.y,
                node.bounds.width,
                node.bounds.height,
                node.insets().top,
                node.insets().right,
                node.insets().bottom,
                node.insets().left,
            ] {
                value.to_bits().hash(&mut hash);
            }
            let style = self.style(id)?;
            style.font.hash(&mut hash);
            for value in style
                .foreground
                .to_rgba_array()
                .into_iter()
                .chain(style.background.to_rgba_array())
            {
                value.to_bits().hash(&mut hash);
            }
            style.alpha.to_bits().hash(&mut hash);
            for child in node.children.iter().rev() {
                stack.push(*child);
            }
        }
        Ok(hash.finish())
    }
}

pub(crate) fn scene_error(error: impl std::fmt::Display) -> TextError {
    TextError::InvalidPreparedScene(error.to_string())
}

/// Candidate drawing plus ordinary, parent-owned layout effects.
/// Runtime validates the complete batch before publishing any part.
pub struct PreparedScene {
    pub(crate) root: FigurePresentation,
    pub(crate) layouts: Vec<(FigureId, LayoutOutput)>,
    pub(crate) descendants: Vec<(FigureId, FigurePresentation)>,
}

impl PreparedScene {
    pub fn new(root: FigurePresentation) -> Self {
        Self {
            root,
            layouts: Vec::new(),
            descendants: Vec::new(),
        }
    }

    pub fn root_presentation(&self) -> &FigurePresentation {
        &self.root
    }

    /// Composes root drawing while retaining the candidate's descendant and layout effects.
    pub fn with_root_presentation(mut self, root: FigurePresentation) -> Self {
        self.root = root;
        self
    }

    pub fn layout(&mut self, parent: FigureId, output: LayoutOutput) {
        self.layouts.push((parent, output));
    }

    pub fn present(&mut self, source: FigureId, presentation: FigurePresentation) {
        self.descendants.push((source, presentation));
    }

    pub(crate) fn validate(&self, scene: &PreparationScene<'_>) -> Result<(), TextError> {
        let mut sources = std::collections::HashSet::new();
        for (id, _) in &self.descendants {
            scene.check(*id)?;
            if *id == scene.root || !sources.insert(*id) {
                return Err(TextError::InvalidPreparedScene(
                    "duplicate presentation source".into(),
                ));
            }
        }
        for (parent, output) in &self.layouts {
            scene.check(*parent)?;
            scene
                .tree
                .validate_layout_output(*parent, output)
                .map_err(|error: LayoutError| scene_error(error))?;
        }
        Ok(())
    }
}
