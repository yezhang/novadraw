use crate::Color;
use crate::geometry::{Dimension, Rectangle};
use crate::graphics::{GraphicsError, PaintContext};
use crate::render::ImageResourceRef;

use crate::{Alignment, ImageId, ResourceStatus};

use super::{Bounded, Figure};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageDisplayState {
    Pending,
    Ready,
    Failed,
    Unavailable,
}

#[derive(Clone)]
pub struct ImageFigure {
    bounds: Rectangle,
    image: ImageId,
    alignment: Alignment,
    state: ImageDisplayState,
    resource: Option<ImageResourceRef>,
}

impl ImageFigure {
    pub fn new(image: ImageId) -> Self {
        Self {
            bounds: Rectangle::ZERO,
            image,
            alignment: Alignment::Center,
            state: ImageDisplayState::Pending,
            resource: None,
        }
    }

    pub fn with_bounds(mut self, bounds: Rectangle) -> Self {
        self.bounds = bounds;
        self
    }

    pub fn image(&self) -> ImageId {
        self.image
    }

    pub fn display_state(&self) -> ImageDisplayState {
        self.state
    }

    pub fn alignment(&self) -> Alignment {
        self.alignment
    }

    pub fn with_alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn set_alignment(&mut self, alignment: Alignment) {
        self.alignment = alignment;
    }

    pub fn set_image(&mut self, image: ImageId) {
        self.image = image;
        self.state = ImageDisplayState::Pending;
        self.resource = None;
    }

    pub(crate) fn refresh(
        &mut self,
        status: Option<&ResourceStatus>,
        resource: Option<ImageResourceRef>,
    ) -> bool {
        let next_state = match status {
            Some(ResourceStatus::Pending) => ImageDisplayState::Pending,
            Some(ResourceStatus::Ready { .. }) => ImageDisplayState::Ready,
            Some(ResourceStatus::Failed { .. }) => ImageDisplayState::Failed,
            None => ImageDisplayState::Unavailable,
        };
        let next_resource = (next_state == ImageDisplayState::Ready)
            .then_some(resource)
            .flatten();
        let changed = self.state != next_state || self.resource != next_resource;
        self.state = next_state;
        self.resource = next_resource;
        changed
    }

    fn size(&self) -> (f64, f64) {
        self.resource.map_or(
            (self.bounds.width, self.bounds.height),
            ImageResourceRef::logical_size,
        )
    }
}

impl Bounded for ImageFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "ImageFigure"
    }
}

impl super::FigureResourceDependencies for ImageFigure {
    fn resource_dependencies(&self) -> Vec<crate::render::ResourceId> {
        vec![self.image.resource_id()]
    }
}

impl Figure for ImageFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ImageFigure"
    }

    fn register_capabilities(
        &self,
        out: &mut super::FigureCapabilityBuilder,
    ) -> Result<(), super::FigureCapabilityRegistrationError> {
        out.register(
            super::RESOURCE_DEPENDENCIES,
            super::ResourceDependenciesCapability::of::<Self>(),
        )
    }

    fn intrinsic_size(&self) -> Dimension {
        self.size().into()
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        match (self.state, self.resource) {
            (ImageDisplayState::Ready, Some(image)) => {
                let (image_width, image_height) = image.logical_size();
                let x = aligned(bounds.width, image_width, self.alignment);
                let y = aligned(bounds.height, image_height, self.alignment);
                context.draw_image(image, Rectangle::new(x, y, image_width, image_height))?;
            }
            (ImageDisplayState::Pending, _) => {
                context.set_fill_paint(Color::rgba(0.72, 0.76, 0.82, 1.0));
                context.fill_rect(bounds)?;
            }
            (ImageDisplayState::Failed, _) => {
                context.set_fill_paint(Color::rgba(0.8, 0.1, 0.1, 1.0));
                context.fill_rect(bounds)?;
            }
            (ImageDisplayState::Unavailable, _) => {}
            _ => {}
        }
        Ok(())
    }
}

fn aligned(available: f64, used: f64, alignment: Alignment) -> f64 {
    match alignment {
        Alignment::Start => 0.0,
        Alignment::Center => (available - used).max(0.0) / 2.0,
        Alignment::End => (available - used).max(0.0),
    }
}
