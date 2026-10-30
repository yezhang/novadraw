//! Zoom's UML row composition through the public preparation protocol.
use std::sync::Arc;

use novadraw::{
    figure::{
        CONTAINER, ContainerCapability, Figure, FigureCapabilityBuilder,
        FigureCapabilityRegistrationError, FigureContainer, FigureDrawing, FigureMeasurement,
        FigurePreparation, FigurePresentation, MeasureConstraints, PREPARATION,
        PreparationCapability, PreparationMeasureError, PreparationMeasureErrorKind,
        PreparationMeasurement, PreparationScene, PreparedScene,
        border::{Border, MarginBorder},
    },
    geometry::{Dimension, Insets, Point, Rectangle},
    graphics::{Color, GraphicsError, PaintContext},
    layout::LayoutOutput,
    text::{MeasureContext, StyledTextRequest, TextConstraints, TextError, TextLayout, TextStyle},
    tree::FigureId,
};

const GAP: f64 = 4.0;
const SOURCE_SIZE: f64 = 16.0;
const PADDING: Insets = Insets {
    top: 2.0,
    left: 4.0,
    bottom: 2.0,
    right: 4.0,
};

// Visible alpha >= 128 bounds of the original SVGs, measured at density 16.
pub(super) const ICON_INK: [Rectangle; 3] = [
    Rectangle {
        x: 1.0,
        y: 0.0,
        width: 13.0,
        height: 13.0625,
    },
    Rectangle {
        x: 6.0,
        y: 6.0,
        width: 5.0,
        height: 5.0,
    },
    Rectangle {
        x: 4.0,
        y: 5.0,
        width: 8.0625,
        height: 8.0625,
    },
];

#[derive(Clone, Copy)]
pub(super) enum Anchor {
    Baseline,
    CapitalCenter,
}

pub(super) struct Row {
    text: String,
    ink: Rectangle,
    anchor: Anchor,
    border: MarginBorder,
}

impl Row {
    fn layout_text(context: &mut MeasureContext<'_>, text: &str) -> Result<TextLayout, TextError> {
        let request = StyledTextRequest::new(
            text,
            TextStyle::new(context.font().clone(), Color::BLACK),
            Vec::new(),
            Vec::new(),
            TextConstraints::UNBOUNDED,
        )?;
        // Reuses OutlineTextEngine's bounded, revision-aware shaping cache.
        Ok(context.layout_styled(&request)?.layout().clone())
    }

    pub fn new(text: &str, ink: Rectangle, anchor: Anchor) -> Self {
        Self {
            text: text.to_owned(),
            ink,
            anchor,
            border: MarginBorder::new(PADDING).expect("fixed row padding"),
        }
    }
}

impl FigureContainer for Row {}
impl Figure for Row {
    fn name(&self) -> &'static str {
        "ZoomTypographicRow"
    }
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }
    fn initial_insets(&self) -> Insets {
        PADDING
    }
    fn get_border(&self) -> Option<&dyn Border> {
        Some(&self.border)
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())?;
        out.register(PREPARATION, PreparationCapability::of::<Self>())
    }
}

#[derive(Clone, Debug)]
pub(super) struct Geometry {
    pub text: Point,
    pub icon: Rectangle,
    pub size: Dimension,
    pub baseline: f64,
    pub capital_center: f64,
}

fn geometry(
    text: &TextLayout,
    capital: &TextLayout,
    image: Dimension,
    ink: Rectangle,
    anchor: Anchor,
) -> Result<Geometry, TextError> {
    let baseline = f64::from(text.baseline());
    let cap_ink = capital.ink_bounds().ok_or(TextError::InvalidMetric)?;
    let capital_center =
        baseline - f64::from(capital.baseline()) + cap_ink.y + cap_ink.height / 2.0;
    let ink_top = ink.y * image.height / SOURCE_SIZE;
    let ink_height = ink.height * image.height / SOURCE_SIZE;
    let icon_y = match anchor {
        Anchor::Baseline => baseline - ink_top - ink_height,
        Anchor::CapitalCenter => capital_center - ink_top - ink_height / 2.0,
    };
    // Reserve the entire image box, including transparent padding.
    let top = icon_y.min(0.0);
    let height = f64::from(text.height()).max(icon_y + image.height) - top;
    Ok(Geometry {
        text: Point::new(image.width + GAP, -top),
        icon: Rectangle::new(0.0, icon_y - top, image.width, image.height),
        size: Dimension::new(image.width + GAP + f64::from(text.width()), height),
        baseline: baseline - top,
        capital_center: capital_center - top,
    })
}

struct Drawing {
    text: TextLayout,
    origin: Point,
}
impl FigureDrawing for Drawing {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        if let Some(foreground) = context.stroke_paint().cloned() {
            context.set_fill_paint(foreground);
        }
        context.fill_text(&self.text, self.origin)
    }
}

impl Row {
    fn measured(
        &self,
        context: &mut MeasureContext<'_>,
        scene: &PreparationScene<'_>,
    ) -> Result<(FigureId, TextLayout, Geometry), TextError> {
        let children = scene.children(scene.root())?;
        let [image] = children.as_slice() else {
            return Err(TextError::InvalidMetric);
        };
        let size = scene
            .measure_figure(context, *image, MeasureConstraints::UNBOUNDED)
            .map_err(|error| TextError::PreparationMeasurement(Box::new(error)))?
            .preferred
            .size();
        let text = Self::layout_text(context, &self.text)?;
        // H has no descender. The same resolved descriptor and engine measure both.
        let capital = Self::layout_text(context, "H")?;
        let geometry = geometry(&text, &capital, size, self.ink, self.anchor)?;
        Ok((*image, text, geometry))
    }
}

impl FigurePreparation for Row {
    fn measure_scene(
        &self,
        context: &mut MeasureContext<'_>,
        constraints: MeasureConstraints,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparationMeasurement, PreparationMeasureError> {
        let (_, _, geometry) =
            self.measured(context, scene)
                .map_err(|error| PreparationMeasureError {
                    figure: scene.root(),
                    outer_constraints: constraints,
                    content_constraints: Some(constraints),
                    kind: PreparationMeasureErrorKind::Text(Box::new(error)),
                })?;
        Ok(PreparationMeasurement {
            preferred: FigureMeasurement::new(
                geometry.size.width,
                geometry.size.height,
                Some(geometry.baseline),
            ),
            // These official rows keep their full text, as the reference compartments do.
            minimum: geometry.size,
        })
    }

    fn prepare(
        &self,
        _: &mut MeasureContext<'_>,
        _: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        // An image child must be read from the frozen scene, never an external cache.
        Err(TextError::InvalidMetric)
    }

    fn prepare_scene(
        &self,
        context: &mut MeasureContext<'_>,
        bounds: Rectangle,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparedScene, TextError> {
        let (image, text, mut geometry) = self.measured(context, scene)?;
        let x = match self.anchor {
            Anchor::CapitalCenter => (bounds.width - geometry.size.width).max(0.0) / 2.0,
            Anchor::Baseline => 0.0,
        };
        geometry.text = Point::new(
            bounds.x + x + geometry.text.x(),
            bounds.y + geometry.text.y(),
        );
        // Child placements are client-local; Runtime applies the row's Insets.
        // Root drawing origins are node-local and therefore include bounds.x/y.
        geometry.icon.x += x;
        geometry.baseline += bounds.y;
        geometry.capital_center += bounds.y;
        let envelope = Rectangle::new(
            bounds.x + x,
            bounds.y,
            geometry.size.width,
            geometry.size.height,
        );
        let presentation = FigurePresentation::new(
            FigureMeasurement::new(geometry.size.width, geometry.size.height, None),
            geometry.size,
            envelope,
            Arc::new(Drawing {
                text,
                origin: geometry.text,
            }),
        )?
        .with_data(Arc::new(geometry.clone()));
        let mut prepared = PreparedScene::new(presentation);
        let mut layout = LayoutOutput::new();
        layout.set_child_bounds(image, geometry.icon);
        prepared.layout(scene.root(), layout);
        Ok(prepared)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::text::{BuiltinFont, FontDescriptor, TextSystem};

    #[test]
    fn visible_anchors_follow_font_metrics_and_keep_full_image_inside_row() {
        let mut system =
            TextSystem::with_engine(Box::new(novadraw::text::outline::OutlineTextEngine::new(
                Box::new(novadraw::text::shaping::ParleyTextEngine::new()),
                Box::new(novadraw::text::outline::SkrifaOutlineProvider),
            )));
        system.register_builtin_font(BuiltinFont::Inter).unwrap();
        for size in [8.0, 12.0, 24.0] {
            let mut context =
                system.measure(FontDescriptor::new(BuiltinFont::Inter.family(), size).unwrap());
            let capital = context
                .layout_text("H", TextConstraints::UNBOUNDED)
                .unwrap();
            for source in ["Table", "columns: Column[]", "getRows(): Row[]", "gyp"] {
                let text = context
                    .layout_text(source, TextConstraints::UNBOUNDED)
                    .unwrap();
                for (index, ink) in ICON_INK.into_iter().enumerate() {
                    let anchor = if index == 0 {
                        Anchor::CapitalCenter
                    } else {
                        Anchor::Baseline
                    };
                    let g =
                        geometry(&text, &capital, Dimension::new(16.0, 16.0), ink, anchor).unwrap();
                    let actual = g.icon.y
                        + ink.y
                        + match anchor {
                            Anchor::Baseline => ink.height,
                            Anchor::CapitalCenter => ink.height / 2.0,
                        };
                    let expected = match anchor {
                        Anchor::Baseline => g.baseline,
                        Anchor::CapitalCenter => g.capital_center,
                    };
                    assert!((actual - expected).abs() < 1e-9);
                    assert!(g.icon.y >= 0.0 && g.icon.y + g.icon.height <= g.size.height);
                    assert!(
                        g.text.y() >= 0.0 && g.text.y() + f64::from(text.height()) <= g.size.height
                    );
                }
            }
        }
    }
}
