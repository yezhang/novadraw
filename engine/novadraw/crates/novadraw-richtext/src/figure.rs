use crate::{DocumentError, DocumentVersion, RichTextDocumentSnapshot, RichTextLayoutSnapshot};
use novadraw::event::PropertyKey;
use novadraw::figure::{
    BORDER, BorderCapability, BorderedFigure, CONTAINER, ContainerCapability, FigureContainer,
    FigureStyle,
    border::{Border, MarginBorder},
};
use novadraw::graphics::{Color, GraphicsError, PaintContext};
use novadraw::{
    container::{ScrollBarVisibility, ScrollPaneHandle},
    figure::{
        Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureMeasurement,
        FigurePreparation, FigurePresentation, MeasureConstraints, PREPARATION,
        PreparationBudgetHandle, PreparationCapability, PreparationMeasureError,
        PreparationMeasureErrorKind, PreparationMeasurement, PreparationScene, PreparedScene,
        RectangleFigure,
    },
    geometry::{Dimension, Insets, Rectangle},
    layout::StackLayout,
    runtime::{FigureComponentContext, FigureComponentUpdate, PreparedFigureUpdate},
    text::{MeasureContext, TextError},
    tree::{FigureId, FigureTreeBuilder},
};
use std::sync::Arc;

const DEFAULT_CONTENT_PADDING: f64 = 16.0;

pub const RICH_TEXT_DOCUMENT: PropertyKey<DocumentVersion> =
    PropertyKey::new("novadraw-richtext", "document");

/// Single source owner for the entire document, regardless of paragraph count.
pub struct RichTextContentFigure {
    document: RichTextDocumentSnapshot,
    cache: crate::cache::SnapshotCache,
    bounds: Rectangle,
}
impl RichTextContentFigure {
    pub fn new(document: RichTextDocumentSnapshot) -> Self {
        Self {
            document,
            cache: Default::default(),
            bounds: Rectangle::ZERO,
        }
    }
    pub fn document(&self) -> &RichTextDocumentSnapshot {
        &self.document
    }
    /// Sets detached scene bounds; attached changes use Runtime mutation.
    pub fn with_bounds(mut self, bounds: Rectangle) -> Self {
        self.bounds = bounds;
        self
    }
    fn presentation(
        snapshot: Arc<RichTextLayoutSnapshot>,
    ) -> Result<FigurePresentation, TextError> {
        let size = snapshot.size();
        FigurePresentation::new(
            FigureMeasurement::new(size.width, size.height, None),
            Dimension::ZERO,
            snapshot.visual_bounds(),
            snapshot.clone(),
        )
        .map(|presentation| presentation.with_data(snapshot))
    }
}
impl Figure for RichTextContentFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }
    fn name(&self) -> &'static str {
        "RichTextContentFigure"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(PREPARATION, PreparationCapability::of::<Self>())
    }
}
impl FigurePreparation for RichTextContentFigure {
    fn measure_scene(
        &self,
        context: &mut MeasureContext<'_>,
        constraints: MeasureConstraints,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparationMeasurement, PreparationMeasureError> {
        let result = (|| {
            let style = scene.style(scene.root())?;
            self.cache.prepare(
                &self.document,
                context,
                constraints.max_width(),
                scene.budget(),
                style.layout_direction,
            )
        })();
        result
            .map(|snapshot| PreparationMeasurement {
                preferred: FigureMeasurement::new(
                    snapshot.size().width,
                    snapshot.size().height,
                    None,
                ),
                minimum: Dimension::ZERO,
            })
            .map_err(|e| PreparationMeasureError {
                figure: scene.root(),
                outer_constraints: constraints,
                content_constraints: Some(constraints),
                kind: PreparationMeasureErrorKind::Text(Box::new(e)),
            })
    }
    fn prepare(
        &self,
        context: &mut MeasureContext<'_>,
        bounds: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        Self::presentation(self.cache.prepare(
            &self.document,
            context,
            Some(bounds.width),
            PreparationBudgetHandle::new(Default::default()),
            Default::default(),
        )?)
    }
    fn prepare_scene(
        &self,
        context: &mut MeasureContext<'_>,
        bounds: Rectangle,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparedScene, TextError> {
        let direction = scene.style(scene.root())?.layout_direction;
        Self::presentation(self.cache.prepare(
            &self.document,
            context,
            Some(bounds.width),
            scene.budget(),
            direction,
        )?)
        .map(PreparedScene::new)
    }
}

/// Typed conditional source replacement. Candidates can be constructed outside
/// Runtime, but attached source commits only through FigureMut::update_component.
pub struct ReplaceRichTextDocument {
    pub expected: DocumentVersion,
    pub document: RichTextDocumentSnapshot,
}
impl FigureComponentUpdate for ReplaceRichTextDocument {
    type Figure = RichTextContentFigure;
    type Prepared = RichTextDocumentSnapshot;
    type Error = DocumentError;
    fn prepare(
        self,
        current: &Self::Figure,
        _: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        if current.document.version() != self.expected {
            return Err(DocumentError::StaleVersion);
        }
        if current.document.version() == self.document.version() {
            return Ok(PreparedFigureUpdate::unchanged(self.document));
        }
        let old = current.document.version();
        let new = self.document.version();
        Ok(
            PreparedFigureUpdate::layout_geometry_and_paint(self.document).with_property_change(
                RICH_TEXT_DOCUMENT,
                old,
                new,
            ),
        )
    }
    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.document = prepared;
    }
}

/// Named identities, not a second document or scroll-state owner.
#[derive(Clone)]
pub struct RichTextHandle {
    root: FigureId,
    content: FigureId,
    pane: ScrollPaneHandle,
}
impl RichTextHandle {
    pub fn root(&self) -> FigureId {
        self.root
    }
    pub fn content(&self) -> FigureId {
        self.content
    }
    pub fn scroll_pane(&self) -> &ScrollPaneHandle {
        &self.pane
    }
}

/// Detached construction facade. All children are built before Runtime takes
/// ownership. This is not an atomic attached composite mounting API.
pub struct RichTextFigure {
    shell: RectangleFigure,
}
impl Figure for RichTextFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.shell.initial_bounds()
    }
    fn initial_style(&self) -> FigureStyle {
        self.shell.initial_style()
    }
    fn initial_insets(&self) -> Insets {
        self.shell.initial_insets()
    }
    fn name(&self) -> &'static str {
        "RichTextFigure"
    }
    fn get_border(&self) -> Option<&dyn Border> {
        self.shell.get_border()
    }
    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        self.shell.paint(context, bounds)
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())?;
        out.register(BORDER, BorderCapability::of::<Self>())
    }
}
impl FigureContainer for RichTextFigure {}
impl BorderedFigure for RichTextFigure {
    fn border(&self) -> Option<&Arc<dyn Border>> {
        self.shell.border()
    }
    fn replace_border(
        &mut self,
        candidate: novadraw::figure::border::BorderCandidate,
    ) -> Option<Arc<dyn Border>> {
        self.shell.replace_border(candidate)
    }
}
impl RichTextFigure {
    pub fn add_to(
        builder: &mut FigureTreeBuilder<'_>,
        parent: FigureId,
        bounds: Rectangle,
        document: RichTextDocumentSnapshot,
    ) -> Result<RichTextHandle, Box<dyn std::error::Error>> {
        let padding = DEFAULT_CONTENT_PADDING;
        let shell = RectangleFigure::new_with_color(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            Color::WHITE,
        )?
        .with_border(MarginBorder::new(Insets {
            top: padding,
            left: padding,
            bottom: padding,
            right: padding,
        })?)?;
        let root = builder.add_child(parent, Box::new(Self { shell }))?;
        builder.set_layout_manager(root, Box::new(StackLayout))?;
        let pane = builder
            .add_scroll_pane_to(root, Rectangle::new(0.0, 0.0, bounds.width, bounds.height))?;
        builder.set_scroll_bar_visibility(
            &pane,
            ScrollBarVisibility::Never,
            ScrollBarVisibility::Always,
        )?;
        builder.set_viewport_tracks_width(pane.viewport().figure_id(), true)?;
        let content = builder
            .set_scroll_pane_contents(&pane, Box::new(RichTextContentFigure::new(document)))?;
        Ok(RichTextHandle {
            root,
            content,
            pane,
        })
    }
}
