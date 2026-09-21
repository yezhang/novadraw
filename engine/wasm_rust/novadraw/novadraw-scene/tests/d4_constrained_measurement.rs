use novadraw_render::{
    BackendCapabilities, BuiltinFont, FontDescriptor, NdCanvas, RenderCommandKind, SurfaceInfo,
    TextConstraints, TextLayout,
};
use novadraw_scene::{
    Figure, FigureId, FigureMeasurement, LayoutError, LayoutManager, LayoutOutput, LayoutSnapshot,
    MeasureConstraints, Rectangle, RectangleFigure, Runtime, XYConstraint, XYLayout,
};

struct WrappedTextFigure {
    natural: TextLayout,
    constrained: TextLayout,
}

impl WrappedTextFigure {
    fn selected_layout(&self, constraints: MeasureConstraints) -> &TextLayout {
        if constraints.max_width()
            == self
                .constrained
                .key()
                .constraints()
                .max_width
                .map(f64::from)
        {
            &self.constrained
        } else {
            &self.natural
        }
    }
}

impl Figure for WrappedTextFigure {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }

    fn name(&self) -> &'static str {
        "WrappedTextFigure"
    }

    fn intrinsic_measurement(&self, constraints: MeasureConstraints) -> FigureMeasurement {
        let layout = self.selected_layout(constraints);
        FigureMeasurement::new(
            f64::from(layout.width()),
            f64::from(layout.height()),
            Some(f64::from(layout.baseline())),
        )
    }

    fn paint_figure_in_bounds(&self, canvas: &mut NdCanvas, bounds: Rectangle) {
        let layout = self.selected_layout(MeasureConstraints::from_hints(bounds.width, -1.0));
        canvas.fill_text_layout(layout, 0.0, 0.0);
    }
}

struct ConstrainedColumnLayout;

impl LayoutManager for ConstrainedColumnLayout {
    fn get_preferred_size(
        &self,
        container: FigureId,
        width_hint: f64,
        height_hint: f64,
        snapshot: &LayoutSnapshot<'_>,
    ) -> (f64, f64) {
        snapshot
            .children(container)
            .first()
            .map(|(child, _)| snapshot.measurement(*child, width_hint, height_hint).size())
            .unwrap_or_default()
    }

    fn get_minimum_size(
        &self,
        container: FigureId,
        width_hint: f64,
        height_hint: f64,
        snapshot: &LayoutSnapshot<'_>,
    ) -> (f64, f64) {
        self.get_preferred_size(container, width_hint, height_hint, snapshot)
    }

    fn layout(
        &mut self,
        container: FigureId,
        snapshot: &LayoutSnapshot<'_>,
        output: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        let area = snapshot.container_bounds(container);
        for (child, _) in snapshot.children(container) {
            let measured = snapshot.measurement(child, area.width, -1.0);
            output.set_child_bounds(
                child,
                Rectangle::new(area.x, area.y, area.width, measured.height),
            );
        }
        Ok(())
    }
}

fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 160.0,
        logical_height: 120.0,
        pixel_width: 160,
        pixel_height: 120,
        scale_factor: 1.0,
    }
}

#[test]
fn constrained_measurement_drives_arrange_and_reuses_the_same_glyph_ir() {
    const WIDTH: f32 = 72.0;

    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let font = FontDescriptor::default();
    let text = "alpha beta gamma delta";
    let natural = runtime
        .layout_text(text, &font, TextConstraints::UNBOUNDED)
        .unwrap();
    let constrained = runtime
        .layout_text(text, &font, TextConstraints::new(Some(WIDTH)).unwrap())
        .unwrap();
    assert!(constrained.line_metrics().len() > 1);
    assert!(constrained.height() > natural.height());

    let expected_height = f64::from(constrained.height());
    let expected_baseline = f64::from(constrained.baseline());
    let expected_runs = constrained.glyph_runs().to_vec();
    let root = runtime.set_contents(Box::new(RectangleFigure::new(
        0.0,
        0.0,
        f64::from(WIDTH),
        120.0,
    )));
    let text_figure = runtime.add_figure(
        root,
        Box::new(WrappedTextFigure {
            natural,
            constrained,
        }),
    );
    runtime
        .set_layout_manager(root, Box::new(ConstrainedColumnLayout))
        .unwrap();

    let submission = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .unwrap();
    let arranged = runtime.tree().figure_bounds(text_figure).unwrap();
    assert_eq!(arranged.width, f64::from(WIDTH));
    assert_eq!(arranged.height, expected_height);
    assert_eq!(
        runtime
            .tree()
            .measurement(text_figure, f64::from(WIDTH), -1.0)
            .unwrap()
            .baseline,
        Some(expected_baseline)
    );

    let recorded_runs: Vec<_> = submission
        .commands
        .iter()
        .filter_map(|command| match &command.kind {
            RenderCommandKind::DrawGlyphRun { run, .. } => Some(run.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(recorded_runs, expected_runs);
}

#[test]
fn xy_layout_passes_fixed_width_hint_when_height_is_automatic() {
    const WIDTH: f32 = 72.0;

    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let font = FontDescriptor::default();
    let text = "alpha beta gamma delta";
    let natural = runtime
        .layout_text(text, &font, TextConstraints::UNBOUNDED)
        .unwrap();
    let constrained = runtime
        .layout_text(text, &font, TextConstraints::new(Some(WIDTH)).unwrap())
        .unwrap();
    assert!(constrained.height() > natural.height());

    let expected_height = f64::from(constrained.height());
    let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 160.0, 120.0)));
    let text_figure = runtime.add_figure(
        root,
        Box::new(WrappedTextFigure {
            natural,
            constrained,
        }),
    );
    runtime
        .set_layout_constraint(
            text_figure,
            XYConstraint::at_size(10.0, 12.0, f64::from(WIDTH), -1.0),
        )
        .unwrap();
    runtime
        .set_layout_manager(root, Box::new(XYLayout::new()))
        .unwrap();

    runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .unwrap();

    assert_eq!(
        runtime.tree().figure_bounds(text_figure),
        Some(Rectangle::new(
            10.0,
            12.0,
            f64::from(WIDTH),
            expected_height,
        ))
    );
}
