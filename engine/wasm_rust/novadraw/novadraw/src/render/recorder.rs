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
            commands: self.canvas.commands().to_vec(),
            resources: ResourceSnapshot { ready: self.leases },
        })
    }
}

/// Frozen commands and resource leases for an explicit snapshot replay session.
/// A bare command slice does not retain fonts. This value does not submit frames.
#[derive(Clone, Debug)]
pub struct RecordedDrawing {
    commands: Vec<RenderCommand>,
    resources: ResourceSnapshot,
}

impl RecordedDrawing {
    pub fn commands(&self) -> &[RenderCommand] {
        &self.commands
    }
    pub fn resources(&self) -> &ResourceSnapshot {
        &self.resources
    }
}
