use std::{any::Any, sync::Arc};

use super::{CapabilityKey, Figure};
use crate::{
    Rectangle,
    graphics::{GraphicsError, PaintContext},
};

/// Immutable, size-dependent self-paint inputs with a finite recording bound.
///
/// Application inputs are `Copy`; owned recordings and compositions use explicit storage
/// accounting. Callbacks are plain function pointers and cannot retain opaque captured heaps.
#[derive(Clone)]
pub struct BoundsDrawing {
    payload: Arc<dyn Any + Send + Sync>,
    paint: fn(&dyn Any, Rectangle, &mut PaintContext<'_>) -> Result<(), GraphicsError>,
    envelope: fn(&dyn Any, Rectangle) -> Result<Rectangle, GraphicsError>,
    recording_bytes: usize,
    payload_bytes: usize,
    resource_inputs: Vec<(crate::render::ResourceId, u64)>,
    resources: Vec<crate::render::ResourceUpdate>,
}

impl BoundsDrawing {
    /// Freezes inline inputs. `recording_bytes` bounds commands and their owned heaps.
    pub fn new<D: Copy + Send + Sync + 'static>(
        inputs: D,
        recording_bytes: usize,
        paint: fn(&D, Rectangle, &mut PaintContext<'_>) -> Result<(), GraphicsError>,
        envelope: fn(&D, Rectangle) -> Result<Rectangle, GraphicsError>,
    ) -> Self {
        // Function pointers and the inline payload share one fixed-size allocation.
        struct Inputs<D> {
            value: D,
            paint: fn(&D, Rectangle, &mut PaintContext<'_>) -> Result<(), GraphicsError>,
            envelope: fn(&D, Rectangle) -> Result<Rectangle, GraphicsError>,
        }
        Self {
            payload: Arc::new(Inputs {
                value: inputs,
                paint,
                envelope,
            }),
            paint: |payload, bounds, context| {
                let inputs = payload.downcast_ref::<Inputs<D>>().expect("typed drawing");
                (inputs.paint)(&inputs.value, bounds, context)
            },
            envelope: |payload, bounds| {
                let inputs = payload.downcast_ref::<Inputs<D>>().expect("typed drawing");
                (inputs.envelope)(&inputs.value, bounds)
            },
            recording_bytes,
            payload_bytes: std::mem::size_of::<Inputs<D>>() + 2 * std::mem::size_of::<usize>(),
            resource_inputs: Vec::new(),
            resources: Vec::new(),
        }
    }

    /// Replays an owned recording at a size-dependent placement without reshaping its content.
    /// Only inline placement inputs and immutable commands are retained.
    pub fn recorded<D: Copy + Send + Sync + 'static>(
        inputs: D,
        drawing: crate::render::RecordedDrawing,
        placement: fn(&D, Rectangle) -> Result<crate::Affine2D, GraphicsError>,
        envelope: fn(&D, Rectangle) -> Result<Rectangle, GraphicsError>,
    ) -> Result<Self, GraphicsError> {
        struct Recording<D> {
            inputs: D,
            drawing: crate::render::RecordedDrawing,
            placement: fn(&D, Rectangle) -> Result<crate::Affine2D, GraphicsError>,
            envelope: fn(&D, Rectangle) -> Result<Rectangle, GraphicsError>,
        }
        let owned_bytes = drawing
            .retained_bytes(&mut std::collections::HashSet::new())
            .ok_or(GraphicsError::ImmutableDrawingUnavailable)?;
        let recording_bytes = owned_bytes
            .checked_add(8 * std::mem::size_of::<crate::render::RenderCommand>())
            .ok_or(GraphicsError::ImmutableDrawingUnavailable)?;
        let mut resource_inputs: Vec<_> = drawing
            .resources()
            .ready
            .iter()
            .map(|resource| (resource.id, resource.revision))
            .collect();
        for command in drawing.commands() {
            match &command.kind {
                crate::render::RenderCommandKind::DrawGlyphRun { run, .. } => {
                    resource_inputs.push((run.font.resource_id(), run.font.revision()));
                }
                crate::render::RenderCommandKind::Image { image, .. } => {
                    resource_inputs.push((image.resource_id(), image.revision()));
                }
                _ => {}
            }
        }
        let payload_bytes = owned_bytes
            .checked_add(std::mem::size_of::<Recording<D>>() + 2 * std::mem::size_of::<usize>())
            .and_then(|bytes| {
                resource_inputs
                    .capacity()
                    .checked_mul(std::mem::size_of::<(crate::render::ResourceId, u64)>())
                    .and_then(|inputs| bytes.checked_add(inputs))
            })
            .ok_or(GraphicsError::ImmutableDrawingUnavailable)?;
        Ok(Self {
            payload: Arc::new(Recording {
                inputs,
                drawing,
                placement,
                envelope,
            }),
            paint: |payload, bounds, context| {
                let recording = payload
                    .downcast_ref::<Recording<D>>()
                    .expect("typed recording");
                let transform = (recording.placement)(&recording.inputs, bounds)?;
                context.push_state();
                let result = context
                    .concat_transform(transform)
                    .and_then(|()| context.replay(&recording.drawing));
                let restored = context.pop_state();
                result.and(restored)
            },
            envelope: |payload, bounds| {
                let recording = payload
                    .downcast_ref::<Recording<D>>()
                    .expect("typed recording");
                (recording.envelope)(&recording.inputs, bounds)
            },
            recording_bytes,
            payload_bytes,
            resource_inputs,
            resources: Vec::new(),
        })
    }

    pub(crate) fn freeze_resources(
        &mut self,
        registry: &crate::runtime::ResourceRegistry,
        retained: &mut std::collections::HashSet<(crate::render::ResourceId, u64)>,
    ) -> Result<(), GraphicsError> {
        if self.resource_inputs.is_empty() {
            return Ok(());
        }
        let mut resources = Vec::new();
        for (id, revision) in &self.resource_inputs {
            if let Some(previous) = resources
                .iter()
                .find(|resource: &&crate::render::ResourceUpdate| resource.id == *id)
            {
                if previous.revision != *revision {
                    return Err(GraphicsError::ConflictingResource(*id));
                }
                continue;
            }
            let resource = registry
                .snapshot_resource(*id)
                .map_err(GraphicsError::Resource)?;
            if resource.revision != *revision {
                return Err(GraphicsError::ConflictingResource(*id));
            }
            retained.insert((*id, *revision));
            resources.push(resource);
        }
        let leases = crate::render::RecordedDrawing {
            commands: Vec::new(),
            resources: crate::render::ResourceSnapshot { ready: resources },
        };
        let bytes = leases
            .retained_bytes(&mut std::collections::HashSet::new())
            .ok_or(GraphicsError::ImmutableDrawingUnavailable)?;
        self.payload_bytes = self
            .payload_bytes
            .checked_add(bytes)
            .ok_or(GraphicsError::ImmutableDrawingUnavailable)?;
        self.recording_bytes = self
            .recording_bytes
            .checked_add(bytes)
            .ok_or(GraphicsError::ImmutableDrawingUnavailable)?;
        self.resources = leases.resources.ready;
        Ok(())
    }

    /// Combines immutable painters inside explicit inset rectangles.
    ///
    /// Storage and recording bounds include every child. Each child has its own graphics scope.
    pub fn compose(parts: Vec<(crate::Insets, BoundsDrawing)>) -> Result<Self, GraphicsError> {
        struct Parts(Vec<(crate::Insets, BoundsDrawing)>);
        let mut recording_bytes = 0usize;
        let mut resource_inputs = Vec::new();
        let mut payload_bytes = parts
            .capacity()
            .checked_mul(std::mem::size_of::<(crate::Insets, BoundsDrawing)>())
            .and_then(|bytes| {
                bytes.checked_add(std::mem::size_of::<Parts>() + 2 * std::mem::size_of::<usize>())
            })
            .ok_or(GraphicsError::ImmutableDrawingUnavailable)?;
        for (_, drawing) in &parts {
            resource_inputs.extend(drawing.resource_inputs.iter().copied());
            recording_bytes = recording_bytes
                .checked_add(drawing.recording_bytes)
                .and_then(|bytes| {
                    bytes.checked_add(2 * std::mem::size_of::<crate::render::RenderCommand>())
                })
                .ok_or(GraphicsError::ImmutableDrawingUnavailable)?;
            payload_bytes = payload_bytes
                .checked_add(drawing.retained_bytes())
                .ok_or(GraphicsError::ImmutableDrawingUnavailable)?;
        }
        payload_bytes = resource_inputs
            .capacity()
            .checked_mul(std::mem::size_of::<(crate::render::ResourceId, u64)>())
            .and_then(|bytes| payload_bytes.checked_add(bytes))
            .ok_or(GraphicsError::ImmutableDrawingUnavailable)?;
        Ok(Self {
            payload: Arc::new(Parts(parts)),
            paint: |payload, bounds, context| {
                for (insets, drawing) in &payload.downcast_ref::<Parts>().expect("drawing parts").0
                {
                    context.push_state();
                    let result =
                        drawing.paint(super::border::inset_rectangle(bounds, *insets), context);
                    let restored = context.pop_state();
                    result?;
                    restored?;
                }
                Ok(())
            },
            envelope: |payload, bounds| {
                let mut envelope = bounds;
                for (insets, drawing) in &payload.downcast_ref::<Parts>().expect("drawing parts").0
                {
                    envelope = envelope.union(
                        drawing.visual_bounds(super::border::inset_rectangle(bounds, *insets))?,
                    );
                }
                Ok(envelope)
            },
            recording_bytes,
            payload_bytes,
            resource_inputs,
            resources: Vec::new(),
        })
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.payload_bytes
    }

    pub(crate) fn recording_bytes(&self) -> usize {
        self.recording_bytes
    }

    pub(crate) fn visual_bounds(&self, bounds: Rectangle) -> Result<Rectangle, GraphicsError> {
        (self.envelope)(self.payload.as_ref(), bounds)
    }

    pub(crate) fn paint(
        &self,
        bounds: Rectangle,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        (self.paint)(self.payload.as_ref(), bounds, context)
    }

    pub(crate) fn materialize(
        &self,
        bounds: Rectangle,
        foreground: crate::Color,
        background: crate::Color,
    ) -> Result<super::FigurePresentation, GraphicsError> {
        let mut recorder = crate::render::CommandRecorder::new();
        recorder
            .canvas
            .limit_recording_storage(self.recording_bytes);
        recorder.canvas.set_foreground_color(foreground);
        recorder.canvas.set_background_color(background);
        recorder.leases.extend(self.resources.iter().cloned());
        {
            let mut context =
                PaintContext::for_recorder(&mut recorder.canvas, None, Some(&mut recorder.leases));
            self.paint(bounds, &mut context)?;
        }
        let envelope = self.visual_bounds(bounds)?;
        let drawing = recorder.finish()?;
        super::FigurePresentation::new(
            super::FigureMeasurement::new(bounds.width, bounds.height, None),
            crate::Dimension::new(0.0, 0.0),
            envelope,
            Arc::new(RecordedBoundsDrawing(drawing)),
        )
        .map_err(|_| GraphicsError::ImmutableDrawingUnavailable)
    }
}

struct RecordedBoundsDrawing(crate::render::RecordedDrawing);

impl super::FigureDrawing for RecordedBoundsDrawing {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        context.replay(&self.0)
    }
}

/// Freezes a Figure whose self-paint follows sampled size without scaling its strokes.
pub trait FigureBoundsSnapshot {
    /// Revision of externally owned paint inputs; attached changes also follow Runtime mutation.
    fn bounds_drawing_revision(&self) -> u64 {
        0
    }

    fn snapshot_bounds_drawing(&self) -> Result<BoundsDrawing, GraphicsError>;
}

/// Typed registration adapter shared by built-in and external Figures.
#[derive(Clone, Copy)]
pub struct BoundsSnapshotCapability {
    resolve: for<'a> fn(&'a dyn Figure) -> Option<&'a dyn FigureBoundsSnapshot>,
}

impl BoundsSnapshotCapability {
    pub fn of<T: Figure + FigureBoundsSnapshot + 'static>() -> Self {
        Self {
            resolve: |figure| {
                figure
                    .as_any()
                    .downcast_ref::<T>()
                    .map(|figure| figure as &dyn FigureBoundsSnapshot)
            },
        }
    }

    pub(crate) fn resolve(self, figure: &dyn Figure) -> Option<&dyn FigureBoundsSnapshot> {
        (self.resolve)(figure)
    }
}

pub const BOUNDS_SNAPSHOT: CapabilityKey<BoundsSnapshotCapability> =
    CapabilityKey::new("novadraw.bounds-snapshot");
