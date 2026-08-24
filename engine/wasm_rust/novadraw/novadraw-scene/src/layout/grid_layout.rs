//! Grid layout with per-child alignment, span and excess-space constraints.

use super::{LayoutContext, LayoutManager};
use crate::graph::BlockId;
use novadraw_geometry::Rectangle;

const DEFAULT_MARGIN: f64 = 5.0;
const DEFAULT_SPACING: f64 = 5.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GridAlignment {
    #[default]
    Start,
    Center,
    End,
    Fill,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridConstraint {
    pub horizontal_alignment: GridAlignment,
    pub vertical_alignment: GridAlignment,
    pub width_hint: Option<f64>,
    pub height_hint: Option<f64>,
    pub horizontal_indent: f64,
    pub horizontal_span: usize,
    pub vertical_span: usize,
    pub grab_horizontal: bool,
    pub grab_vertical: bool,
}

impl GridConstraint {
    pub fn fill() -> Self {
        Self {
            horizontal_alignment: GridAlignment::Fill,
            vertical_alignment: GridAlignment::Fill,
            grab_horizontal: true,
            grab_vertical: true,
            ..Self::default()
        }
    }

    pub fn with_span(mut self, columns: usize, rows: usize) -> Self {
        self.horizontal_span = columns.max(1);
        self.vertical_span = rows.max(1);
        self
    }
}

impl Default for GridConstraint {
    fn default() -> Self {
        Self {
            horizontal_alignment: GridAlignment::Start,
            vertical_alignment: GridAlignment::Center,
            width_hint: None,
            height_hint: None,
            horizontal_indent: 0.0,
            horizontal_span: 1,
            vertical_span: 1,
            grab_horizontal: false,
            grab_vertical: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GridLayout {
    columns: usize,
    equal_column_widths: bool,
    margin_width: f64,
    margin_height: f64,
    horizontal_spacing: f64,
    vertical_spacing: f64,
}

#[derive(Clone, Copy)]
struct Placement {
    child: BlockId,
    row: usize,
    column: usize,
    row_span: usize,
    column_span: usize,
    constraint: GridConstraint,
    preferred: (f64, f64),
    minimum: (f64, f64),
}

impl GridLayout {
    pub fn new(columns: usize) -> Self {
        Self {
            columns: columns.max(1),
            equal_column_widths: false,
            margin_width: DEFAULT_MARGIN,
            margin_height: DEFAULT_MARGIN,
            horizontal_spacing: DEFAULT_SPACING,
            vertical_spacing: DEFAULT_SPACING,
        }
    }

    pub fn with_equal_column_widths(mut self, equal: bool) -> Self {
        self.equal_column_widths = equal;
        self
    }

    pub fn with_margins(mut self, horizontal: f64, vertical: f64) -> Self {
        self.margin_width = horizontal.max(0.0);
        self.margin_height = vertical.max(0.0);
        self
    }

    pub fn with_spacing(mut self, horizontal: f64, vertical: f64) -> Self {
        self.horizontal_spacing = horizontal.max(0.0);
        self.vertical_spacing = vertical.max(0.0);
        self
    }

    fn constraint(ctx: &dyn LayoutContext, child: BlockId) -> GridConstraint {
        ctx.get_constraint(child)
            .and_then(|constraint| constraint.as_any().downcast_ref::<GridConstraint>())
            .copied()
            .unwrap_or_default()
    }

    fn placements(&self, container: BlockId, ctx: &dyn LayoutContext) -> Vec<Placement> {
        let mut occupied: Vec<Vec<bool>> = Vec::new();
        let mut row = 0;
        let mut column = 0;
        let mut placements = Vec::new();

        for (child, _) in ctx.get_children(container) {
            let constraint = Self::constraint(ctx, child);
            let column_span = constraint.horizontal_span.max(1).min(self.columns);
            let row_span = constraint.vertical_span.max(1);

            loop {
                if column + column_span > self.columns {
                    row += 1;
                    column = 0;
                    continue;
                }
                while occupied.len() < row + row_span {
                    occupied.push(vec![false; self.columns]);
                }
                let fits = (row..row + row_span).all(|candidate_row| {
                    (column..column + column_span)
                        .all(|candidate_column| !occupied[candidate_row][candidate_column])
                });
                if fits {
                    break;
                }
                column += 1;
            }

            for occupied_row in occupied.iter_mut().skip(row).take(row_span) {
                for cell in occupied_row.iter_mut().skip(column).take(column_span) {
                    *cell = true;
                }
            }

            let width_hint = constraint.width_hint.unwrap_or(-1.0);
            let height_hint = constraint.height_hint.unwrap_or(-1.0);
            let mut preferred = ctx.get_preferred_size(child, width_hint, height_hint);
            let mut minimum = ctx.get_minimum_size(child, width_hint, height_hint);
            if let Some(width) = constraint.width_hint {
                preferred.0 = width.max(0.0);
                minimum.0 = minimum.0.min(preferred.0);
            }
            if let Some(height) = constraint.height_hint {
                preferred.1 = height.max(0.0);
                minimum.1 = minimum.1.min(preferred.1);
            }
            preferred.0 += constraint.horizontal_indent.max(0.0);
            minimum.0 += constraint.horizontal_indent.max(0.0);

            placements.push(Placement {
                child,
                row,
                column,
                row_span,
                column_span,
                constraint,
                preferred,
                minimum,
            });
            column += column_span;
        }

        placements
    }

    fn track_sizes(
        &self,
        placements: &[Placement],
        minimum: bool,
    ) -> (Vec<f64>, Vec<f64>, Vec<bool>, Vec<bool>) {
        let row_count = placements
            .iter()
            .map(|placement| placement.row + placement.row_span)
            .max()
            .unwrap_or(0);
        let mut columns = vec![0.0_f64; self.columns];
        let mut rows = vec![0.0_f64; row_count];
        let mut grab_columns = vec![false; self.columns];
        let mut grab_rows = vec![false; row_count];

        for placement in placements {
            let size = if minimum {
                placement.minimum
            } else {
                placement.preferred
            };
            if placement.column_span == 1 {
                columns[placement.column] = columns[placement.column].max(size.0);
            }
            if placement.row_span == 1 {
                rows[placement.row] = rows[placement.row].max(size.1);
            }
            if placement.constraint.grab_horizontal {
                for grab in grab_columns
                    .iter_mut()
                    .skip(placement.column)
                    .take(placement.column_span)
                {
                    *grab = true;
                }
            }
            if placement.constraint.grab_vertical {
                for grab in grab_rows
                    .iter_mut()
                    .skip(placement.row)
                    .take(placement.row_span)
                {
                    *grab = true;
                }
            }
        }

        for placement in placements {
            let size = if minimum {
                placement.minimum
            } else {
                placement.preferred
            };
            ensure_span_size(
                &mut columns,
                placement.column,
                placement.column_span,
                self.horizontal_spacing,
                size.0,
            );
            ensure_span_size(
                &mut rows,
                placement.row,
                placement.row_span,
                self.vertical_spacing,
                size.1,
            );
        }

        if self.equal_column_widths {
            let width = columns.iter().copied().fold(0.0_f64, f64::max);
            columns.fill(width);
        }

        (columns, rows, grab_columns, grab_rows)
    }

    fn measured_size(
        &self,
        container: BlockId,
        ctx: &dyn LayoutContext,
        minimum: bool,
    ) -> (f64, f64) {
        let placements = self.placements(container, ctx);
        let (columns, rows, _, _) = self.track_sizes(&placements, minimum);
        (
            columns.iter().sum::<f64>()
                + self.horizontal_spacing * columns.len().saturating_sub(1) as f64
                + self.margin_width * 2.0,
            rows.iter().sum::<f64>()
                + self.vertical_spacing * rows.len().saturating_sub(1) as f64
                + self.margin_height * 2.0,
        )
    }
}

impl Default for GridLayout {
    fn default() -> Self {
        Self::new(1)
    }
}

impl LayoutManager for GridLayout {
    fn get_preferred_size(
        &self,
        container: BlockId,
        _w_hint: f64,
        _h_hint: f64,
        ctx: &dyn LayoutContext,
    ) -> (f64, f64) {
        self.measured_size(container, ctx, false)
    }

    fn get_minimum_size(
        &self,
        container: BlockId,
        _w_hint: f64,
        _h_hint: f64,
        ctx: &dyn LayoutContext,
    ) -> (f64, f64) {
        self.measured_size(container, ctx, true)
    }

    fn layout(&self, container: BlockId, ctx: &mut dyn LayoutContext) {
        let placements = self.placements(container, ctx);
        if placements.is_empty() {
            return;
        }
        let (mut columns, mut rows, grab_columns, grab_rows) = self.track_sizes(&placements, false);
        let client = ctx.get_container_bounds(container);
        let available_width = (client.width
            - self.margin_width * 2.0
            - self.horizontal_spacing * columns.len().saturating_sub(1) as f64)
            .max(0.0);
        let available_height = (client.height
            - self.margin_height * 2.0
            - self.vertical_spacing * rows.len().saturating_sub(1) as f64)
            .max(0.0);
        distribute_extra(&mut columns, &grab_columns, available_width);
        distribute_extra(&mut rows, &grab_rows, available_height);

        let column_positions = track_positions(
            client.x + self.margin_width,
            &columns,
            self.horizontal_spacing,
        );
        let row_positions =
            track_positions(client.y + self.margin_height, &rows, self.vertical_spacing);

        for placement in placements {
            let cell_width = span_size(
                &columns,
                placement.column,
                placement.column_span,
                self.horizontal_spacing,
            );
            let cell_height = span_size(
                &rows,
                placement.row,
                placement.row_span,
                self.vertical_spacing,
            );
            let indent = placement
                .constraint
                .horizontal_indent
                .max(0.0)
                .min(cell_width);
            let available_child_width = (cell_width - indent).max(0.0);
            let child_width = aligned_size(
                placement.constraint.horizontal_alignment,
                placement.preferred.0 - indent,
                placement.minimum.0 - indent,
                available_child_width,
            );
            let child_height = aligned_size(
                placement.constraint.vertical_alignment,
                placement.preferred.1,
                placement.minimum.1,
                cell_height,
            );
            let x = column_positions[placement.column]
                + indent
                + alignment_offset(
                    placement.constraint.horizontal_alignment,
                    available_child_width,
                    child_width,
                );
            let y = row_positions[placement.row]
                + alignment_offset(
                    placement.constraint.vertical_alignment,
                    cell_height,
                    child_height,
                );
            ctx.set_child_bounds(
                placement.child,
                Rectangle::new(x, y, child_width, child_height),
            );
        }
    }
}

fn span_size(tracks: &[f64], start: usize, span: usize, spacing: f64) -> f64 {
    tracks.iter().skip(start).take(span).sum::<f64>() + spacing * span.saturating_sub(1) as f64
}

fn ensure_span_size(tracks: &mut [f64], start: usize, span: usize, spacing: f64, requested: f64) {
    let current = span_size(tracks, start, span, spacing);
    let deficit = (requested - current).max(0.0);
    if deficit == 0.0 {
        return;
    }
    let per_track = deficit / span as f64;
    for track in tracks.iter_mut().skip(start).take(span) {
        *track += per_track;
    }
}

fn distribute_extra(tracks: &mut [f64], grab: &[bool], available: f64) {
    let extra = available - tracks.iter().sum::<f64>();
    let grab_count = grab.iter().filter(|grab| **grab).count();
    if extra <= 0.0 || grab_count == 0 {
        return;
    }
    let share = extra / grab_count as f64;
    for (track, grab) in tracks.iter_mut().zip(grab) {
        if *grab {
            *track += share;
        }
    }
}

fn track_positions(origin: f64, tracks: &[f64], spacing: f64) -> Vec<f64> {
    let mut cursor = origin;
    tracks
        .iter()
        .map(|track| {
            let position = cursor;
            cursor += *track + spacing;
            position
        })
        .collect()
}

fn aligned_size(alignment: GridAlignment, preferred: f64, minimum: f64, available: f64) -> f64 {
    if alignment == GridAlignment::Fill {
        available
    } else {
        preferred.max(minimum).min(available)
    }
}

fn alignment_offset(alignment: GridAlignment, available: f64, size: f64) -> f64 {
    match alignment {
        GridAlignment::Start | GridAlignment::Fill => 0.0,
        GridAlignment::Center => (available - size) / 2.0,
        GridAlignment::End => available - size,
    }
    .max(0.0)
}
