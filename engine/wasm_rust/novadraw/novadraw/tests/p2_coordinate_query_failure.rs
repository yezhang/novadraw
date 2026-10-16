use novadraw::figure::{
    CONTAINER, ChildTransform, ContainerCapability, Figure, FigureCapabilityBuilder,
    FigureCapabilityRegistrationError, FigureContainer, RectangleFigure,
};
use novadraw::geometry::{Affine2D, Rectangle};
use novadraw::runtime::Runtime;
use novadraw::tree::{
    CoordinateFailure, CoordinateQueryError, FigureId, FigureTree, TreeQueryError,
};

struct MatrixContainer(Affine2D);

impl Figure for MatrixContainer {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }
    fn name(&self) -> &'static str {
        "ExternalMatrixContainer"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for MatrixContainer {
    fn child_transform(&self) -> ChildTransform {
        ChildTransform::from_affine(self.0)
    }
}

fn scene(transform: Affine2D) -> (FigureTree, FigureId, FigureId) {
    let mut tree = FigureTree::new();
    let parent = tree
        .builder()
        .set_contents(Box::new(MatrixContainer(transform)))
        .unwrap();
    let child = tree
        .builder()
        .add_child(
            parent,
            Box::new(RectangleFigure::from_bounds(Rectangle::ZERO)),
        )
        .unwrap();
    (tree, parent, child)
}

#[test]
fn external_container_singular_inverse_is_legal_absence() {
    let (tree, parent, child) = scene(Affine2D::from_scale(0.0, 1.0));
    assert!(tree.child_content_to_surface_transform(parent).is_ok());
    assert!(tree.local_to_surface_transform(child).is_ok());
    assert_eq!(tree.surface_to_local_transform(child), Ok(None));
    assert_eq!(
        tree.parent_to_local_transform(child),
        Ok(Some(Affine2D::IDENTITY))
    );
}

#[test]
fn external_container_inverse_distinguishes_range_and_input_failures() {
    for (transform, reason) in [
        (
            Affine2D::from_scale(1e-320, 1.0),
            CoordinateFailure::InverseUnrepresentable,
        ),
        (
            Affine2D::new(1e-200, 0.0, 0.0, 1.0, 1e200, 0.0),
            CoordinateFailure::InverseUnrepresentable,
        ),
        (
            Affine2D::new(0.0, 0.0, 0.0, 0.0, f64::NAN, 0.0),
            CoordinateFailure::NonFiniteInput,
        ),
    ] {
        let (tree, _, child) = scene(transform);
        assert_eq!(
            tree.surface_to_local_transform(child),
            Err(CoordinateQueryError::Geometry {
                figure: child,
                reason
            })
        );
    }
}

#[test]
fn external_container_representable_extreme_inverse_succeeds() {
    for (x, y) in [(1e200, 1e200), (1e-200, 1e-200), (1e200, 1e-200)] {
        let (tree, _, child) = scene(Affine2D::from_scale(x, y));
        let inverse = tree.surface_to_local_transform(child).unwrap().unwrap();
        assert!((inverse.coeffs()[0] * x - 1.0).abs() < 1e-14);
        assert!((inverse.coeffs()[3] * y - 1.0).abs() < 1e-14);
    }
}

#[test]
fn coordinate_queries_revalidate_foreign_and_retired_identity() {
    let (tree, parent, child) = scene(Affine2D::IDENTITY);
    let (_, foreign, _) = scene(Affine2D::IDENTITY);
    let mut runtime = Runtime::new(tree);
    runtime.figure(parent).unwrap().remove(child).unwrap();
    for (id, identity) in [
        (foreign, TreeQueryError::ForeignFigure(foreign)),
        (child, TreeQueryError::UnknownFigure(child)),
    ] {
        let expected = CoordinateQueryError::Identity(identity);
        let tree = runtime.tree();
        assert_eq!(tree.local_to_parent_transform(id), Err(expected));
        assert_eq!(tree.parent_to_local_transform(id), Err(expected));
        assert_eq!(tree.local_to_surface_transform(id), Err(expected));
        assert_eq!(tree.surface_to_local_transform(id), Err(expected));
        assert_eq!(tree.child_content_to_surface_transform(id), Err(expected));
    }
}

#[test]
fn finite_parent_composition_overflow_is_a_geometry_error() {
    let mut tree = FigureTree::new();
    let parent = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(f64::MAX, 0.0, 0.0, 0.0)))
        .unwrap();
    let child = tree
        .builder()
        .add_child(
            parent,
            Box::new(RectangleFigure::new(f64::MAX, 0.0, 0.0, 0.0)),
        )
        .unwrap();
    assert!(tree.local_to_parent_transform(child).is_ok());
    assert_eq!(
        tree.local_to_surface_transform(child),
        Err(CoordinateQueryError::Geometry {
            figure: child,
            reason: CoordinateFailure::DerivedOverflow,
        })
    );
}
