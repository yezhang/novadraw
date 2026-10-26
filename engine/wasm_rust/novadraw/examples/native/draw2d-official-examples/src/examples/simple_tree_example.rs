//! Source port of SimpleTreeExample and PageNode (IBM and contributors, EPL-2.0).
use std::sync::{Arc, Mutex};

use novadraw::{
    event::{EventContext, FigureEventHandler, MouseEvent},
    figure::{
        BOUNDS_SNAPSHOT, Border, BoundsDrawing, BoundsSnapshotCapability, CONTAINER,
        ChildClippingStrategy, ContainerCapability, Figure, FigureBoundsSnapshot,
        FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureContainer, FigureStyle,
        HitParticipation, INPUT, InputCapability, LabelFigure, RectangleFigure,
        border::{CompoundBorder, MarginBorder},
    },
    geometry::{Insets, Point, Rectangle},
    graphics::{Color, GradientStop, GraphicsError, LinearGradient, PaintContext, Path},
    layout::StackLayout,
    runtime::Runtime,
    tree::{FigureId, FigureSubtree, SubtreeNodeId},
};

use super::{
    runtime_with_fonts,
    tree_layout::{Branch, BranchLayout, BranchStyle, ContentsLayout, Lines, RectEdges, TreeModel},
};

const CORNER_SIZE: f64 = 10.0;
const PAGE_RECORDING_BYTES: usize = 4096;
const FOLDED_BORDER_RECORDING_BYTES: usize = 16384;
const SHADOW_OUTSET: f64 = 3.0;
fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::rgba(
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
        1.0,
    )
}

struct FoldedPageBorder;

impl Border for FoldedPageBorder {
    fn snapshot_bounds_drawing(
        &self,
        _: Option<&novadraw::figure::border::BorderSnapshot>,
    ) -> Result<BoundsDrawing, GraphicsError> {
        Ok(BoundsDrawing::new(
            (),
            FOLDED_BORDER_RECORDING_BYTES,
            |_, bounds, g| FoldedPageBorder.paint(bounds, g),
            |_, bounds| {
                Ok(Rectangle::new(
                    bounds.x - SHADOW_OUTSET,
                    bounds.y - SHADOW_OUTSET,
                    bounds.width + SHADOW_OUTSET * 2.0,
                    bounds.height + SHADOW_OUTSET * 2.0,
                ))
            },
        ))
    }
    fn get_insets(&self) -> Insets {
        Insets::new(CORNER_SIZE, 2.0, 4.0, 4.0)
    }
    fn paint(&self, bounds: Rectangle, g: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        let mut rect = Rectangle::new(bounds.x, bounds.y, bounds.width - 2.0, bounds.height - 2.0);
        g.push_state();
        g.set_stroke_width(4.0)?;
        g.set_stroke_paint(rgb(202, 202, 202));
        g.stroke_line(
            Point::new(rect.x + 3.0, rect.bottom()),
            Point::new(rect.right() + 3.0, rect.bottom()),
        )?;
        g.stroke_line(
            Point::new(rect.right(), rect.y + CORNER_SIZE),
            Point::new(rect.right(), rect.bottom()),
        )?;
        g.restore_state()?;
        rect.width -= 1.0;
        rect.height -= 1.0;
        g.stroke_rect(rect)?;
        g.set_stroke_paint(rgb(152, 168, 200));
        g.stroke_rect(Rectangle::new(
            rect.x + 1.0,
            rect.y + 1.0,
            rect.width - 2.0,
            rect.height - 2.0,
        ))?;
        let (x, y) = (rect.right(), rect.y - 1.0);
        let mut erase = Path::new();
        erase.move_to(x + 1.0, y);
        erase.line_to(x + 1.0, y + CORNER_SIZE + 1.0);
        erase.line_to(x - CORNER_SIZE, y);
        erase.close();
        // Graphics.restoreState restores the figure's inherited background.
        g.set_fill_paint(Color::WHITE);
        g.fill_path(&erase)?;
        let mut corner = Path::new();
        corner.move_to(x - CORNER_SIZE, y);
        corner.line_to(x, y + CORNER_SIZE);
        corner.line_to(x - CORNER_SIZE, y + CORNER_SIZE);
        corner.close();
        g.set_fill_paint(rgb(200, 208, 223));
        g.fill_path(&corner)?;
        g.set_stroke_paint(Color::BLACK);
        g.stroke_path(&corner)?;
        g.restore_state()?;
        g.set_stroke_paint(rgb(160, 172, 200));
        g.stroke_line(
            Point::new(rect.right() - CORNER_SIZE + 1.0, rect.y + 2.0),
            Point::new(rect.right() - 2.0, rect.y + CORNER_SIZE - 1.0),
        )?;
        g.pop_state()
    }
}

type PageSelection = (usize, Arc<Mutex<Option<usize>>>, Arc<Mutex<TreeModel>>);

pub(super) struct PageNode {
    border: CompoundBorder,
    selection: Option<PageSelection>,
}
impl PageNode {
    fn new() -> Self {
        Self {
            selection: None,
            border: CompoundBorder::new(
                FoldedPageBorder,
                MarginBorder::new(Color::BLACK, 0.0).with_margins(4.0, 4.0, 8.0, 3.0),
            ),
        }
    }

    fn selected(&self) -> bool {
        self.selection
            .as_ref()
            .is_some_and(|(index, selection, _)| {
                *selection.lock().expect("tree selection") == Some(*index)
            })
    }

    fn paint_page(
        selected: &bool,
        bounds: Rectangle,
        g: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        let height = bounds.height - 3.0;
        if height <= 0.0 || bounds.width <= 3.0 {
            return Ok(());
        }
        let (start, end) = if *selected {
            (rgb(49, 106, 197), rgb(100, 145, 215))
        } else {
            (rgb(232, 232, 240), rgb(176, 184, 216))
        };
        let gradient = LinearGradient::try_new(
            Point::ZERO,
            Point::new(0.0, height),
            &[
                GradientStop::try_new(0.0, start)?,
                GradientStop::try_new(1.0, end)?,
            ],
        )?;
        g.set_fill_paint(gradient);
        g.fill_rect(Rectangle::new(0.0, 0.0, bounds.width - 3.0, height))
    }
}

impl FigureBoundsSnapshot for PageNode {
    fn bounds_drawing_revision(&self) -> u64 {
        u64::from(self.selected())
    }
    fn snapshot_bounds_drawing(&self) -> Result<BoundsDrawing, GraphicsError> {
        Ok(BoundsDrawing::new(
            self.selected(),
            PAGE_RECORDING_BYTES,
            Self::paint_page,
            |_, bounds| Ok(bounds),
        ))
    }
}

impl Figure for PageNode {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }
    fn name(&self) -> &'static str {
        "PageNode"
    }
    fn initial_insets(&self) -> Insets {
        self.border.get_insets()
    }
    fn get_border(&self) -> Option<&dyn Border> {
        Some(&self.border)
    }
    fn initial_style(&self) -> FigureStyle {
        FigureStyle {
            foreground: Some(Color::BLACK),
            background: Some(Color::WHITE),
            ..FigureStyle::default()
        }
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())?;
        out.register(BOUNDS_SNAPSHOT, BoundsSnapshotCapability::of::<Self>())?;
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
    fn paint(&self, g: &mut PaintContext<'_>, bounds: Rectangle) -> Result<(), GraphicsError> {
        Self::paint_page(&self.selected(), bounds, g)
    }
}
impl FigureContainer for PageNode {}
impl FigureEventHandler for PageNode {
    fn on_mouse_pressed(&self, _: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        let Some((index, selection, model)) = &self.selection else {
            return false;
        };
        let previous = selection.lock().expect("tree selection").replace(*index);
        let model = model.lock().expect("tree model");
        if let Some(previous) = previous {
            let title = model.branches[previous].title;
            for label in context.scene().children(title).expect("PageNode children") {
                let mut style = context.scene().figure_style(label).expect("Label style");
                style.foreground = None;
                context.effects().set_style_later(label, style);
            }
            let bounds = context
                .scene()
                .figure_bounds(title)
                .expect("PageNode bounds");
            context
                .effects()
                .repaint_figure(title, Rectangle::new(0.0, 0.0, bounds.width, bounds.height));
        }
        for label in context
            .scene()
            .children(model.branches[*index].title)
            .expect("PageNode children")
        {
            let mut style = context.scene().figure_style(label).expect("Label style");
            style.foreground = Some(Color::WHITE);
            context.effects().set_style_later(label, style);
        }
        context.effects().repaint(None);
        true
    }
    fn on_mouse_double_clicked(&self, _: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        let Some((_, selection, model)) = &self.selection else {
            return false;
        };
        let mut model = model.lock().expect("tree model");
        if let Some(index) = *selection.lock().expect("tree selection")
            && !model.branches[index].children.is_empty()
        {
            if model.use_animation {
                model.pending_toggle = Some(index);
                return true;
            }
            model.branches[index].expanded = !model.branches[index].expanded;
            let root = &model.branches[0];
            context.effects().set_layout_manager_later(
                root.contents,
                Box::new(ContentsLayout {
                    model: self.selection.as_ref().unwrap().2.clone(),
                    index: 0,
                }),
            );
        }
        true
    }
}

pub(super) struct EnablePageSelection {
    pub index: usize,
    pub selection: Arc<Mutex<Option<usize>>>,
    pub model: Arc<Mutex<TreeModel>>,
}
impl novadraw::runtime::FigureComponentUpdate for EnablePageSelection {
    type Figure = PageNode;
    type Prepared = Self;
    type Error = std::convert::Infallible;
    fn prepare(
        self,
        _: &PageNode,
        _: novadraw::runtime::FigureComponentContext,
    ) -> Result<novadraw::runtime::PreparedFigureUpdate<Self>, Self::Error> {
        Ok(novadraw::runtime::PreparedFigureUpdate::paint(self))
    }
    fn commit(prepared: Self, target: &mut PageNode) {
        target.selection = Some((prepared.index, prepared.selection, prepared.model));
    }
}

struct TreeBranchFigure {
    root: bool,
    lines: Lines,
}
impl Figure for TreeBranchFigure {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }
    fn name(&self) -> &'static str {
        if self.root { "TreeRoot" } else { "TreeBranch" }
    }
    fn hit_participation(&self) -> HitParticipation {
        HitParticipation::DescendantsOnly
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
    fn paint(&self, g: &mut PaintContext<'_>, _: Rectangle) -> Result<(), GraphicsError> {
        g.set_stroke_paint(Color::BLACK);
        g.set_stroke_width(1.0)?;
        for (start, end) in self.lines.lock().expect("branch lines").iter() {
            g.stroke_line(*start, *end)?;
        }
        Ok(())
    }
}
impl FigureContainer for TreeBranchFigure {}

struct AnimatingLayer;
impl Figure for AnimatingLayer {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }
    fn name(&self) -> &'static str {
        "AnimatingLayer"
    }
    fn hit_participation(&self) -> HitParticipation {
        HitParticipation::DescendantsOnly
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}
impl FigureContainer for AnimatingLayer {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::DoNotClipChildBounds
    }
}

#[cfg_attr(not(test), allow(dead_code))]
struct Fixture {
    runtime: Runtime,
    model: Arc<Mutex<TreeModel>>,
}

pub fn simple_tree_example() -> Runtime {
    fixture(true).runtime
}

pub(super) fn add_branch(
    runtime: &mut Runtime,
    parent: Option<FigureId>,
    text: &str,
) -> (FigureId, FigureId, FigureId, Lines) {
    add_branch_with_root(runtime, parent, text, parent.is_none())
}

pub(super) fn add_branch_with_root(
    runtime: &mut Runtime,
    parent: Option<FigureId>,
    text: &str,
    root: bool,
) -> (FigureId, FigureId, FigureId, Lines) {
    let lines: Lines = Arc::new(Mutex::new(Vec::new()));
    let figure = Box::new(TreeBranchFigure {
        root,
        lines: lines.clone(),
    });
    let branch = match parent {
        Some(parent) => runtime
            .figure(parent)
            .expect("parent")
            .add(figure)
            .expect("TreeBranch"),
        None => runtime.set_contents(figure).expect("TreeRoot"),
    };
    // Official TreeBranch adds the transparent layer before the title.
    let contents = runtime
        .figure(branch)
        .expect("branch")
        .add(Box::new(AnimatingLayer))
        .expect("contents");
    let title = runtime
        .figure(branch)
        .expect("branch")
        .add(Box::new(PageNode::new()))
        .expect("PageNode");
    runtime
        .figure(title)
        .expect("title")
        .set_layout_manager(Box::new(StackLayout::new()))
        .expect("StackLayout");
    runtime
        .figure(title)
        .expect("title")
        .add(Box::new(LabelFigure::new(text)))
        .expect("Label");
    (branch, contents, title, lines)
}

pub(super) fn branch_subtree(
    text: &str,
    index: usize,
    selection: Arc<Mutex<Option<usize>>>,
    model: Arc<Mutex<TreeModel>>,
) -> (FigureSubtree, SubtreeNodeId, SubtreeNodeId, Lines) {
    let lines: Lines = Arc::new(Mutex::new(Vec::new()));
    let mut subtree = FigureSubtree::new(Box::new(TreeBranchFigure {
        root: false,
        lines: lines.clone(),
    }));
    let branch = subtree.root();
    let contents = subtree.add(branch, Box::new(AnimatingLayer)).unwrap();
    let mut page = PageNode::new();
    page.selection = Some((index, selection, model.clone()));
    let title = subtree.add(branch, Box::new(page)).unwrap();
    subtree
        .add(title, Box::new(LabelFigure::new(text)))
        .unwrap();
    subtree
        .set_layout_manager(title, Box::new(StackLayout::new()))
        .unwrap();
    subtree
        .set_layout_manager(
            branch,
            Box::new(BranchLayout {
                model: model.clone(),
                index,
                lines: lines.clone(),
            }),
        )
        .unwrap();
    subtree
        .set_layout_manager(contents, Box::new(ContentsLayout { model, index }))
        .unwrap();
    (subtree, contents, title, lines)
}

fn fixture(horizontal: bool) -> Fixture {
    let mut runtime = runtime_with_fonts();
    let canvas = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            800.0,
            600.0,
            Color::WHITE,
        )))
        .expect("white Canvas");
    runtime
        .figure(canvas)
        .unwrap()
        .set_layout_manager(Box::new(StackLayout::new()))
        .unwrap();
    let (root, root_contents, root_title, root_lines) =
        add_branch_with_root(&mut runtime, Some(canvas), "Graph Root", true);
    let (first, first_contents, first_title, first_lines) =
        add_branch(&mut runtime, Some(root_contents), "Child 1");
    let (second, second_contents, second_title, second_lines) =
        add_branch(&mut runtime, Some(root_contents), "Child 2");
    let (last, last_contents, last_title, last_lines) =
        add_branch(&mut runtime, Some(second_contents), "Child 1");
    let mut model = TreeModel::new(vec![
        Branch {
            figure: root,
            contents: root_contents,
            title: root_title,
            children: vec![1, 2],
            style: BranchStyle::Normal,
            centered: false,
            expanded: true,
            active: true,
        },
        Branch {
            figure: first,
            contents: first_contents,
            title: first_title,
            children: vec![],
            style: BranchStyle::Normal,
            centered: true,
            expanded: true,
            active: true,
        },
        Branch {
            figure: second,
            contents: second_contents,
            title: second_title,
            children: vec![3],
            style: BranchStyle::Hanging,
            centered: true,
            expanded: true,
            active: true,
        },
        Branch {
            figure: last,
            contents: last_contents,
            title: last_title,
            children: vec![],
            style: BranchStyle::Normal,
            centered: true,
            expanded: true,
            active: true,
        },
    ]);
    model.horizontal = horizontal;
    let model = Arc::new(Mutex::new(model));
    for (index, lines) in [root_lines, first_lines, second_lines, last_lines]
        .into_iter()
        .enumerate()
    {
        let locked = model.lock().expect("tree model");
        let branch = &locked.branches[index];
        runtime
            .figure(branch.figure)
            .expect("branch")
            .set_layout_manager(Box::new(BranchLayout {
                model: model.clone(),
                index,
                lines,
            }))
            .expect("branch layout");
        runtime
            .figure(branch.contents)
            .expect("contents")
            .set_layout_manager(Box::new(ContentsLayout {
                model: model.clone(),
                index,
            }))
            .expect("contents layout");
    }
    Fixture { runtime, model }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::render::{BackendCapabilities, SurfaceInfo};

    fn prepare(runtime: &mut Runtime) {
        let submission = runtime
            .frames()
            .prepare_submission(
                SurfaceInfo {
                    logical_width: 800.0,
                    logical_height: 600.0,
                    pixel_width: 800,
                    pixel_height: 600,
                    scale_factor: 1.0,
                },
                BackendCapabilities::RETAINED_PARTIAL.with_linear_gradients(),
            )
            .into_ready()
            .expect("valid tree frame");
        assert!(!submission.commands.is_empty());
    }

    #[test]
    fn page_gradient_follows_selection_and_restores_normal_palette() {
        use novadraw::{
            graphics::Paint,
            render::{CommandRecorder, command::RenderCommandKind},
        };
        let fixture = fixture(true);
        let selection = Arc::new(Mutex::new(None));
        let mut page = PageNode::new();
        page.selection = Some((1, selection.clone(), fixture.model));
        for (selected, expected) in [
            (None, rgb(232, 232, 240)),
            (Some(1), rgb(49, 106, 197)),
            (Some(2), rgb(232, 232, 240)),
        ] {
            *selection.lock().unwrap() = selected;
            let mut recorder = CommandRecorder::new();
            page.paint(&mut recorder.paint(), Rectangle::new(0.0, 0.0, 100.0, 50.0))
                .unwrap();
            let gradient = recorder
                .commands()
                .iter()
                .find_map(|c| {
                    if let RenderCommandKind::FillRect {
                        paint: Paint::LinearGradient(g),
                        ..
                    } = &c.kind
                    {
                        Some(g)
                    } else {
                        None
                    }
                })
                .unwrap();
            assert_eq!(gradient.stops()[0].color(), expected);
        }
    }

    #[test]
    fn official_page_and_branch_composition() {
        let fixture = fixture(true);
        let canvas = fixture.runtime.tree().contents().unwrap();
        assert_eq!(
            fixture
                .runtime
                .tree()
                .figure_style(canvas)
                .unwrap()
                .background,
            Some(Color::WHITE)
        );
        for branch in &fixture.model.lock().unwrap().branches {
            assert_eq!(
                fixture.runtime.tree().child_order(branch.figure).unwrap(),
                &[branch.contents, branch.title]
            );
            let labels = fixture.runtime.tree().child_order(branch.title).unwrap();
            assert_eq!(labels.len(), 1);
            assert_eq!(
                fixture
                    .runtime
                    .tree()
                    .child_order(branch.contents)
                    .unwrap()
                    .len(),
                branch.children.len()
            );
        }
    }

    #[test]
    fn normal_children_share_a_row_and_hanging_child_is_indented() {
        let mut fixture = fixture(true);
        prepare(&mut fixture.runtime);
        let locked = fixture.model.lock().unwrap();
        let b = &locked.branches;
        let first = fixture.runtime.tree().figure_bounds(b[1].figure).unwrap();
        let second = fixture.runtime.tree().figure_bounds(b[2].figure).unwrap();
        assert_eq!(first.y, second.y);
        assert!(second.x >= first.right() + 10.0);
        let hanging_title = fixture.runtime.tree().figure_bounds(b[2].title).unwrap();
        let hanging_contents = fixture.runtime.tree().figure_bounds(b[2].contents).unwrap();
        assert_eq!(hanging_contents.x, hanging_title.x + 20.0);
        assert!(hanging_contents.y >= hanging_title.bottom() + 10.0);
    }

    #[test]
    fn transposition_preserves_the_same_tree_geometry() {
        let mut horizontal = fixture(true);
        let mut vertical = fixture(false);
        prepare(&mut horizontal.runtime);
        prepare(&mut vertical.runtime);
        // Root orientation switches the major axis, not the source hierarchy.
        for branch in &vertical.model.lock().unwrap().branches {
            let title = vertical.runtime.tree().figure_bounds(branch.title).unwrap();
            assert!(title.width > 0.0 && title.height > 0.0);
        }
        let locked = vertical.model.lock().unwrap();
        let b = &locked.branches;
        let first = vertical.runtime.tree().figure_bounds(b[1].figure).unwrap();
        let second = vertical.runtime.tree().figure_bounds(b[2].figure).unwrap();
        assert_eq!(first.x, second.x);
        assert!(second.y >= first.bottom() + 10.0);
        let title = vertical.runtime.tree().figure_bounds(b[2].title).unwrap();
        let contents = vertical
            .runtime
            .tree()
            .figure_bounds(b[2].contents)
            .unwrap();
        assert_eq!(contents.y, title.y + 20.0);
    }
}
