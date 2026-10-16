use std::sync::{Arc, Mutex};

use novadraw::figure::{
    CONTAINER, ContainerCapability, Figure, FigureCapabilityBuilder,
    FigureCapabilityRegistrationError, FigureContainer, FigureMeasurement, FigureStyle,
    LabelFigure, MeasureConstraints, RectangleFigure,
};
use novadraw::geometry::{Dimension, Insets, Point, Rectangle};
use novadraw::layout::{LayoutError, LayoutManager, LayoutOutput, LayoutSceneQuery, ToolbarLayout};
use novadraw::render::{
    BackendCapabilities, BuiltinFont, DamageMode, RenderCommandKind, RenderDisposition, SurfaceInfo,
};
use novadraw::runtime::{FramePreparation, Runtime, SubmissionAcknowledgement};
use novadraw::tree::{FigureId, FigureTree};

const INSET: f64 = 10.0;
const SCALE: f64 = 2.0;
const CONTENT_WIDTH: f64 = 30.0;
const CONTENT_HEIGHT: f64 = 10.0;
const CONTENT_BASELINE: f64 = 4.0;

#[derive(Clone)]
struct ProjectingInsetContainer {
    bounds: Rectangle,
}

impl ProjectingInsetContainer {
    fn project_size(size: Dimension) -> Dimension {
        let inset_width = INSET * 2.0;
        let inset_height = INSET * 2.0;
        Dimension::new(
            (size.width - inset_width).max(0.0) * SCALE + inset_width,
            (size.height - inset_height).max(0.0) * SCALE + inset_height,
        )
    }
}

impl Figure for ProjectingInsetContainer {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ExternalProjectingInsetContainer"
    }

    fn initial_insets(&self) -> Insets {
        Insets::uniform(INSET)
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for ProjectingInsetContainer {
    fn layout_constraints(&self, constraints: MeasureConstraints) -> MeasureConstraints {
        MeasureConstraints::new(
            constraints.max_width().map(|width| width / SCALE),
            constraints.max_height().map(|height| height / SCALE),
        )
        .expect("positive scale preserves valid constraints")
    }

    fn project_preferred_measurement(&self, measurement: FigureMeasurement) -> FigureMeasurement {
        let size = Self::project_size(measurement.size());
        FigureMeasurement::new(
            size.width,
            size.height,
            measurement
                .baseline
                .map(|baseline| (baseline - INSET).max(0.0) * SCALE + INSET),
        )
    }

    fn project_minimum_size(&self, size: Dimension) -> Dimension {
        Self::project_size(size)
    }
}

struct RecordingContentLayout {
    constraints: Arc<Mutex<Vec<MeasureConstraints>>>,
}

impl LayoutManager for RecordingContentLayout {
    fn preferred_measurement(
        &self,
        _container: FigureId,
        constraints: MeasureConstraints,
        _snapshot: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        self.constraints.lock().unwrap().push(constraints);
        Ok(FigureMeasurement::new(
            CONTENT_WIDTH,
            CONTENT_HEIGHT,
            Some(CONTENT_BASELINE),
        ))
    }

    fn minimum_size(
        &self,
        _container: FigureId,
        constraints: MeasureConstraints,
        _snapshot: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        self.constraints.lock().unwrap().push(constraints);
        Ok(Dimension::new(CONTENT_WIDTH, CONTENT_HEIGHT))
    }

    fn layout(
        &mut self,
        _container: FigureId,
        _snapshot: &LayoutSceneQuery<'_>,
        _out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        Ok(())
    }
}

#[test]
fn layout_measurement_orders_outer_constraints_insets_and_container_projection() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(ProjectingInsetContainer {
            bounds: Rectangle::new(0.0, 0.0, 100.0, 60.0),
        }))
        .expect("valid projecting container");
    tree.builder()
        .set_layout_manager(
            root,
            Box::new(RecordingContentLayout {
                constraints: Arc::clone(&observed),
            }),
        )
        .expect("valid external layout");

    let parent_constraints = MeasureConstraints::bounded(100.0, 60.0).unwrap();
    let preferred = tree
        .preferred_measurement(root, parent_constraints)
        .expect("preferred measurement");
    let minimum = tree
        .minimum_size(root, parent_constraints)
        .expect("minimum measurement");

    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            MeasureConstraints::bounded(40.0, 20.0).unwrap(),
            MeasureConstraints::bounded(40.0, 20.0).unwrap(),
        ]
    );
    assert_eq!(preferred, FigureMeasurement::new(80.0, 40.0, Some(18.0)));
    assert_eq!(minimum, Dimension::new(80.0, 40.0));
}

#[derive(Clone)]
struct InsetContainer {
    bounds: Rectangle,
    insets: Insets,
}

impl Figure for InsetContainer {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ExternalInsetContainer"
    }

    fn initial_insets(&self) -> Insets {
        self.insets
    }

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for InsetContainer {}

fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 240.0,
        logical_height: 160.0,
        pixel_width: 240,
        pixel_height: 160,
        scale_factor: 1.0,
    }
}

fn surface_bounds(runtime: &Runtime, figure: FigureId) -> Rectangle {
    let bounds = runtime.tree().figure_bounds(figure).unwrap();
    let transform = runtime.tree().local_to_surface_transform(figure).unwrap();
    let top_left = transform.transform_point(Point::ORIGIN);
    let bottom_right = transform.transform_point(Point::new(bounds.width, bounds.height));
    Rectangle::new(
        top_left.x(),
        top_left.y(),
        bottom_right.x() - top_left.x(),
        bottom_right.y() - top_left.y(),
    )
}

fn contains_rectangle(outer: Rectangle, inner: Rectangle) -> bool {
    outer.contains(inner.top_left()) && outer.contains(inner.bottom_right())
}

#[test]
fn nested_toolbar_labels_use_final_outer_geometry_in_the_first_submission() {
    const PADDING: f64 = 8.0;

    let mut runtime = Runtime::empty();
    runtime
        .resources()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 240.0, 160.0)))
        .expect("valid root");
    runtime
        .figure(root)
        .unwrap()
        .set_style(FigureStyle {
            font: Some("13px Inter Variable".to_owned()),
            ..FigureStyle::default()
        })
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .set_layout_manager(Box::new(ToolbarLayout::new().with_stretch_minor_axis(true)))
        .unwrap();

    let compartment = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(InsetContainer {
            bounds: Rectangle::ZERO,
            insets: Insets::uniform(PADDING),
        }))
        .unwrap();
    runtime
        .figure(compartment)
        .unwrap()
        .set_layout_manager(Box::new(ToolbarLayout::new().with_stretch_minor_axis(true)))
        .unwrap();
    let first = runtime
        .figure(compartment)
        .unwrap()
        .add(Box::new(
            LabelFigure::new("first").with_bounds(Rectangle::new(0.0, 0.0, 80.0, 20.0)),
        ))
        .unwrap();
    let second = runtime
        .figure(compartment)
        .unwrap()
        .add(Box::new(
            LabelFigure::new("second").with_bounds(Rectangle::new(0.0, 0.0, 80.0, 20.0)),
        ))
        .unwrap();

    let FramePreparation::Ready(submission) = runtime
        .frames()
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
    else {
        panic!("first frame must stabilize and produce a submission");
    };

    let first_preferred = runtime
        .tree()
        .preferred_measurement(first, MeasureConstraints::UNBOUNDED)
        .unwrap();
    let second_preferred = runtime
        .tree()
        .preferred_measurement(second, MeasureConstraints::UNBOUNDED)
        .unwrap();
    let first_minimum = runtime
        .tree()
        .minimum_size(first, MeasureConstraints::UNBOUNDED)
        .unwrap();
    let second_minimum = runtime
        .tree()
        .minimum_size(second, MeasureConstraints::UNBOUNDED)
        .unwrap();
    let expected_preferred = Dimension::new(
        first_preferred.width.max(second_preferred.width) + PADDING * 2.0,
        first_preferred.height + second_preferred.height + PADDING * 2.0,
    );
    let expected_minimum = Dimension::new(
        first_minimum.width.max(second_minimum.width) + PADDING * 2.0,
        first_minimum.height + second_minimum.height + PADDING * 2.0,
    );
    assert_eq!(
        runtime
            .tree()
            .preferred_measurement(compartment, MeasureConstraints::UNBOUNDED)
            .unwrap()
            .size(),
        expected_preferred
    );
    assert_eq!(
        runtime
            .tree()
            .minimum_size(compartment, MeasureConstraints::UNBOUNDED)
            .unwrap(),
        expected_minimum
    );

    let compartment_bounds = runtime.tree().figure_bounds(compartment).unwrap();
    let first_bounds = runtime.tree().figure_bounds(first).unwrap();
    let second_bounds = runtime.tree().figure_bounds(second).unwrap();
    assert_eq!(compartment_bounds.height, expected_preferred.height);
    assert_eq!(first_bounds.y, 0.0);
    assert_eq!(second_bounds.y, first_bounds.height);
    assert_eq!(
        first_bounds.height + second_bounds.height,
        compartment_bounds.height - PADDING * 2.0
    );

    let expected_client_clip = Rectangle::new(
        PADDING,
        PADDING,
        compartment_bounds.width - PADDING * 2.0,
        compartment_bounds.height - PADDING * 2.0,
    );
    assert!(submission.commands.iter().any(
        |command| matches!(command.kind, RenderCommandKind::Clip { rect } if rect == expected_client_clip)
    ));
    assert_eq!(
        submission
            .commands
            .iter()
            .filter(|command| matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }))
            .count(),
        2
    );
    assert_eq!(submission.damage.mode(), DamageMode::Full);

    let old_first_surface = surface_bounds(&runtime, first);
    let old_second_surface = surface_bounds(&runtime, second);
    assert_eq!(
        runtime.frames().complete_submission(
            submission.session_id,
            submission.frame_id,
            Ok(RenderDisposition::Presented),
        ),
        Ok(SubmissionAcknowledgement::Presented)
    );
    runtime
        .figure(first)
        .unwrap()
        .set_style(FigureStyle {
            font: Some("26px Inter Variable".to_owned()),
            ..FigureStyle::default()
        })
        .unwrap();

    let FramePreparation::Ready(updated) = runtime
        .frames()
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
    else {
        panic!("font mutation must stabilize and produce a submission");
    };
    let updated_compartment_bounds = runtime.tree().figure_bounds(compartment).unwrap();
    let updated_first_bounds = runtime.tree().figure_bounds(first).unwrap();
    let updated_second_bounds = runtime.tree().figure_bounds(second).unwrap();
    assert!(updated_compartment_bounds.height > compartment_bounds.height);
    assert!(updated_first_bounds.height > first_bounds.height);
    assert_eq!(updated_second_bounds.y, updated_first_bounds.height);
    assert_eq!(
        updated
            .commands
            .iter()
            .filter(|command| matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }))
            .count(),
        2
    );
    assert_eq!(updated.damage.mode(), DamageMode::Partial);
    let damage = updated.damage.union().expect("partial damage union");
    let new_first_surface = surface_bounds(&runtime, first);
    let new_second_surface = surface_bounds(&runtime, second);
    for bounds in [
        old_first_surface,
        old_second_surface,
        new_first_surface,
        new_second_surface,
    ] {
        assert!(
            contains_rectangle(damage, bounds),
            "damage {damage:?} must contain {bounds:?}"
        );
    }
}
