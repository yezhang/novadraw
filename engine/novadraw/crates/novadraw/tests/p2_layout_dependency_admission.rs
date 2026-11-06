use novadraw::{
    figure::{FigureMeasurement, MeasureConstraints, PreparationBudget, RectangleFigure},
    geometry::Dimension,
    layout::{
        LayoutConstraint, LayoutDependencyDeclaration, LayoutDependencyRegistrationError,
        LayoutError, LayoutManager, LayoutOutput, LayoutSceneQuery,
    },
    runtime::{Runtime, RuntimeMutationError, update::PropertyKey},
    tree::{FigureId, FigureSubtree},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

const VALUE: PropertyKey<f64> = PropertyKey::new("external.container", "value");
const OTHER_VALUE_TYPE: PropertyKey<bool> = PropertyKey::new("external.container", "value");

enum Registration {
    All,
    Specific(FigureId),
    Overlap(FigureId),
    DuplicateIgnored,
    BudgetIgnored,
    Reserved(Arc<Mutex<Option<FigureId>>>),
    Panic,
    None,
}
struct ExternalLayout {
    mode: Registration,
    calls: Arc<AtomicUsize>,
}
impl LayoutManager for ExternalLayout {
    fn register_child_layout_dependencies(
        &self,
        out: &mut LayoutDependencyDeclaration,
    ) -> Result<(), LayoutDependencyRegistrationError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        match &self.mode {
            Registration::All => out.independent_child_property(VALUE),
            Registration::Specific(child) => out.independent_child_property_for(*child, VALUE),
            Registration::Overlap(child) => {
                out.independent_child_property(VALUE)?;
                out.independent_child_property_for(*child, VALUE)
            }
            Registration::DuplicateIgnored => {
                out.independent_child_property(VALUE)?;
                let _ = out.independent_child_property(VALUE);
                Ok(())
            }
            Registration::BudgetIgnored => {
                let _ = out.independent_child_property(VALUE);
                Ok(())
            }
            Registration::Reserved(child) => {
                out.independent_child_property_for(child.lock().unwrap().unwrap(), VALUE)
            }
            Registration::Panic => {
                out.independent_child_property(VALUE)?;
                panic!("external registration failure")
            }
            Registration::None => Ok(()),
        }
    }
    fn validate_constraint(
        &self,
        container: FigureId,
        child: FigureId,
        constraint: &dyn LayoutConstraint,
    ) -> Result<(), LayoutError> {
        if let Registration::Reserved(observed) = &self.mode {
            *observed.lock().unwrap() = Some(child);
            Ok(())
        } else {
            Err(LayoutError::UnsupportedConstraint {
                container,
                child,
                actual: constraint.type_name(),
            })
        }
    }
    fn preferred_measurement(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        Ok(FigureMeasurement::new(10.0, 10.0, None))
    }
    fn minimum_size(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        Ok(Dimension::new(10.0, 10.0))
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
fn layout(mode: Registration, calls: &Arc<AtomicUsize>) -> Box<dyn LayoutManager> {
    Box::new(ExternalLayout {
        mode,
        calls: calls.clone(),
    })
}
fn rectangle() -> Box<RectangleFigure> {
    Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap())
}
fn fixture() -> (Runtime, FigureId, FigureId) {
    let mut runtime = Runtime::empty();
    let owner = runtime.set_contents(rectangle()).unwrap();
    let child = runtime.figure(owner).unwrap().add(rectangle()).unwrap();
    (runtime, owner, child)
}
fn count(runtime: &Runtime, owner: FigureId) -> usize {
    runtime
        .tree()
        .node(owner)
        .unwrap()
        .layout_state()
        .child_dependency_count()
}

#[test]
fn extension_declarations_are_admitted_once_and_match_complete_typed_identity() {
    let (mut runtime, owner, child) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    runtime
        .figure(owner)
        .unwrap()
        .set_layout_manager(layout(Registration::All, &calls))
        .unwrap();
    for _ in 0..3 {
        let state = runtime.tree().node(owner).unwrap().layout_state();
        assert!(state.declares_child_property_independent(child, VALUE));
        assert!(!state.declares_child_property_independent(child, OTHER_VALUE_TYPE));
    }
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn ignored_duplicate_does_not_replace_the_old_manager_or_declarations() {
    let (mut runtime, owner, child) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    runtime
        .figure(owner)
        .unwrap()
        .set_layout_manager(layout(Registration::Specific(child), &calls))
        .unwrap();
    let error = runtime
        .figure(owner)
        .unwrap()
        .set_layout_manager(layout(Registration::DuplicateIgnored, &calls))
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeMutationError::Layout(LayoutError::DependencyRegistration(
            LayoutDependencyRegistrationError::Duplicate { .. }
        ))
    ));
    assert_eq!(count(&runtime, owner), 1);
    assert!(
        runtime
            .tree()
            .node(owner)
            .unwrap()
            .layout_state()
            .declares_child_property_independent(child, VALUE)
    );
}

#[test]
fn foreign_and_non_direct_children_are_rejected_without_installation() {
    let (mut runtime, owner, child) = fixture();
    let grandchild = runtime.figure(child).unwrap().add(rectangle()).unwrap();
    let (_, foreign, _) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    for invalid in [owner, grandchild, foreign] {
        let error = runtime
            .figure(owner)
            .unwrap()
            .set_layout_manager(layout(Registration::Specific(invalid), &calls))
            .unwrap_err();
        assert!(matches!(
            error,
            RuntimeMutationError::Layout(LayoutError::DependencyRegistration(
                LayoutDependencyRegistrationError::InvalidChild { .. }
            ))
        ));
        assert_eq!(count(&runtime, owner), 0);
        assert!(runtime.tree().layout_manager(owner).unwrap().is_none());
    }
}

#[test]
fn overlapping_all_and_specific_entries_are_normalized_and_replacement_retires_them() {
    let (mut runtime, owner, child) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    runtime
        .figure(owner)
        .unwrap()
        .set_layout_manager(layout(Registration::Overlap(child), &calls))
        .unwrap();
    assert_eq!(count(&runtime, owner), 1);
    runtime
        .figure(owner)
        .unwrap()
        .set_layout_manager(layout(Registration::None, &calls))
        .unwrap();
    assert_eq!(count(&runtime, owner), 0);
}

#[test]
fn budget_failure_keeps_the_previous_declaration_table() {
    let (mut runtime, owner, _) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    runtime
        .figure(owner)
        .unwrap()
        .set_layout_manager(layout(Registration::All, &calls))
        .unwrap();
    runtime
        .frames()
        .set_preparation_budget(PreparationBudget::new(100, 0));
    let error = runtime
        .figure(owner)
        .unwrap()
        .set_layout_manager(layout(Registration::All, &calls))
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeMutationError::Layout(LayoutError::DependencyRegistration(
            LayoutDependencyRegistrationError::Budget(_)
        ))
    ));
    assert_eq!(count(&runtime, owner), 1);
}

#[test]
fn an_ignored_budget_error_still_rejects_manager_admission() {
    let (mut runtime, owner, _) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    runtime
        .frames()
        .set_preparation_budget(PreparationBudget::new(100, 0));
    let error = runtime
        .figure(owner)
        .unwrap()
        .set_layout_manager(layout(Registration::BudgetIgnored, &calls))
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeMutationError::Layout(LayoutError::DependencyRegistration(
            LayoutDependencyRegistrationError::Budget(_)
        ))
    ));
    assert!(runtime.tree().layout_manager(owner).unwrap().is_none());
}

#[test]
fn a_reserved_direct_child_can_be_declared_before_subtree_publication() {
    let (mut runtime, owner, _) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::new(Mutex::new(None));
    let mut subtree = FigureSubtree::new(rectangle());
    let root = subtree.root();
    let child = subtree.add(root, rectangle()).unwrap();
    subtree.set_constraint(child, Box::new(())).unwrap();
    subtree
        .set_layout_manager(
            root,
            layout(Registration::Reserved(observed.clone()), &calls),
        )
        .unwrap();
    let receipt = runtime.figure(owner).unwrap().add_subtree(subtree).unwrap();
    let child = receipt.figure(child).unwrap();
    assert_eq!(*observed.lock().unwrap(), Some(child));
    assert!(
        runtime
            .tree()
            .node(receipt.root())
            .unwrap()
            .layout_state()
            .declares_child_property_independent(child, VALUE)
    );
}

#[test]
fn all_managers_in_a_subtree_share_the_same_admission_budget() {
    let (mut runtime, owner, _) = fixture();
    let before = runtime.tree().descendant_ids(owner).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut subtree = FigureSubtree::new(rectangle());
    let root = subtree.root();
    let child = subtree.add(root, rectangle()).unwrap();
    for node in [root, child] {
        subtree
            .set_layout_manager(node, layout(Registration::All, &calls))
            .unwrap();
    }
    // Pending identities: 2; each declaration: entry + overlap index + table metadata.
    // This admits either manager by itself but refuses the complete branch.
    runtime
        .frames()
        .set_preparation_budget(PreparationBudget::new(100, 5));
    assert!(runtime.figure(owner).unwrap().add_subtree(subtree).is_err());
    assert_eq!(runtime.tree().descendant_ids(owner).unwrap(), before);
}

#[test]
fn registration_panic_cannot_publish_a_partial_replacement() {
    let (mut runtime, owner, _) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    runtime
        .figure(owner)
        .unwrap()
        .set_layout_manager(layout(Registration::All, &calls))
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime
            .figure(owner)
            .unwrap()
            .set_layout_manager(layout(Registration::Panic, &calls))
    }));
    assert!(result.is_err());
    assert_eq!(count(&runtime, owner), 1);
}

#[test]
fn subtree_managers_are_admitted_once_before_publication() {
    let (mut runtime, owner, _) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut subtree = FigureSubtree::new(rectangle());
    let root = subtree.root();
    subtree.add(root, rectangle()).unwrap();
    subtree
        .set_layout_manager(root, layout(Registration::All, &calls))
        .unwrap();
    let receipt = runtime.figure(owner).unwrap().add_subtree(subtree).unwrap();
    assert_eq!(count(&runtime, receipt.root()), 1);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn rejected_subtree_declarations_leave_no_published_nodes() {
    let (mut runtime, owner, child) = fixture();
    let before = runtime.tree().descendant_ids(owner).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut subtree = FigureSubtree::new(rectangle());
    let root = subtree.root();
    subtree.add(root, rectangle()).unwrap();
    subtree
        .set_layout_manager(root, layout(Registration::Specific(child), &calls))
        .unwrap();
    assert!(runtime.figure(owner).unwrap().add_subtree(subtree).is_err());
    assert_eq!(runtime.tree().descendant_ids(owner).unwrap(), before);
}
