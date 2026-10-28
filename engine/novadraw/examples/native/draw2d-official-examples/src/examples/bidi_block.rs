use super::text_examples::{block, font, text};
use novadraw::{
    container::DefaultRangeModel,
    figure::{
        RectangleFigure,
        flow::{FlowBlockStyle, FlowPageFigure},
    },
    geometry::Rectangle,
    graphics::Color,
    layout::{GridConstraint, GridLayout},
    render::BuiltinFont,
    runtime::Runtime,
    text::{ParagraphAlignment, ParagraphDirection},
    tree::FigureTree,
};
use std::sync::Arc;

pub fn bidi_block_example() -> Runtime {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            600.0,
            500.0,
            Color::WHITE,
        )))
        .expect("two canvases");
    tree.builder()
        .set_layout_manager(
            root,
            Box::new(GridLayout::new(2).with_equal_column_widths(true)),
        )
        .expect("two columns");
    let vertical = Arc::new(DefaultRangeModel::default());
    let panes = [0, 1].map(|_| {
        tree.builder()
            .add_scroll_pane_with_models_to(
                root,
                Rectangle::new(0.0, 0.0, 300.0, 500.0),
                Arc::new(DefaultRangeModel::default()),
                vertical.clone(),
            )
            .expect("shared vertical range")
    });
    let mut runtime = Runtime::new(tree);
    for font in BuiltinFont::ALL {
        runtime
            .resources()
            .register_builtin_font(font)
            .expect("font");
    }
    for (index, pane) in panes.into_iter().enumerate() {
        runtime
            .figure(pane.pane_id())
            .expect("pane")
            .set_style(novadraw::figure::FigureStyle {
                layout_direction: Some(if index == 0 {
                    novadraw::figure::LayoutDirection::LeftToRight
                } else {
                    novadraw::figure::LayoutDirection::RightToLeft
                }),
                ..Default::default()
            })
            .expect("Canvas layout direction");
        runtime
            .figure(pane.pane_id())
            .expect("pane")
            .set_layout_constraint(GridConstraint::fill())
            .expect("fill column");
        runtime
            .figure(pane.pane_id())
            .expect("pane")
            .set_preferred_size(novadraw::geometry::Dimension::new(300.0, 500.0))
            .expect("column size");
        runtime
            .viewports()
            .set_tracks_width(pane.viewport().figure_id(), true)
            .expect("track width");
        let page = runtime
            .viewports()
            .set_scroll_contents(
                &pane,
                Box::new(FlowPageFigure::new(Rectangle::ZERO).with_style(font("13px Arial"))),
            )
            .expect("page");
        for direction in [
            ParagraphDirection::LeftToRight,
            ParagraphDirection::RightToLeft,
        ] {
            for alignment in [
                Some(ParagraphAlignment::Leading),
                Some(ParagraphAlignment::Trailing),
                Some(ParagraphAlignment::Left),
                Some(ParagraphAlignment::Right),
                None,
            ] {
                let paragraph = block(
                    &mut runtime,
                    page,
                    FlowBlockStyle {
                        direction,
                        alignment,
                        ..FlowBlockStyle::default()
                    },
                );
                text(&mut runtime, paragraph, "شصجح ذبغ and some english text.");
            }
        }
    }
    runtime
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires a native GPU; run through text.layout-direction"]
    fn bidi_direction_mutation_partial_matches_full_pixels() {
        use super::super::gpu_test_support::GpuCanvas;
        use novadraw::figure::{FigureStyle, LayoutDirection};
        let mut canvas = GpuCanvas::new(640, 512, 1.0);
        let mut runtime = bidi_block_example();
        let root = runtime.tree().contents().unwrap();
        let _ = canvas.render(&mut runtime, true);
        let pane = runtime.tree().child_order(root).unwrap()[1];
        for (index, (direction, width, height)) in [
            (LayoutDirection::LeftToRight, 420.0, 180.0),
            (LayoutDirection::RightToLeft, 420.0, 180.0),
            (LayoutDirection::RightToLeft, 600.0, 500.0),
        ]
        .into_iter()
        .enumerate()
        {
            runtime
                .figure(pane)
                .unwrap()
                .set_style(FigureStyle {
                    layout_direction: Some(direction),
                    ..Default::default()
                })
                .unwrap();
            runtime
                .figure(root)
                .unwrap()
                .set_bounds(Rectangle::new(0.0, 0.0, width, height))
                .unwrap();
            let partial = canvas.render(&mut runtime, false);
            let full = canvas.render(&mut runtime, true);
            assert_eq!(partial, full, "direction/resize state {index}");
            assert!(
                full.chunks_exact(4)
                    .any(|pixel| pixel[3] > 0 && pixel[0] < 100)
            );
            canvas.save(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../../target/verification/official-example-capability-increment/bidi/state-{index}.png")), &full);
        }
    }
    #[test]
    fn both_trees_keep_direction_and_share_scroll_and_resize() {
        let mut runtime = bidi_block_example();
        let root = runtime.tree().contents().unwrap();
        runtime
            .figure(root)
            .unwrap()
            .set_bounds(Rectangle::new(0.0, 0.0, 420.0, 80.0))
            .unwrap();
        runtime.frames().stabilize_for_query().unwrap();
        let panes = runtime.tree().child_order(root).unwrap();
        let viewports = panes
            .iter()
            .map(|pane| {
                runtime
                    .tree()
                    .child_order(*pane)
                    .unwrap()
                    .into_iter()
                    .find(|id| runtime.tree().node(*id).unwrap().figure_name() == "ViewportFigure")
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let mut pages = Vec::new();
        for (index, &viewport) in viewports.iter().enumerate() {
            let page = runtime.tree().child_order(viewport).unwrap()[0];
            pages.push(page);
            let snapshot = runtime
                .tree()
                .prepared_presentation(page)
                .unwrap()
                .unwrap()
                .data::<novadraw::figure::flow::FlowSnapshot>()
                .unwrap();
            assert_eq!(snapshot.paragraphs.len(), 10);
            assert_eq!(
                snapshot.paragraphs[0].layout.request().layout_direction(),
                if index == 0 {
                    novadraw::figure::LayoutDirection::LeftToRight
                } else {
                    novadraw::figure::LayoutDirection::RightToLeft
                },
            );
            assert_eq!(
                snapshot.paragraphs[0].layout.request().direction(),
                ParagraphDirection::LeftToRight
            );
            assert_eq!(
                snapshot.paragraphs[5].layout.request().direction(),
                ParagraphDirection::RightToLeft
            );
            assert_eq!(
                snapshot.paragraphs[7].layout.request().alignment(),
                ParagraphAlignment::Left
            );
            assert_eq!(
                snapshot.paragraphs[8].layout.request().alignment(),
                ParagraphAlignment::Right
            );
        }
        assert_ne!(pages[0], pages[1]);
        runtime
            .viewports()
            .set_vertical_location(viewports[0], 20.0)
            .unwrap();
        runtime.frames().stabilize_for_query().unwrap();
        for viewport in viewports {
            assert_eq!(
                runtime
                    .viewports()
                    .handle(viewport)
                    .unwrap()
                    .view_location()
                    .y(),
                20.0
            );
        }
    }
}
