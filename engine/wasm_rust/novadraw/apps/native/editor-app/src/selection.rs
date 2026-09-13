use novadraw::{
    Color, FigureId, FigureTree, NdCanvas, RenderCommand, RenderSubmission,
    command::{LineCap, LineJoin},
};

const OUTLINE_COLOR: Color = Color {
    r: 0.98,
    g: 0.86,
    b: 0.22,
    a: 1.0,
};
const OUTLINE_INSET: f64 = 2.0;
const OUTLINE_STROKE_WIDTH: f64 = 4.0;

/// Editor-owned selection state and feedback composition.
#[derive(Debug, Default)]
pub struct SelectionModel {
    selected: Option<FigureId>,
}

impl SelectionModel {
    pub fn new(selected: Option<FigureId>) -> Self {
        Self { selected }
    }

    #[cfg(test)]
    pub fn selected(&self) -> Option<FigureId> {
        self.selected
    }

    pub fn select(&mut self, tree: &FigureTree, selected: Option<FigureId>) -> bool {
        let selected = selected.filter(|id| {
            tree.is_attached(*id)
                && tree.is_effectively_visible(*id)
                && tree.is_effectively_enabled(*id)
        });
        if self.selected == selected {
            return false;
        }
        self.selected = selected;
        true
    }

    pub fn reconcile(&mut self, tree: &FigureTree) -> bool {
        let selected = self.selected.filter(|id| {
            tree.is_attached(*id)
                && tree.is_effectively_visible(*id)
                && tree.is_effectively_enabled(*id)
        });
        if self.selected == selected {
            return false;
        }
        self.selected = selected;
        true
    }

    pub fn append_feedback(&self, tree: &FigureTree, submission: &mut RenderSubmission) {
        submission.commands.extend(self.feedback_commands(tree));
    }

    pub(crate) fn feedback_commands(&self, tree: &FigureTree) -> Vec<RenderCommand> {
        let Some(selected) = self.selected else {
            return Vec::new();
        };
        let Some(bounds) = tree.figure_bounds(selected) else {
            return Vec::new();
        };
        let Some(transform) = tree.local_to_surface_transform(selected) else {
            return Vec::new();
        };

        let mut canvas = NdCanvas::new();
        let [a, b, c, d, e, f] = transform.coeffs();
        canvas.push_state();
        canvas.transform(a, b, c, d, e, f);
        canvas.stroke_rect(
            OUTLINE_INSET,
            OUTLINE_INSET,
            (bounds.width - 2.0 * OUTLINE_INSET).max(0.0),
            (bounds.height - 2.0 * OUTLINE_INSET).max(0.0),
            OUTLINE_COLOR,
            OUTLINE_STROKE_WIDTH,
            LineCap::default(),
            LineJoin::default(),
        );
        canvas.pop_state();
        canvas.commands().clone()
    }
}

#[cfg(test)]
mod tests {
    use novadraw::{RectangleFigure, RenderCommandKind};

    use super::*;

    fn scene() -> (FigureTree, FigureId) {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(RectangleFigure::new(10.0, 20.0, 300.0, 200.0)));
        let child = tree.builder().add_child_to(
            root,
            Box::new(RectangleFigure::new(30.0, 40.0, 100.0, 60.0)),
        );
        (tree, child)
    }

    #[test]
    fn selection_is_owned_outside_the_figure_tree() {
        let (mut tree, child) = scene();
        let mut selection = SelectionModel::default();

        assert!(selection.select(&tree, Some(child)));
        assert_eq!(selection.selected(), Some(child));

        tree.set_visible(child, false);
        assert!(selection.reconcile(&tree));
        assert_eq!(selection.selected(), None);
    }

    #[test]
    fn feedback_uses_the_selected_figure_transform() {
        let (tree, child) = scene();
        let mut selection = SelectionModel::default();
        assert!(selection.select(&tree, Some(child)));

        let commands = selection.feedback_commands(&tree);
        assert!(commands.iter().any(|command| {
            matches!(
                command.kind,
                RenderCommandKind::StrokeRect {
                    color: OUTLINE_COLOR,
                    width: OUTLINE_STROKE_WIDTH,
                    ..
                }
            )
        }));
    }
}
