//! Host background consumer for the official SWT InheritBackgroundExample.
use novadraw::{
    figure::{
        FigureStyle, RectangleFigure,
        flow::{FlowPageFigure, StructuredTextFigure},
    },
    geometry::{Point, Rectangle},
    graphics::{Color, GradientStop, LinearGradient},
    host::{
        HostBackgroundContent, HostBackgroundError, HostBackgroundProvider,
        HostBackgroundProviderKey, HostBackgroundRequest,
    },
    render::{BuiltinFont, CommandRecorder},
    runtime::Runtime,
    tree::FigureId,
};

pub const TEXT: &str = "This is a figure canvas\nwith a label.  The background\nis either painted by the root\nfigure, or by the operating system.\nNote that vertical scrolling may not be\nas fast when the background from the\nparent is inherited.";
pub const ORANGE: Color = Color::rgba8(255, 165, 0, 255);
pub const LIGHT_GREEN: Color = Color::rgba8(192, 255, 192, 255);
pub const GRADIENT_HEIGHT: f64 = 800.0;
pub const CONTENT_WIDTH: f64 = 700.0;
pub const CONTENT_HEIGHT: f64 = 900.0;
pub const GRADIENT: HostBackgroundProviderKey<GradientBackground> =
    HostBackgroundProviderKey::new("draw2d.official", "parent-gradient");

/// The Host owns this provider and its coverage; no Figure paints the gradient.
pub struct GradientBackground {
    pub coverage: Rectangle,
    pub midpoint: Color,
}
pub struct ParentGradient;
impl HostBackgroundProvider<GradientBackground> for ParentGradient {
    fn provide(
        &self,
        payload: &GradientBackground,
        _: HostBackgroundRequest,
    ) -> Result<HostBackgroundContent, HostBackgroundError> {
        let mut recorder = CommandRecorder::new();
        let mut graphics = recorder.paint();
        let gradient = LinearGradient::try_new(
            Point::ZERO,
            Point::new(0.0, GRADIENT_HEIGHT),
            &[
                GradientStop::try_new(0.0, Color::WHITE).map_err(provider_error)?,
                GradientStop::try_new(0.5, payload.midpoint).map_err(provider_error)?,
                GradientStop::try_new(1.0, Color::WHITE).map_err(provider_error)?,
            ],
        )
        .map_err(provider_error)?;
        graphics.set_fill_paint(gradient);
        graphics
            .fill_rect(payload.coverage)
            .map_err(provider_error)?;
        drop(graphics);
        Ok(HostBackgroundContent {
            coverage: payload.coverage,
            drawing: recorder.finish().map_err(provider_error)?,
        })
    }
}
fn provider_error(error: impl std::fmt::Display) -> HostBackgroundError {
    HostBackgroundError::Provider(error.to_string())
}

pub struct CanvasScene {
    pub runtime: Runtime,
    pub root: FigureId,
    pub viewport: FigureId,
}
impl CanvasScene {
    pub fn new(opaque: bool, width: f64, height: f64) -> Self {
        let mut runtime = Runtime::empty();
        for font in BuiltinFont::ALL {
            runtime
                .resources()
                .register_builtin_font(font)
                .expect("font");
        }
        let root = runtime
            .set_contents(Box::new(RectangleFigure::new_with_color(
                0.0,
                0.0,
                width,
                height,
                if opaque { ORANGE } else { Color::TRANSPARENT },
            )))
            .expect("Canvas root");
        let viewport = runtime
            .viewports()
            .add(root, Rectangle::new(0.0, 0.0, width, height))
            .expect("viewport")
            .figure_id();
        let contents = runtime
            .viewports()
            .set_contents(
                viewport,
                Box::new(RectangleFigure::new_with_color(
                    0.0,
                    0.0,
                    CONTENT_WIDTH,
                    CONTENT_HEIGHT,
                    Color::TRANSPARENT,
                )),
            )
            .expect("contents");
        let page = runtime
            .figure(contents)
            .expect("contents")
            .add(Box::new(FlowPageFigure::new(Rectangle::new(
                24.0, 24.0, 650.0, 300.0,
            ))))
            .expect("label page");
        runtime
            .figure(page)
            .expect("label page")
            .add(Box::new(StructuredTextFigure::new(TEXT).with_style(
                FigureStyle {
                    foreground: Some(Color::BLACK),
                    font: Some("18px Inter Variable".into()),
                    ..FigureStyle::default()
                },
            )))
            .expect("label");
        Self {
            runtime,
            root,
            viewport,
        }
    }
    pub fn resize(
        &mut self,
        width: f64,
        height: f64,
    ) -> Result<(), novadraw::runtime::RuntimeMutationError> {
        let bounds = Rectangle::new(0.0, 0.0, width, height);
        self.runtime.figure(self.root)?.set_bounds(bounds)?;
        self.runtime.figure(self.viewport)?.set_bounds(bounds)?;
        Ok(())
    }
}

pub fn canvas() -> Runtime {
    CanvasScene::new(false, 500.0, 600.0).runtime
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::{
        geometry::Affine2D,
        host::{
            BackgroundFrameInput, BackgroundInputVersions, HostBackgroundId, HostBackgroundRegistry,
        },
        render::{BackendCapabilities, SurfaceInfo},
    };

    #[test]
    fn both_canvases_shape_the_same_text_and_scroll_without_changing_host_input() {
        let mut registry = HostBackgroundRegistry::default();
        registry.register(GRADIENT, ParentGradient).unwrap();
        let input = BackgroundFrameInput {
            surface: SurfaceInfo {
                logical_width: 400.0,
                logical_height: 200.0,
                pixel_width: 600,
                pixel_height: 300,
                scale_factor: 1.5,
            },
            versions: BackgroundInputVersions {
                surface: 1,
                position: 1,
                dpi: 1,
            },
            host_to_canvas: Affine2D::from_translation(-24.0, -56.0),
        };
        let capabilities = BackendCapabilities::RETAINED_PARTIAL
            .with_host_background()
            .with_linear_gradients();
        for opaque in [false, true] {
            let mut scene = CanvasScene::new(opaque, 400.0, 200.0);
            let request = scene
                .runtime
                .frames()
                .begin_background(HostBackgroundId::new(), input, Color::WHITE)
                .unwrap();
            let snapshot = registry
                .provide(
                    GRADIENT,
                    &GradientBackground {
                        coverage: Rectangle::new(0.0, 0.0, 1000.0, 800.0),
                        midpoint: LIGHT_GREEN,
                    },
                    request,
                    1,
                )
                .unwrap();
            scene
                .runtime
                .frames()
                .complete_background(request, Ok(snapshot))
                .unwrap();
            let initial = scene
                .runtime
                .frames()
                .prepare_submission(input.surface, capabilities)
                .into_ready()
                .unwrap();
            assert!(initial.commands.iter().any(|c| matches!(
                c.kind,
                novadraw::render::RenderCommandKind::DrawGlyphRun { .. }
            )));
            let acknowledgement = scene
                .runtime
                .frames()
                .complete_submission(
                    initial.session_id,
                    initial.frame_id,
                    Ok(novadraw::render::RenderDisposition::Presented),
                )
                .unwrap();
            assert_eq!(
                acknowledgement,
                novadraw::runtime::SubmissionAcknowledgement::Presented
            );
            assert!(
                scene
                    .runtime
                    .viewports()
                    .set_view_location(scene.viewport, 24.0, 24.0)
                    .unwrap()
            );
            let scrolled = scene
                .runtime
                .frames()
                .prepare_submission(input.surface, capabilities)
                .into_ready()
                .unwrap();
            assert_eq!(scrolled.background_input.as_deref(), Some(&input));
            assert_ne!(initial.commands, scrolled.commands);
        }
    }
}
