use std::{error::Error, fmt};
use winit::dpi::{LogicalPosition, PhysicalPosition, PhysicalSize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanvasError {
    InvalidGeometry,
    InvalidScaleFactor,
    UnknownCanvas,
    Unavailable,
    InvalidRedrawToken,
    IdentityExhausted,
    RevisionExhausted,
    TextureTooLarge,
}

impl fmt::Display for CanvasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "canvas error: {self:?}")
    }
}
impl Error for CanvasError {}

/// A rectangle in window logical coordinates. The right/bottom edges are exclusive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasRect {
    origin: LogicalPosition<f64>,
    width: f64,
    height: f64,
}

impl CanvasRect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Result<Self, CanvasError> {
        if ![x, y, width, height, x + width, y + height]
            .iter()
            .all(|value| value.is_finite())
            || width < 0.0
            || height < 0.0
        {
            return Err(CanvasError::InvalidGeometry);
        }
        Ok(Self {
            origin: LogicalPosition::new(x, y),
            width,
            height,
        })
    }
    pub fn origin(self) -> LogicalPosition<f64> {
        self.origin
    }
    pub fn width(self) -> f64 {
        self.width
    }
    pub fn height(self) -> f64 {
        self.height
    }
    pub fn is_empty(self) -> bool {
        self.width == 0.0 || self.height == 0.0
    }
    pub fn contains(self, point: LogicalPosition<f64>) -> bool {
        point.x >= self.origin.x
            && point.y >= self.origin.y
            && point.x < self.origin.x + self.width
            && point.y < self.origin.y + self.height
    }
    pub fn window_to_local(self, point: LogicalPosition<f64>) -> LogicalPosition<f64> {
        LogicalPosition::new(point.x - self.origin.x, point.y - self.origin.y)
    }
    pub fn local_to_window(self, point: LogicalPosition<f64>) -> LogicalPosition<f64> {
        LogicalPosition::new(point.x + self.origin.x, point.y + self.origin.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowMetrics {
    pixels: PhysicalSize<u32>,
    scale_factor: f64,
}

impl WindowMetrics {
    pub fn new(pixels: PhysicalSize<u32>, scale_factor: f64) -> Result<Self, CanvasError> {
        if !scale_factor.is_finite()
            || scale_factor <= 0.0
            || !(f64::from(pixels.width) / scale_factor).is_finite()
            || !(f64::from(pixels.height) / scale_factor).is_finite()
        {
            return Err(CanvasError::InvalidScaleFactor);
        }
        Ok(Self {
            pixels,
            scale_factor,
        })
    }
    pub fn pixels(self) -> PhysicalSize<u32> {
        self.pixels
    }
    pub fn scale_factor(self) -> f64 {
        self.scale_factor
    }
    pub fn physical_to_logical(
        self,
        point: PhysicalPosition<f64>,
    ) -> Result<LogicalPosition<f64>, CanvasError> {
        let logical =
            LogicalPosition::new(point.x / self.scale_factor, point.y / self.scale_factor);
        if !logical.x.is_finite() || !logical.y.is_finite() {
            return Err(CanvasError::InvalidGeometry);
        }
        Ok(logical)
    }
    pub fn texture_size(self, rect: CanvasRect) -> Result<PhysicalSize<u32>, CanvasError> {
        let width = (rect.width * self.scale_factor).ceil();
        let height = (rect.height * self.scale_factor).ceil();
        if !width.is_finite()
            || !height.is_finite()
            || width > f64::from(u32::MAX)
            || height > f64::from(u32::MAX)
        {
            return Err(CanvasError::TextureTooLarge);
        }
        Ok(PhysicalSize::new(width as u32, height as u32))
    }
}
