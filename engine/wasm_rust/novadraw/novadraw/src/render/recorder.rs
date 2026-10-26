use super::{NdCanvas, RenderCommand, RenderCommandKind, ResourceSnapshot, ResourceUpdate};
use crate::graphics::GraphicsError;

/// Owns standalone command storage and the exact resources used by its Graphics.
#[derive(Default)]
pub struct CommandRecorder {
    pub(crate) canvas: NdCanvas,
    pub(crate) leases: Vec<ResourceUpdate>,
}

impl CommandRecorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Borrows the recorder as a drawing-only context.
    pub fn paint(&mut self) -> crate::graphics::PaintContext<'_> {
        crate::graphics::PaintContext::for_recorder(&mut self.canvas, None, None)
    }

    pub fn commands(&self) -> &[RenderCommand] {
        self.canvas.commands()
    }

    pub fn finish(self) -> Result<RecordedDrawing, GraphicsError> {
        if let Some(error) = self.canvas.recording_error() {
            return Err(error.clone());
        }
        let mut depth = 0_usize;
        for command in self.canvas.commands() {
            match command.kind {
                RenderCommandKind::PushState => depth += 1,
                RenderCommandKind::RestoreState if depth == 0 => {
                    return Err(GraphicsError::UnbalancedState);
                }
                RenderCommandKind::PopState => {
                    depth = depth.checked_sub(1).ok_or(GraphicsError::UnbalancedState)?;
                }
                _ => {}
            }
        }
        if depth != 0 {
            return Err(GraphicsError::UnbalancedState);
        }
        Ok(RecordedDrawing {
            commands: self.canvas.into_commands(),
            resources: ResourceSnapshot { ready: self.leases },
        })
    }
}

/// Frozen commands and resource leases for an explicit snapshot replay session.
/// A bare command slice does not retain fonts. This value does not submit frames.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordedDrawing {
    pub(crate) commands: Vec<RenderCommand>,
    pub(crate) resources: ResourceSnapshot,
}

impl RecordedDrawing {
    pub(crate) fn command_bytes(command: &RenderCommand) -> Option<usize> {
        use super::{DashPattern, Paint};
        let mut bytes = std::mem::size_of::<RenderCommand>();
        let mut add = |count: usize, item: usize| -> Option<()> {
            bytes = bytes.checked_add(count.checked_mul(item)?)?;
            Some(())
        };
        for paint in command.kind.paints() {
            if let Paint::LinearGradient(gradient) = paint {
                add(
                    gradient.stops().len(),
                    std::mem::size_of::<super::GradientStop>(),
                )?;
            }
        }
        if let Some(stroke) = command.kind.stroke()
            && let DashPattern::Custom(dash) = stroke.dash_pattern()
        {
            add(dash.lengths().len(), std::mem::size_of::<f64>())?;
        }
        match &command.kind {
            RenderCommandKind::Polyline { points, .. } => {
                add(points.len(), std::mem::size_of::<crate::Point>())?;
            }
            RenderCommandKind::FillPath { path, .. }
            | RenderCommandKind::StrokePath { path, .. } => {
                add(
                    path.operations().len(),
                    std::mem::size_of::<super::command::PathOp>(),
                )?;
            }
            RenderCommandKind::ClipPath { clip } => {
                add(
                    clip.path().operations().len(),
                    std::mem::size_of::<super::command::PathOp>(),
                )?;
            }
            RenderCommandKind::DrawGlyphRun { run, .. } => {
                add(
                    run.glyphs.capacity(),
                    std::mem::size_of::<super::PositionedGlyph>(),
                )?;
                add(run.normalized_coords.capacity(), std::mem::size_of::<i16>())?;
            }
            _ => {}
        }
        Some(bytes)
    }

    /// Conservative owned storage, including exact resource revisions retained by replay.
    /// Shared allocations are charged to every independent retention responsibility.
    pub(crate) fn retained_bytes(
        &self,
        retained_resources: &mut std::collections::HashSet<(super::ResourceId, u64)>,
    ) -> Option<usize> {
        use super::ResourcePayload;
        let mut bytes = std::mem::size_of::<Self>();
        let mut add = |count: usize, item: usize| -> Option<()> {
            bytes = bytes.checked_add(count.checked_mul(item)?)?;
            Some(())
        };
        add(
            self.commands.capacity(),
            std::mem::size_of::<RenderCommand>(),
        )?;
        add(
            self.resources.ready.capacity(),
            std::mem::size_of::<ResourceUpdate>(),
        )?;
        for command in &self.commands {
            add(
                1,
                Self::command_bytes(command)?.checked_sub(std::mem::size_of::<RenderCommand>())?,
            )?;
        }
        for resource in &self.resources.ready {
            if !retained_resources.insert((resource.id, resource.revision)) {
                continue;
            }
            match &resource.payload {
                ResourcePayload::Image(image) => {
                    add(1, std::mem::size_of::<super::ImageData>())?;
                    add(image.pixels.capacity(), 1)?;
                }
                ResourcePayload::Font(font) => {
                    add(1, std::mem::size_of::<super::FontData>())?;
                    add(font.shared_bytes().capacity(), 1)?;
                }
            }
        }
        Some(bytes)
    }

    pub fn commands(&self) -> &[RenderCommand] {
        &self.commands
    }
    pub fn resources(&self) -> &ResourceSnapshot {
        &self.resources
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_recorder_latches_rejection_even_when_extension_ignores_it() {
        let mut recorder = CommandRecorder::new();
        let limit = std::mem::size_of::<RenderCommand>();
        recorder.canvas.limit_recording_storage(limit);
        let mut context = recorder.paint();
        context.set_stroke_paint(crate::Color::BLACK);
        let start = crate::Point::new(0.0, 0.0);
        let end = crate::Point::new(1.0, 1.0);
        context.stroke_line(start, end).unwrap();
        assert!(context.stroke_line(start, end).is_err());
        // A callback cannot clear the latched failure by issuing further commands.
        let _ = context.stroke_line(start, end);
        drop(context);
        assert_eq!(recorder.commands().len(), 1);
        assert!(matches!(
            recorder.finish(),
            Err(GraphicsError::RecordingBudgetExceeded { .. })
        ));
    }
}
