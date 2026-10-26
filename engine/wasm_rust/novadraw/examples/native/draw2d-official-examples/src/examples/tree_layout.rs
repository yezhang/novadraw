//! Ports of the official example's NormalLayout, HangingLayout and TreeLayout.
//! Derived from Eclipse Draw2D examples (IBM and contributors, EPL-2.0).
//! Coordinates below are in the upstream Transposer's normalized domain.
use novadraw::{
    animation::{
        AnimationError, AnimationPlan, AnimationValue, GeometryLine, GeometryPatch,
        GeometryProvider, GeometryProviderContext, GeometryProviderKey,
    },
    figure::{FigureMeasurement, MeasureConstraints},
    geometry::{Dimension, Point, Rectangle, Translatable},
    graphics::Color,
    layout::{LayoutError, LayoutManager, LayoutOutput, LayoutSceneQuery},
    tree::FigureId,
};
use std::sync::{Arc, Mutex};

const CONTOUR_CLEARANCE: f64 = 5.0;
const DEFAULT_SPACING: f64 = 10.0;
const LINE_WIDTH: f64 = 1.0;

/// Shared upstream endpoint arithmetic, in the normalized branch-local domain.
#[derive(Clone, Copy, PartialEq)]
struct ConnectorGeometry {
    title: Rectangle,
    contents: Rectangle,
    style: BranchStyle,
    minor: f64,
}

#[derive(Clone, Copy)]
struct ConnectorSpan {
    line: (Point, Point),
    minimum: f64,
    maximum: f64,
    previous: Option<GeometryProviderKey<Self>>,
}

impl PartialEq for ConnectorSpan {
    fn eq(&self, other: &Self) -> bool {
        self.line == other.line
            && self.minimum == other.minimum
            && self.maximum == other.maximum
            && self.previous.map(|key| key.dependency())
                == other.previous.map(|key| key.dependency())
    }
}

impl AnimationValue for ConnectorSpan {
    fn is_valid(&self) -> bool {
        [
            self.line.0.x(),
            self.line.0.y(),
            self.line.1.x(),
            self.line.1.y(),
            self.minimum,
            self.maximum,
        ]
        .into_iter()
        .all(f64::is_finite)
    }
    fn interpolate(&self, _: &Self, _: f64) -> Self {
        *self
    }
}

impl ConnectorGeometry {
    fn seed(self) -> ConnectorSpan {
        let coordinate = match self.style {
            BranchStyle::Normal => self.title.x + (self.title.width / 2.0).trunc(),
            BranchStyle::Hanging => self.title.bottom(),
        };
        ConnectorSpan {
            line: (Point::ZERO, Point::ZERO),
            minimum: coordinate,
            maximum: coordinate,
            previous: None,
        }
    }

    fn child(self, title: Rectangle, mut span: ConnectorSpan) -> ConnectorSpan {
        match self.style {
            BranchStyle::Normal => {
                let bottom = self.contents.y - LINE_WIDTH;
                let middle = ((self.title.bottom() + bottom) / 2.0).trunc();
                let x = title.x + (title.width / 2.0).trunc();
                span.line = (Point::new(x, middle), Point::new(x, bottom));
                span.minimum = span.minimum.min(x);
                span.maximum = span.maximum.max(x);
            }
            BranchStyle::Hanging => {
                let x = self.title.x + self.minor;
                let y = title.y + (title.height / 2.0).trunc();
                span.line = (Point::new(x, y), Point::new(title.x, y));
                span.maximum = span.maximum.max(y);
            }
        }
        span
    }

    fn trunks(self, span: ConnectorSpan) -> impl Iterator<Item = (Point, Point)> {
        match self.style {
            BranchStyle::Normal => {
                let x = self.title.x + (self.title.width / 2.0).trunc();
                let middle = ((self.title.bottom() + self.contents.y - LINE_WIDTH) / 2.0).trunc();
                [
                    Some((Point::new(x, self.title.bottom()), Point::new(x, middle))),
                    Some((
                        Point::new(span.minimum, middle),
                        Point::new(span.maximum, middle),
                    )),
                ]
            }
            BranchStyle::Hanging => {
                let x = self.title.x + self.minor;
                [
                    Some((
                        Point::new(x, self.title.bottom()),
                        Point::new(x, span.maximum),
                    )),
                    None,
                ]
            }
        }
        .into_iter()
        .flatten()
    }
}

fn transpose_rect(rect: Rectangle, horizontal: bool) -> Rectangle {
    if horizontal {
        rect
    } else {
        Rectangle::new(rect.y, rect.x, rect.height, rect.width)
    }
}

fn transpose_point(point: Point, horizontal: bool) -> Point {
    if horizontal {
        point
    } else {
        Point::new(point.y(), point.x())
    }
}

fn connector_lines(
    geometry: ConnectorGeometry,
    children: impl IntoIterator<Item = Rectangle>,
    horizontal: bool,
) -> Vec<(Point, Point)> {
    let mut span = geometry.seed();
    let mut lines = Vec::new();
    for title in children {
        span = geometry.child(title, span);
        lines.push(span.line);
    }
    if !lines.is_empty() {
        lines.extend(geometry.trunks(span));
    }
    lines
        .into_iter()
        .map(|(a, b)| {
            (
                transpose_point(a, horizontal),
                transpose_point(b, horizontal),
            )
        })
        .collect()
}

fn presented_rectangle(
    context: &GeometryProviderContext<'_>,
    figure: FigureId,
    branch: FigureId,
    horizontal: bool,
) -> Result<Rectangle, AnimationError> {
    let bounds = context.figure(figure)?.bounds;
    let transform = context
        .relative_transform(figure, branch)?
        .ok_or(AnimationError::InvalidValue)?;
    let mut local = Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
    local.transform(transform);
    Ok(transpose_rect(local, horizontal))
}

/// Inline provider inputs and an explicitly declared chain bound storage and query work.
/// The final drawing owns all lines; no per-frame write reaches the static Lines store.
pub(super) fn with_tree_connectors(mut plan: AnimationPlan, model: &TreeModel) -> AnimationPlan {
    for branch in model.branches.iter().filter(|branch| branch.active) {
        if branch.children.is_empty() {
            continue;
        }
        let owner = branch.figure;
        let title = branch.title;
        let contents = branch.contents;
        let horizontal = model.horizontal;
        let style = branch.style;
        let minor = model.minor;
        let mut previous = None::<GeometryProviderKey<ConnectorSpan>>;
        let mut dependencies = Vec::new();
        for child in &branch.children {
            let child_title = model.branches[*child].title;
            let key = GeometryProviderKey::<ConnectorSpan>::new(child_title);
            plan = plan.with_geometry_provider(GeometryProvider::new(
                key,
                [owner, title, contents, child_title],
                previous.into_iter().map(|key| key.dependency()).collect(),
                move |context| {
                    let geometry = ConnectorGeometry {
                        title: presented_rectangle(context, title, owner, horizontal)?,
                        contents: presented_rectangle(context, contents, owner, horizontal)?,
                        style,
                        minor,
                    };
                    let span = match previous {
                        Some(key) => *context.value(key)?,
                        None => geometry.seed(),
                    };
                    let mut span = geometry.child(
                        presented_rectangle(context, child_title, owner, horizontal)?,
                        span,
                    );
                    span.previous = previous;
                    Ok(span)
                },
                |_| Ok(GeometryPatch::default()),
            ));
            dependencies.push(key.dependency());
            previous = Some(key);
        }
        let tail = previous.expect("nonempty child chain");
        plan = plan.with_geometry_provider(GeometryProvider::drawing(
            GeometryProviderKey::<ConnectorGeometry>::new(owner),
            [owner, title, contents],
            dependencies,
            branch.children.len() + 2,
            move |context| {
                Ok(ConnectorGeometry {
                    title: presented_rectangle(context, title, owner, horizontal)?,
                    contents: presented_rectangle(context, contents, owner, horizontal)?,
                    style,
                    minor,
                })
            },
            move |geometry, context, drawing| {
                // Local visibility of contents is retained for collapse, never inferred from
                // mutable model state. External ancestor hiding remains engine-owned.
                if !context.figure(contents)?.effective_visible {
                    return Ok(());
                }
                let mut key = Some(tail);
                let span = *context.value(tail)?;
                while let Some(current) = key {
                    let value = context.value(current)?;
                    drawing.line(GeometryLine {
                        start: transpose_point(value.line.0, horizontal),
                        end: transpose_point(value.line.1, horizontal),
                        width: LINE_WIDTH,
                        color: Color::BLACK,
                    })?;
                    key = value.previous;
                }
                for (a, b) in geometry.trunks(span) {
                    drawing.line(GeometryLine {
                        start: transpose_point(a, horizontal),
                        end: transpose_point(b, horizontal),
                        width: LINE_WIDTH,
                        color: Color::BLACK,
                    })?;
                }
                Ok(())
            },
        ));
    }
    plan
}

impl AnimationValue for ConnectorGeometry {
    fn is_valid(&self) -> bool {
        self.title.is_valid() && self.contents.is_valid() && self.minor.is_finite()
    }
    fn interpolate(&self, _: &Self, _: f64) -> Self {
        *self
    }
}

// Preserve the upstream Rectangle edge arithmetic in the algorithm translation.
pub(super) trait RectEdges {
    fn right(&self) -> f64;
    fn bottom(&self) -> f64;
}
impl RectEdges for Rectangle {
    fn right(&self) -> f64 {
        self.x + self.width
    }
    fn bottom(&self) -> f64 {
        self.y + self.height
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum BranchStyle {
    Normal,
    Hanging,
}

pub(super) struct Branch {
    pub figure: FigureId,
    pub contents: FigureId,
    pub title: FigureId,
    pub children: Vec<usize>,
    pub style: BranchStyle,
    pub centered: bool,
    pub expanded: bool,
    pub active: bool,
}

pub(super) struct TreeModel {
    pub branches: Vec<Branch>,
    pub horizontal: bool,
    pub compressed: bool,
    pub major: f64,
    pub minor: f64,
    pub use_animation: bool,
    pub pending_toggle: Option<usize>,
    pub animation: Option<novadraw::animation::AnimationId>,
    pub viewport: Option<FigureId>,
    pub animation_error: Option<String>,
}

impl TreeModel {
    pub fn new(branches: Vec<Branch>) -> Self {
        Self {
            branches,
            horizontal: true,
            compressed: false,
            major: DEFAULT_SPACING,
            minor: DEFAULT_SPACING,
            use_animation: false,
            pending_toggle: None,
            animation: None,
            viewport: None,
            animation_error: None,
        }
    }

    fn solve(&self, scene: &LayoutSceneQuery<'_>) -> Result<Vec<Plan>, LayoutError> {
        let sizes = self
            .branches
            .iter()
            .map(|branch| {
                if !branch.active {
                    return Ok(Dimension::ZERO);
                }
                let size = scene
                    .preferred_measurement(branch.title, MeasureConstraints::UNBOUNDED)?
                    .size();
                // SWT's preferred sizes and geometry are integers.
                Ok(self.transpose_size(Dimension::new(size.width.ceil(), size.height.ceil())))
            })
            .collect::<Result<Vec<_>, LayoutError>>()?;
        Ok(self.solve_sizes(&sizes))
    }

    fn transpose_size(&self, size: Dimension) -> Dimension {
        if self.horizontal {
            size
        } else {
            Dimension::new(size.height, size.width)
        }
    }

    fn transpose_rect(&self, rect: Rectangle) -> Rectangle {
        if self.horizontal {
            rect
        } else {
            Rectangle::new(rect.y, rect.x, rect.height, rect.width)
        }
    }

    fn rows(&self, index: usize, sizes: &[Dimension]) -> Vec<f64> {
        let branch = &self.branches[index];
        let mut rows = vec![sizes[index].height + self.major];
        if !branch.expanded {
            return rows;
        }
        for child in &branch.children {
            let child_rows = self.rows(*child, sizes);
            match branch.style {
                BranchStyle::Hanging => rows.extend(child_rows),
                BranchStyle::Normal => {
                    rows.resize(rows.len().max(child_rows.len() + 1), 0.0);
                    for (depth, height) in child_rows.iter().enumerate() {
                        rows[depth + 1] = rows[depth + 1].max(*height);
                    }
                }
            }
        }
        rows
    }

    fn assign_rows(&self, index: usize, offset: usize, rows: &[f64], assigned: &mut [f64]) {
        assigned[index] = rows[offset];
        if !self.branches[index].expanded {
            return;
        }
        let mut next = offset + 1;
        for child in &self.branches[index].children {
            self.assign_rows(*child, next, rows, assigned);
            if self.branches[index].style == BranchStyle::Hanging {
                next += self.depth(*child);
            }
        }
    }

    fn depth(&self, index: usize) -> usize {
        let branch = &self.branches[index];
        if !branch.expanded {
            return 1;
        }
        match branch.style {
            BranchStyle::Normal => {
                1 + branch
                    .children
                    .iter()
                    .map(|c| self.depth(*c))
                    .max()
                    .unwrap_or(0)
            }
            BranchStyle::Hanging => {
                1 + branch
                    .children
                    .iter()
                    .map(|c| self.depth(*c))
                    .sum::<usize>()
            }
        }
    }

    fn solve_sizes(&self, sizes: &[Dimension]) -> Vec<Plan> {
        let rows = self.rows(0, sizes);
        let mut assigned = vec![0.0; sizes.len()];
        self.assign_rows(0, 0, &rows, &mut assigned);
        let mut plans = vec![Plan::default(); sizes.len()];
        self.plan_branch(0, sizes, &assigned, &mut plans);
        plans
    }

    fn plan_branch(&self, index: usize, sizes: &[Dimension], rows: &[f64], plans: &mut [Plan]) {
        let branch = &self.branches[index];
        if !branch.expanded {
            // NormalLayout lays out a zero-size hidden pane one row below
            // the title; HangingLayout retains its allocated row in preferred size.
            let height = rows[index] - self.major;
            plans[index] = Plan {
                size: Dimension::new(
                    sizes[index].width,
                    if branch.style == BranchStyle::Hanging {
                        rows[index]
                    } else {
                        height
                    },
                ),
                title: Rectangle::new(0.0, 0.0, sizes[index].width, height),
                contents: Rectangle::new(0.0, rows[index], 0.0, 0.0),
                left: vec![0.0],
                right: vec![0.0],
                ..Plan::default()
            };
            return;
        }
        for child in &branch.children {
            self.plan_branch(*child, sizes, rows, plans);
        }
        let row = rows[index];
        let mut title = Rectangle::new(0.0, 0.0, sizes[index].width, row - self.major);
        let mut children = Vec::<Rectangle>::new();
        let mut content_size = Dimension::ZERO;

        if branch.style == BranchStyle::Hanging {
            // TreeBranch.validate installs a non-stretching ToolbarLayout,
            // opposite to the root orientation, with its default zero spacing.
            for child in &branch.children {
                let size = plans[*child].size;
                children.push(Rectangle::new(
                    0.0,
                    content_size.height,
                    size.width,
                    size.height,
                ));
                content_size.width = content_size.width.max(size.width);
                content_size.height += size.height;
            }
        } else {
            let mut current = 0.0;
            let mut right_contour: Option<Vec<f64>> = None;
            let mut previous_depth = 0;
            for (i, child) in branch.children.iter().enumerate() {
                let plan = &plans[*child];
                let (overlap, contact) = overlap(right_contour.as_deref(), &plan.left);
                let overlap = if self.compressed { overlap } else { 0.0 };
                children.push(Rectangle::new(
                    current - overlap,
                    0.0,
                    plan.size.width,
                    plan.size.height,
                ));
                let advance = self.minor + plan.size.width - overlap;
                right_contour = Some(merge_right(right_contour.as_deref(), &plan.right, advance));
                current += advance;
                let correction = -children[i].x;
                if correction > 0.0 {
                    current += correction;
                    for bounds in &mut children {
                        bounds.x += correction;
                    }
                }
                if contact > previous_depth {
                    let previous = &plans[branch.children[i - 1]];
                    let slack = children[i].x - children[i - 1].right() - self.minor
                        + overlap_between(&previous.right, &plan.left);
                    let end = i;
                    let mut begin = end - 1;
                    while begin > 0 && plans[branch.children[begin]].left.len() < contact {
                        begin -= 1;
                    }
                    for (j, bounds) in children.iter_mut().enumerate().take(end).skip(begin + 1) {
                        bounds.x += (slack * (j - begin) as f64 / (end - begin) as f64).trunc();
                    }
                }
                previous_depth = plan.left.len();
            }
            for bounds in &children {
                content_size.width = content_size.width.max(bounds.right());
                content_size.height = content_size.height.max(bounds.bottom());
            }
        }

        let mut contents = Rectangle::new(0.0, row, content_size.width, content_size.height);
        let size = if branch.style == BranchStyle::Hanging {
            contents.x = self.minor * 2.0;
            Dimension::new(
                title.width.max(if children.is_empty() {
                    0.0
                } else {
                    contents.right()
                }),
                row + contents.height,
            )
        } else if children.is_empty() {
            Dimension::new(title.width, title.height)
        } else {
            let first = branch.children[0];
            let last = *branch.children.last().expect("last subtree");
            let mut left_inset = (plans[first].left[0] + children[0].x).max(0.0);
            let right_inset = (plans[last].right[0]
                - children.last().expect("last bounds").right()
                + contents.width)
                .max(0.0);
            if branch.centered {
                left_inset +=
                    ((contents.width - left_inset - right_inset - title.width) / 2.0).trunc();
            }
            if left_inset > 0.0 {
                title.x += left_inset;
            } else {
                contents.x -= left_inset;
            }
            Dimension::new(title.right().max(contents.right()), contents.bottom())
        };

        let depth = self.depth(index);
        let mut left = vec![0.0; depth];
        let mut right = vec![0.0; depth];
        left[0] = title.x;
        right[0] = size.width - title.right();
        match branch.style {
            BranchStyle::Normal => {
                let mut covered = 0;
                for (child, bounds) in branch.children.iter().zip(&children) {
                    let plan = &plans[*child];
                    for d in covered..plan.left.len() {
                        left[d + 1] = plan.left[d] + bounds.x + contents.x;
                    }
                    covered = covered.max(plan.left.len());
                }
                covered = 0;
                for (child, bounds) in branch.children.iter().zip(&children).rev() {
                    let plan = &plans[*child];
                    for d in covered..plan.right.len() {
                        right[d + 1] = plan.right[d] + size.width - contents.x - bounds.right();
                    }
                    covered = covered.max(plan.right.len());
                }
            }
            BranchStyle::Hanging => {
                left[1..].fill(self.minor);
                let mut offset = 1;
                for (child, bounds) in branch.children.iter().zip(&children) {
                    let plan = &plans[*child];
                    for (d, margin) in plan.right.iter().enumerate() {
                        right[offset + d] = margin + size.width - contents.x - bounds.right();
                    }
                    offset += plan.right.len();
                }
            }
        }
        plans[index] = Plan {
            size,
            title,
            contents,
            children,
            left,
            right,
        };
    }
}

fn overlap(left: Option<&[f64]>, right: &[f64]) -> (f64, usize) {
    let Some(left) = left else { return (0.0, 0) };
    left.iter()
        .zip(right)
        .enumerate()
        .fold((f64::INFINITY, 0), |best, (depth, (l, r))| {
            let value = l + r - if depth > 0 { CONTOUR_CLEARANCE } else { 0.0 };
            if value < best.0 {
                (value, depth + 1)
            } else {
                best
            }
        })
}

fn overlap_between(left: &[f64], right: &[f64]) -> f64 {
    overlap(Some(left), right).0
}

fn merge_right(old: Option<&[f64]>, add: &[f64], shift: f64) -> Vec<f64> {
    let mut result = add.to_vec();
    if let Some(old) = old {
        result.extend(old.iter().skip(add.len()).map(|margin| margin + shift));
    }
    result
}

#[derive(Clone, Debug, Default)]
struct Plan {
    size: Dimension,
    title: Rectangle,
    contents: Rectangle,
    children: Vec<Rectangle>,
    left: Vec<f64>,
    right: Vec<f64>,
}

pub(super) type Lines = Arc<Mutex<Vec<(novadraw::geometry::Point, novadraw::geometry::Point)>>>;

pub(super) struct BranchLayout {
    pub model: Arc<Mutex<TreeModel>>,
    pub index: usize,
    pub lines: Lines,
}

impl LayoutManager for BranchLayout {
    fn preferred_measurement(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        scene: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        let model = self.model.lock().expect("tree model");
        let size = model.transpose_size(model.solve(scene)?[self.index].size);
        Ok(FigureMeasurement::new(size.width, size.height, None))
    }
    fn minimum_size(
        &self,
        id: FigureId,
        constraints: MeasureConstraints,
        scene: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        Ok(self.preferred_measurement(id, constraints, scene)?.size())
    }
    fn layout(
        &mut self,
        id: FigureId,
        scene: &LayoutSceneQuery<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        let model = self.model.lock().expect("tree model");
        let plans = model.solve(scene)?;
        let plan = &plans[self.index];
        let branch = &model.branches[self.index];
        let allocated = model.transpose_rect(scene.container_bounds(id)?);
        let adjust = if branch.style == BranchStyle::Normal {
            ((allocated.width - plan.size.width) / 2.0).trunc()
        } else {
            0.0
        };
        let mut title = plan.title;
        title.x += allocated.x + adjust;
        title.y += allocated.y;
        let mut contents = plan.contents;
        contents.x += allocated.x + adjust;
        contents.y += allocated.y;
        out.set_child_bounds(branch.title, model.transpose_rect(title));
        out.set_child_bounds(branch.contents, model.transpose_rect(contents));
        out.set_child_visible(branch.contents, branch.expanded);

        let lines = if branch.expanded {
            connector_lines(
                ConnectorGeometry {
                    title,
                    contents,
                    style: branch.style,
                    minor: model.minor,
                },
                branch
                    .children
                    .iter()
                    .zip(&plan.children)
                    .map(|(child, location)| {
                        let child_title = plans[*child].title;
                        Rectangle::new(
                            contents.x + location.x + child_title.x,
                            contents.y + location.y + child_title.y,
                            child_title.width,
                            child_title.height,
                        )
                    }),
                model.horizontal,
            )
        } else {
            Vec::new()
        };
        *self.lines.lock().expect("branch lines") = lines;
        Ok(())
    }
}

pub(super) struct ContentsLayout {
    pub model: Arc<Mutex<TreeModel>>,
    pub index: usize,
}

impl LayoutManager for ContentsLayout {
    fn preferred_measurement(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        scene: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        let model = self.model.lock().expect("tree model");
        let plans = model.solve(scene)?;
        let bounds = model.transpose_rect(plans[self.index].contents);
        Ok(FigureMeasurement::new(bounds.width, bounds.height, None))
    }
    fn minimum_size(
        &self,
        id: FigureId,
        constraints: MeasureConstraints,
        scene: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        Ok(self.preferred_measurement(id, constraints, scene)?.size())
    }
    fn layout(
        &mut self,
        _: FigureId,
        scene: &LayoutSceneQuery<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        let model = self.model.lock().expect("tree model");
        let plans = model.solve(scene)?;
        for (child, bounds) in model.branches[self.index]
            .children
            .iter()
            .zip(&plans[self.index].children)
        {
            out.set_child_bounds(model.branches[*child].figure, model.transpose_rect(*bounds));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::{figure::RectangleFigure, runtime::Runtime};

    fn model() -> TreeModel {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0)))
            .unwrap();
        let mut branches = Vec::new();
        for _ in 0..5 {
            let figure = runtime
                .figure(root)
                .unwrap()
                .add(Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0)))
                .unwrap();
            branches.push(Branch {
                figure,
                contents: figure,
                title: figure,
                children: vec![],
                style: BranchStyle::Normal,
                centered: true,
                expanded: true,
                active: true,
            });
        }
        branches[0].children = vec![1, 2];
        branches[1].children = vec![3];
        branches[2].children = vec![4];
        TreeModel::new(branches)
    }

    #[test]
    fn contour_contact_and_distant_sibling_history_match_tree_layout() {
        assert_eq!(overlap(None, &[0.0, 20.0]), (0.0, 0));
        assert_eq!(overlap(Some(&[40.0, 8.0, 20.0]), &[10.0, 12.0]), (15.0, 2));
        assert_eq!(
            merge_right(Some(&[1.0, 2.0, 3.0]), &[8.0], 25.0),
            vec![8.0, 27.0, 28.0]
        );
    }

    #[test]
    fn compression_uses_contours_and_retains_the_official_gap() {
        let mut model = model();
        let sizes = [
            Dimension::new(80.0, 20.0),
            Dimension::new(20.0, 20.0),
            Dimension::new(20.0, 20.0),
            Dimension::new(100.0, 20.0),
            Dimension::new(100.0, 20.0),
        ];
        let plain = model.solve_sizes(&sizes);
        model.compressed = true;
        let compressed = model.solve_sizes(&sizes);
        // The two wide grandchildren meet at depth 2, where upstream reserves
        // the extra five-pixel contour clearance in addition to minor spacing.
        assert_eq!(
            compressed[0].size.width,
            plain[0].size.width + CONTOUR_CLEARANCE
        );
        let a = compressed[0].children[0].x + compressed[1].contents.x;
        let b = compressed[0].children[1].x + compressed[2].contents.x;
        assert_eq!(b - (a + sizes[3].width), model.minor + CONTOUR_CLEARANCE);
    }

    #[test]
    fn mixed_hanging_and_normal_branches_share_root_row_heights() {
        let mut model = model();
        model.branches[2].style = BranchStyle::Hanging;
        let sizes = [
            Dimension::new(80.0, 20.0),
            Dimension::new(20.0, 30.0),
            Dimension::new(20.0, 50.0),
            Dimension::new(100.0, 40.0),
            Dimension::new(100.0, 60.0),
        ];
        let plans = model.solve_sizes(&sizes);
        assert_eq!(plans[1].title.height, 50.0);
        assert_eq!(plans[2].title.height, 50.0);
        assert_eq!(plans[3].title.height, 60.0);
        assert_eq!(plans[4].title.height, 60.0);
        assert_eq!(plans[2].contents.x, model.minor * 2.0);
        assert_eq!(plans[2].left[1], model.minor);
    }
}
