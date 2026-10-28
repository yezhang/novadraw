//! Supplemental text consumers and GPU checks of the actual official layout factories.
use super::{gpu_test_support::GpuCanvas, runtime_with_fonts};
use novadraw::{
    figure::{
        FigureStyle, LabelFigure, MeasureConstraints, RectangleFigure,
        flow::{BlockFlowFigure, FlowPageFigure, FlowSnapshot, StructuredTextFigure},
    },
    geometry::Rectangle,
    graphics::Color,
    layout::{FlowLayout, GridAlignment, GridConstraint, GridLayout, LayoutManager, ToolbarLayout},
    runtime::Runtime,
    tree::FigureId,
};

const TEXT: &str = "alpha beta gamma delta epsilon zeta eta theta";

fn text_fixture(layout: Box<dyn LayoutManager>, grid: bool) -> (Runtime, FigureId, FigureId) {
    let mut runtime = runtime_with_fonts();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            360.0,
            500.0,
            Color::WHITE,
        )))
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .set_style(FigureStyle {
            font: Some("16px Inter".into()),
            ..FigureStyle::default()
        })
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .set_layout_manager(layout)
        .unwrap();
    let label = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(LabelFigure::new("Label minimum")))
        .unwrap();
    let page = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(FlowPageFigure::new(Rectangle::ZERO)))
        .unwrap();
    let block = runtime
        .figure(page)
        .unwrap()
        .add(Box::new(BlockFlowFigure::default()))
        .unwrap();
    runtime
        .figure(block)
        .unwrap()
        .add(Box::new(StructuredTextFigure::new(TEXT)))
        .unwrap();
    if grid {
        for child in [label, page] {
            runtime
                .figure(child)
                .unwrap()
                .set_layout_constraint(GridConstraint {
                    horizontal_alignment: GridAlignment::Fill,
                    grab_horizontal: true,
                    ..GridConstraint::default()
                })
                .unwrap();
        }
    }
    (runtime, label, page)
}

#[test]
fn layout_text_minimum_wrap_and_width_restore_use_real_prepared_consumers() {
    for (name, layout, grid) in [
        (
            "Flow",
            Box::new(FlowLayout::new()) as Box<dyn LayoutManager>,
            false,
        ),
        ("Toolbar", Box::new(ToolbarLayout::new()), false),
        ("Grid", Box::new(GridLayout::new(1)), true),
    ] {
        let (mut runtime, label, page) = text_fixture(layout, grid);
        let mut wide = None;
        let mut narrow_height = 0.0;
        for width in [360.0, 100.0, 40.0, 0.0, 100.0, 360.0] {
            runtime
                .frames()
                .resize_logical_viewport(width, 500.0)
                .unwrap();
            runtime.frames().stabilize_for_query().unwrap();
            let label_bounds = runtime.tree().figure_bounds(label).unwrap();
            let bounds = runtime.tree().figure_bounds(page).unwrap();
            assert!(
                label_bounds.width >= 0.0 && label_bounds.height > 0.0,
                "{name}"
            );
            assert!(bounds.width >= 0.0 && bounds.height.is_finite(), "{name}");
            let minimum = runtime
                .tree()
                .minimum_size(label, MeasureConstraints::UNBOUNDED)
                .unwrap();
            assert!(minimum.width > 0.0 && minimum.height > 0.0, "{name}");
            let snapshot = runtime
                .tree()
                .prepared_presentation(page)
                .unwrap()
                .unwrap()
                .data::<FlowSnapshot>()
                .unwrap();
            assert_eq!(snapshot.paragraphs[0].layout.request().text(), TEXT);
            assert!(snapshot.size.height > 0.0);
            if width == 100.0 {
                narrow_height = bounds.height;
                if grid {
                    let root = runtime.tree().contents().unwrap();
                    let measured = runtime
                        .tree()
                        .preferred_measurement(root, MeasureConstraints::width(width).unwrap())
                        .unwrap();
                    assert!(
                        measured.height >= bounds.y + bounds.height,
                        "Grid constrained height {} must include rewrapped child {bounds:?}",
                        measured.height,
                    );
                }
            }
            if width == 360.0 {
                let state = (label_bounds, bounds, snapshot.size);
                if let Some(initial) = wide {
                    assert_eq!(state, initial, "{name}: width roundtrip");
                    assert!(narrow_height > bounds.height, "{name}: text must wrap");
                } else {
                    wide = Some(state);
                }
            }
        }
        assert!(runtime.take_deferred_mutation_errors().is_empty());
    }
}

#[test]
#[ignore = "requires native GPU; compares actual official layout resize and child erasure"]
fn official_layout_factories_resize_and_erase_without_stale_gpu_pixels() {
    for scale in [1.0, 2.0] {
        for (name, factory) in [
            (
                "Flow",
                super::flow_layout_example::panel_scene
                    as fn() -> novadraw_example_support::panel::PanelScene,
            ),
            ("Grid", super::grid_layout_example::panel_scene),
            ("Toolbar", super::toolbar_layout_example::panel_scene),
        ] {
            let (mut runtime, _) = factory();
            let mut canvas = GpuCanvas::new((832.0 * scale) as u32, (640.0 * scale) as u32, scale);
            canvas.render(&mut runtime, true);
            for width in [820.0, 760.0, 620.0, 440.0, 300.0, 620.0, 820.0] {
                runtime
                    .frames()
                    .resize_logical_viewport(width, 520.0)
                    .unwrap();
                let partial = canvas.render(&mut runtime, false);
                let full = canvas.render(&mut runtime, true);
                assert_eq!(partial, full, "{name}: resize width={width}, dpi={scale}");
            }
            let root = runtime.tree().contents().unwrap();
            let content = runtime.tree().child_order(root).unwrap()[0];
            let child = runtime.tree().child_order(content).unwrap()[0];
            for visible in [false, true] {
                runtime.figure(child).unwrap().set_visible(visible).unwrap();
                let partial = canvas.render(&mut runtime, false);
                assert_eq!(canvas.last_damage_mode, novadraw::render::DamageMode::Full);
                let full = canvas.render(&mut runtime, true);
                assert_eq!(partial, full, "{name}: visible={visible}, dpi={scale}");
            }
        }
    }
}

#[test]
fn grid_span_indent_and_explicit_height_hint_keep_their_measurement_contract() {
    let (mut runtime, _, page) = text_fixture(
        Box::new(GridLayout::new(2).with_equal_column_widths(true)),
        true,
    );
    let root = runtime.tree().contents().unwrap();
    for width in [181.0, 101.0, 181.0] {
        for height_hint in [None, Some(28.0)] {
            runtime
                .figure(page)
                .unwrap()
                .set_layout_constraint(GridConstraint {
                    horizontal_span: 2,
                    horizontal_indent: 7.0,
                    horizontal_alignment: GridAlignment::Fill,
                    grab_horizontal: true,
                    height_hint,
                    ..GridConstraint::default()
                })
                .unwrap();
            runtime
                .frames()
                .resize_logical_viewport(width, 500.0)
                .unwrap();
            runtime.frames().stabilize_for_query().unwrap();
            let bounds = runtime.tree().figure_bounds(page).unwrap();
            let snapshot = runtime
                .tree()
                .prepared_presentation(page)
                .unwrap()
                .unwrap()
                .data::<FlowSnapshot>()
                .unwrap();
            assert_eq!(bounds.x, 12.0);
            assert_eq!(bounds.width, width - 17.0);
            if let Some(height) = height_hint {
                assert_eq!(bounds.height, height);
            } else {
                assert_eq!(bounds.height, snapshot.size.height);
                let measured = runtime
                    .tree()
                    .preferred_measurement(root, MeasureConstraints::width(width).unwrap())
                    .unwrap();
                assert!(measured.height >= bounds.y + bounds.height);
            }
        }
    }
}
