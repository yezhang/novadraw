//! Application-owned adapter between the independent richtext controller and Editor.
use crate::{Config, HEIGHT, Result, WIDTH, editor};
use novadraw::{
    figure::{Figure, PreparationBudget, RectangleFigure, RootFigure},
    geometry::{Point, Rectangle, Translatable},
    render::{CaretGeometry, FontData, SelectionQuad},
    text::{BuiltinFont, TextAffinity, TextMovement},
};
use novadraw_editor::*;
use novadraw_richtext::*;
use std::{convert::Infallible, ops::Range, sync::Arc, time::Instant};

const ROOT: u64 = 1;
const TEXT: u64 = 2;
const MARGIN: f64 = 24.0;
const CONTENT_PADDING: f64 = 16.0;
const CONTENT_TOP: f64 = 126.0;
const WORK_BUDGET: u64 = 80_000_000;
const ALLOCATION_BUDGET: u64 = 64_000_000;
const DOCUMENT_HEIGHT: f64 = 1_000_000.0;

fn feature() -> DirectTextFeature {
    DirectTextFeature::new("richtext.body").unwrap()
}
fn failure(error: impl std::error::Error + 'static) -> ViewerError {
    PolicyError::operation_with_source("richtext controller rejected input", error).into()
}
pub struct Model {
    pub document: RichTextDocumentSnapshot,
    revision: ModelRevision,
    pub bounds: Rectangle,
    events: Vec<ModelEvent<u64, ()>>,
}
impl Model {
    fn replace(&mut self, document: RichTextDocumentSnapshot) {
        self.document = document;
        self.publish();
    }
    fn publish(&mut self) {
        self.revision = self.revision.next().unwrap();
        self.events.push(ModelEvent::new(self.revision, TEXT, ()));
    }
}
impl ModelAdapter for Model {
    type ModelId = u64;
    type Event = ();
    type Error = Infallible;
    fn root(&self) -> u64 {
        ROOT
    }
    fn revision(&self) -> ModelRevision {
        self.revision
    }
    fn children(&self, id: u64) -> std::result::Result<Vec<u64>, Infallible> {
        Ok(if id == ROOT { vec![TEXT] } else { vec![] })
    }
    fn drain_events(&mut self) -> Vec<ModelEvent<u64, ()>> {
        std::mem::take(&mut self.events)
    }
}

/// This adapter owns only the independent controller; no second draft or selection.
#[derive(Clone)]
pub struct Controller(pub RichTextEditController);
impl Controller {
    fn layout(
        context: &EditGeometryContext<'_>,
    ) -> std::result::Result<Arc<RichTextLayoutSnapshot>, ViewerError> {
        context.prepared()
    }
    fn position(
        &self,
        layout: &RichTextLayoutSnapshot,
        cursor: RichTextCursor,
    ) -> std::result::Result<RichTextPosition, ViewerError> {
        layout
            .position(cursor.paragraph, cursor.text)
            .map_err(failure)
    }
    fn transform_bounds(
        context: &EditGeometryContext<'_>,
        mut bounds: Rectangle,
    ) -> std::result::Result<Rectangle, ViewerError> {
        bounds.transform(
            context
                .tree()
                .local_to_surface_transform(context.target())
                .map_err(ViewerError::Coordinate)?,
        );
        Ok(bounds)
    }
}
impl EditController for Controller {
    fn input(
        &mut self,
        event: &TextInputEvent,
        context: &mut EditGeometryContext<'_>,
    ) -> std::result::Result<bool, ViewerError> {
        Ok(match event {
            TextInputEvent::InsertText(text) => self.0.insert(text).map_err(failure)?,
            TextInputEvent::Preedit { text, selection } => {
                self.0.preedit(text, selection.clone()).map_err(failure)?
            }
            TextInputEvent::CancelComposition => self.0.cancel_composition(),
            TextInputEvent::Move { movement, extend } => {
                self.0
                    .move_selection(
                        Self::layout(context)?.as_ref(),
                        *movement,
                        *extend == ExtendTextSelection::Yes,
                    )
                    .map_err(failure)?;
                true
            }
            TextInputEvent::Delete(deletion) => match deletion {
                TextDelete::PreviousVisual => self.0.delete(true).map_err(failure)?,
                TextDelete::NextVisual => self.0.delete(false).map_err(failure)?,
                TextDelete::PreviousWord | TextDelete::NextWord => {
                    if self.0.selection().anchor == self.0.selection().focus {
                        let movement = if *deletion == TextDelete::PreviousWord {
                            TextMovement::PreviousWord
                        } else {
                            TextMovement::NextWord
                        };
                        self.0
                            .move_selection(Self::layout(context)?.as_ref(), movement, true)
                            .map_err(failure)?;
                    }
                    self.0.insert("").map_err(failure)?
                }
            },
            TextInputEvent::SelectAll => {
                let paragraph = self.0.selection().focus.paragraph;
                let len = self
                    .0
                    .document()
                    .paragraphs()
                    .iter()
                    .find(|p| p.id() == paragraph)
                    .unwrap()
                    .len_bytes();
                let anchor = self
                    .0
                    .cursor(paragraph, 0, TextAffinity::Downstream)
                    .map_err(failure)?;
                let focus = self
                    .0
                    .cursor(paragraph, len, TextAffinity::Upstream)
                    .map_err(failure)?;
                self.0
                    .set_selection(RichTextSelection { anchor, focus })
                    .map_err(failure)?;
                true
            }
            TextInputEvent::Synchronize(_) => {
                return Err(PolicyError::operation(
                    "this host uses paragraph input events; buffer synchronization is unsupported",
                )
                .into());
            }
            _ => false,
        })
    }
    fn select_at(
        &mut self,
        point: Point,
        extend: bool,
        context: &mut EditGeometryContext<'_>,
    ) -> std::result::Result<(), ViewerError> {
        let inverse = context
            .tree()
            .surface_to_local_transform(context.target())
            .map_err(ViewerError::Coordinate)?
            .ok_or(DirectTextEditError::InvalidTextPosition)?;
        self.0
            .select_at(
                Self::layout(context)?.as_ref(),
                inverse.transform_point(point),
                extend,
            )
            .map_err(failure)
    }
    fn geometry(
        &mut self,
        context: &mut EditGeometryContext<'_>,
    ) -> std::result::Result<EditGeometry, ViewerError> {
        let bounds = context.tree().figure_bounds(context.target())?;
        let viewport = context
            .host_viewport()
            .ok_or(DirectTextEditError::InvalidFeedbackTarget)?;
        let width = (viewport.width - (MARGIN + CONTENT_PADDING) * 2.0).max(1.0);
        if bounds.width != width {
            context.set_bounds(Rectangle::new(bounds.x, bounds.y, width, bounds.height))?;
            context.stabilize()?;
        }
        let layout = Self::layout(context)?;
        let caret = self.position(&layout, self.0.caret().map_err(failure)?)?;
        let caret = Self::transform_bounds(context, layout.caret_bounds(&caret).map_err(failure)?)?;
        let selection = self.0.selection();
        let quads =
            |selection: RichTextSelection| -> std::result::Result<Vec<SelectionQuad>, ViewerError> {
                let a = self.position(&layout, selection.anchor)?;
                let f = self.position(&layout, selection.focus)?;
                layout
                    .selection_geometry(&a, &f)
                    .map_err(failure)?
                    .into_iter()
                    .enumerate()
                    .map(|(i, quad)| {
                        Ok(SelectionQuad::new(
                            Self::transform_bounds(context, quad.bounds())?,
                            i,
                        ))
                    })
                    .collect()
            };
        let composition = self.0.composition();
        Ok(EditGeometry {
            caret: CaretGeometry::new(caret, 0),
            viewport,
            selection: if composition.is_none() {
                quads(selection)?
            } else {
                vec![]
            },
            preedit: composition
                .map(|c| quads(c.range))
                .transpose()?
                .unwrap_or_default(),
            caret_visible: composition.is_none_or(|c| c.selection.is_some()),
        })
    }
    fn is_changed(&self) -> bool {
        self.0.is_changed()
    }
    fn has_composition(&self) -> bool {
        self.0.composition().is_some()
    }
    fn input_snapshot(&self) -> std::result::Result<TextInputSnapshot, ViewerError> {
        let s = self.0.selection();
        if s.anchor.paragraph != s.focus.paragraph {
            return Err(DirectTextEditError::InvalidTextPosition.into());
        }
        let text = self
            .0
            .document()
            .paragraphs()
            .iter()
            .find(|p| p.id() == s.focus.paragraph)
            .unwrap()
            .text();
        let composition = self
            .0
            .composition()
            .map(|c| c.range.anchor.text.byte_offset()..c.range.focus.text.byte_offset());
        Ok(TextInputSnapshot::new(
            text,
            s.anchor.text.byte_offset()..s.focus.text.byte_offset(),
            composition,
        ))
    }
    fn range_bounds(
        &self,
        range: Range<usize>,
        context: &mut EditGeometryContext<'_>,
    ) -> std::result::Result<Rectangle, ViewerError> {
        let layout = Self::layout(context)?;
        let paragraph = self.0.selection().focus.paragraph;
        let a = self
            .0
            .cursor(paragraph, range.start, TextAffinity::Downstream)
            .map_err(failure)?;
        let f = self
            .0
            .cursor(paragraph, range.end, TextAffinity::Upstream)
            .map_err(failure)?;
        let a = self.position(&layout, a)?;
        let f = self.position(&layout, f)?;
        let bounds = layout
            .selection_geometry(&a, &f)
            .map_err(failure)?
            .into_iter()
            .map(|q| q.bounds())
            .reduce(|a, b| a.union(b))
            .unwrap_or(layout.caret_bounds(&a).map_err(failure)?);
        Self::transform_bounds(context, bounds)
    }
}
struct Plan {
    content: RichTextContentFigure,
}
impl EditSessionPlan<Model, Controller> for Plan {
    fn reuse_feedback(
        &mut self,
        previous: &Controller,
        candidate: &Controller,
        model: &Model,
    ) -> std::result::Result<bool, PolicyError> {
        Ok(
            previous.0.document().version() == candidate.0.document().version()
                && self.content.document().version() == candidate.0.document().version()
                && self.content.initial_bounds() == model.bounds,
        )
    }
    fn feedback(
        &mut self,
        controller: &Controller,
        model: &Model,
    ) -> std::result::Result<DirectTextFeedback, PolicyError> {
        self.content = self
            .content
            .clone()
            .with_document(controller.0.document().clone())
            .with_bounds(model.bounds);
        DirectTextFeedback::new(
            vec![
                FeedbackVisual::scaled(Box::new(
                    RectangleFigure::new_with_color(
                        0.0,
                        0.0,
                        model.bounds.x + model.bounds.width,
                        DOCUMENT_HEIGHT,
                        novadraw::graphics::Color::WHITE,
                    )
                    .unwrap(),
                )),
                FeedbackVisual::scaled(Box::new(self.content.clone())),
            ],
            1,
        )
    }
    fn command(
        &mut self,
        controller: &Controller,
        model: &Model,
    ) -> std::result::Result<Box<dyn Command<Model>>, PolicyError> {
        Ok(Box::new(ReplaceDocument {
            before: model.document.clone(),
            after: controller.0.document().clone(),
        }))
    }
}
struct ReplaceDocument {
    before: RichTextDocumentSnapshot,
    after: RichTextDocumentSnapshot,
}
impl Command<Model> for ReplaceDocument {
    fn label(&self) -> &str {
        "Edit richtext document"
    }
    fn execute(&mut self, model: &mut Model) -> std::result::Result<(), CommandError> {
        model.replace(self.after.clone());
        Ok(())
    }
    fn undo(&mut self, model: &mut Model) -> std::result::Result<(), CommandError> {
        model.replace(self.before.clone());
        Ok(())
    }
}
struct Policy;
impl EditPolicy<Model> for Policy {
    fn register_handlers(builder: &mut PolicyHandlerBuilder<'_, Model, Self>) {
        builder.handle::<StartEditSession>(Self::start);
    }
}
impl Policy {
    fn start(
        &mut self,
        host: PolicyHost<u64>,
        _: &RequestMetadata,
        request: &DirectTextEditRequest,
        model: &Model,
    ) -> std::result::Result<Option<EditSessionContribution<Model>>, PolicyError> {
        if host.model() != TEXT || request.feature() != &feature() {
            return Ok(None);
        }
        let controller = RichTextEditController::new(model.document.clone())
            .map_err(|e| PolicyError::operation_with_source("controller construction", e))?;
        Ok(Some(EditSessionContribution::new(
            EditControllerKey::new(),
            EditSessionDescriptor::new(
                feature(),
                model.revision(),
                TextEditMode::Multiline,
                FocusLossPolicy::Accept,
            ),
            Controller(controller),
            Plan {
                content: RichTextContentFigure::new(model.document.clone()),
            },
        )))
    }
}
struct Part {
    version: Option<DocumentVersion>,
}
impl EditPartBehavior<Model> for Part {
    fn create_figure(
        &mut self,
        model: &Model,
        id: u64,
    ) -> std::result::Result<Box<dyn Figure>, EditPartError> {
        Ok(if id == ROOT {
            Box::new(RootFigure::new(0.0, 0.0, WIDTH, HEIGHT).unwrap())
        } else {
            self.version = Some(model.document.version());
            Box::new(RichTextContentFigure::new(model.document.clone()).with_bounds(model.bounds))
        })
    }
    fn refresh_visuals(
        &mut self,
        model: &Model,
        id: u64,
        context: &mut VisualUpdateContext<'_>,
    ) -> std::result::Result<(), EditPartError> {
        if id == TEXT {
            if let Some(expected) = self.version
                && expected != model.document.version()
            {
                context.update_visual_component(
                    context.primary(),
                    ReplaceRichTextDocument {
                        expected,
                        document: model.document.clone(),
                    },
                )?;
                self.version = Some(model.document.version());
            }
            context.set_primary_bounds(model.bounds)?;
        }
        Ok(())
    }
    fn create_policies(
        &mut self,
        _: &Model,
        id: u64,
    ) -> std::result::Result<Vec<PolicyInstallation<Model>>, EditPartError> {
        Ok(if id == TEXT {
            vec![
                PolicyInstallation::new(DirectTextEditSlot::key(), Policy)
                    .map_err(|e| EditPartError::operation(e.to_string()))?,
            ]
        } else {
            vec![]
        })
    }
}
pub struct Factory;
impl EditPartFactory<Model> for Factory {
    fn create(
        &mut self,
        _: PartFactoryContext<u64>,
        _: &Model,
    ) -> std::result::Result<Box<dyn EditPartBehavior<Model>>, EditPartError> {
        Ok(Box::new(Part { version: None }))
    }
}
pub struct Embedded {
    pub viewer: GraphicalViewer<Model, Factory>,
    pub domain: EditorDomain<Model>,
    pub timing: editor::Timing,
    pub metadata: serde_json::Value,
}
impl Embedded {
    pub fn new(config: &Config) -> Result<Self> {
        let document = editor::document(config)?;
        let metadata = serde_json::json!({"load":config.load,"characters":document.paragraphs().iter().map(|p|p.text().chars().count()).sum::<usize>(),"host":"editor","gpu_submission":false});
        let model = Model {
            document,
            revision: ModelRevision::initial(),
            bounds: Rectangle::new(
                MARGIN + CONTENT_PADDING,
                CONTENT_TOP,
                WIDTH - (MARGIN + CONTENT_PADDING) * 2.0,
                DOCUMENT_HEIGHT,
            ),
            events: vec![],
        };
        let mut viewer =
            GraphicalViewer::new(model, Factory, Rectangle::new(0.0, 0.0, WIDTH, HEIGHT))?;
        viewer.drive().resize_logical_viewport(WIDTH, HEIGHT)?;
        for font in BuiltinFont::ALL {
            viewer.drive().register_builtin_font(font)?;
        }
        viewer
            .drive()
            .register_font_data(FontData::new(std::fs::read(
                "/System/Library/Fonts/Apple Color Emoji.ttc",
            )?))?;
        viewer
            .drive()
            .set_preparation_budget(PreparationBudget::new(WORK_BUDGET, ALLOCATION_BUDGET));
        viewer.stabilize_for_query()?;
        Ok(Self {
            viewer,
            domain: EditorDomain::new(),
            timing: Default::default(),
            metadata,
        })
    }
    pub fn start(&mut self) -> Result<()> {
        if self.viewer.edit_session().is_none() {
            let source = self.viewer.part_for_model(TEXT).unwrap();
            self.domain
                .start_edit_session(&mut self.viewer, source, feature())?;
        }
        Ok(())
    }
    pub fn controller(&self) -> Result<&RichTextEditController> {
        Ok(&self
            .viewer
            .edit_session()
            .ok_or("no session")?
            .controller(EditControllerKey::<Controller>::new())
            .ok_or("controller type")?
            .0)
    }
    pub fn input(&mut self, event: TextInputEvent) -> Result<()> {
        self.start()?;
        let session = self.viewer.edit_session().unwrap().session();
        let start = Instant::now();
        self.domain.handle_text_input_event(
            &mut self.viewer,
            SessionTextInputEvent::new(session, event),
        )?;
        self.timing = editor::Timing {
            mutation_ns: start.elapsed().as_nanos(),
            layout_ns: 0,
        };
        Ok(())
    }
    pub fn locate(&mut self, region: usize) -> Result<()> {
        let point = self.location_point(region)?;
        self.domain.pointer_pressed(
            &mut self.viewer,
            point,
            novadraw::event::MouseButton::Left,
            Default::default(),
        )?;
        self.domain.pointer_released(
            &mut self.viewer,
            point,
            novadraw::event::MouseButton::Left,
        )?;
        Ok(())
    }
    pub fn prepared_layout(&self) -> Result<Arc<RichTextLayoutSnapshot>> {
        let target = self
            .viewer
            .runtime()
            .tree()
            .child_order(self.viewer.root_layers().scaled_feedback())?[1];
        self.viewer
            .runtime()
            .tree()
            .prepared_presentation(target)?
            .and_then(|p| p.shared_data::<RichTextLayoutSnapshot>())
            .ok_or_else(|| "missing layout".into())
    }
    pub fn location_point(&mut self, region: usize) -> Result<Point> {
        // Move through the typed controller's own admitted geometry using scene input.
        self.start()?;
        let layout = self.prepared_layout()?;
        let paragraphs = layout.document().paragraphs();
        let paragraph = &paragraphs[match region {
            0 => 0,
            1 => paragraphs.len() / 2,
            _ => paragraphs.len() - 1,
        }];
        let byte = if region == 0 {
            0
        } else if region == 1 {
            paragraph
                .text()
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|i| *i <= paragraph.len_bytes() / 2)
                .last()
                .unwrap_or(0)
        } else {
            paragraph.len_bytes()
        };
        let p = layout.position(
            paragraph.id(),
            novadraw::text::TextPosition::new(byte, TextAffinity::Downstream),
        )?;
        let b = layout.caret_bounds(&p)?;
        self.domain
            .set_viewport_origin(&mut self.viewer, Point::new(0.0, (b.y - MARGIN).max(0.0)))?;
        let target = self
            .viewer
            .runtime()
            .tree()
            .child_order(self.viewer.root_layers().scaled_feedback())?[1];
        let transform = self
            .viewer
            .runtime()
            .tree()
            .local_to_surface_transform(target)?;
        let point = transform.transform_point(Point::new(b.x, b.y + b.height / 2.0));
        Ok(point)
    }
    pub fn resize(&mut self, width: f64, height: f64) -> Result<()> {
        // Resize publication does not alter source revision captured by the session.
        self.viewer.drive().resize_logical_viewport(width, height)?;
        self.viewer.stabilize_for_query()?;
        self.viewer.synchronize_direct_text_input_area()?;
        Ok(())
    }
}

pub fn verify(config: &Config) -> Result<()> {
    let mut editor = Embedded::new(config)?;
    let original = editor.viewer.model().document.version();
    editor.start()?;
    let session = editor.viewer.edit_session().unwrap().session();
    editor.input(TextInputEvent::Preedit {
        text: "ni".into(),
        selection: Some(0..2),
    })?;
    assert_eq!(editor.viewer.model().document.version(), original);
    assert!(editor.controller()?.composition().is_some());
    editor.input(TextInputEvent::InsertText("你👩‍💻".into()))?;
    assert!(
        editor.controller()?.document().paragraphs()[0]
            .text()
            .starts_with("你👩‍💻")
    );
    editor.input(TextInputEvent::Delete(TextDelete::PreviousVisual))?;
    assert!(
        editor.controller()?.document().paragraphs()[0]
            .text()
            .starts_with("你点击")
    );
    editor.input(TextInputEvent::Accept)?;
    assert!(editor.viewer.edit_session().is_none());
    assert!(editor.domain.has_undo());
    editor.domain.undo(&mut editor.viewer)?;
    assert_eq!(editor.viewer.model().document.version(), original);
    assert!(!editor.domain.has_undo());
    editor.domain.redo(&mut editor.viewer)?;
    editor.start()?;
    assert!(
        editor
            .domain
            .handle_text_input_event(
                &mut editor.viewer,
                SessionTextInputEvent::new(session, TextInputEvent::InsertText("迟到".into()))
            )
            .is_err()
    );
    let model = editor.viewer.model().document.version();
    editor.input(TextInputEvent::InsertText("取消".into()))?;
    editor.input(TextInputEvent::Cancel)?;
    assert_eq!(editor.viewer.model().document.version(), model);
    editor.start()?;
    editor.input(TextInputEvent::Accept)?;
    editor.domain.undo(&mut editor.viewer)?;
    assert_eq!(editor.viewer.model().document.version(), original);
    for region in 0..3 {
        editor.locate(region)?;
        editor.input(TextInputEvent::InsertText(editor::PAYLOAD.into()))?;
    }
    editor.locate(0)?;
    let bounds = editor.viewer.direct_text_caret_geometry()?.bounds();
    let start = Point::new(bounds.x, bounds.y + bounds.height / 2.0);
    let end = Point::new(start.x() + 60.0, start.y());
    editor.domain.pointer_pressed(
        &mut editor.viewer,
        start,
        novadraw::event::MouseButton::Left,
        Default::default(),
    )?;
    editor.domain.pointer_moved(&mut editor.viewer, end)?;
    editor
        .domain
        .pointer_released(&mut editor.viewer, end, novadraw::event::MouseButton::Left)?;
    assert_ne!(
        editor.controller()?.selection().anchor,
        editor.controller()?.selection().focus
    );
    let version = editor.controller()?.document().version();
    editor.resize(WIDTH / 2.0, HEIGHT)?;
    let target = editor
        .viewer
        .runtime()
        .tree()
        .child_order(editor.viewer.root_layers().scaled_feedback())?[1];
    assert_eq!(
        editor.viewer.runtime().tree().figure_bounds(target)?.width,
        WIDTH / 2.0 - (MARGIN + CONTENT_PADDING) * 2.0
    );
    assert_eq!(editor.controller()?.document().version(), version);
    assert!(editor.viewer.direct_text_caret_geometry()?.bounds().height > 0.0);
    editor
        .domain
        .set_viewport_scale_at(&mut editor.viewer, 1.5, None)?;
    assert!(editor.viewer.direct_text_caret_geometry()?.bounds().height > 0.0);
    editor.input(TextInputEvent::Cancel)?;
    println!(
        "ADR-061 embedded richtext: composition, Emoji deletion, model isolation, one-command undo/redo, stale lease, cancel/NoChange, locations, resize and zoom passed"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn pointer_and_drag_reuse_layout_and_blink_resets_on_input() {
        use super::*;
        use novadraw::event::{MonotonicTime, MouseButton};
        let mut editor = Embedded::new(&crate::Config::parse(&[]).unwrap()).unwrap();
        let now = MonotonicTime::from_micros(2_000_000);
        editor.viewer.drive().advance_time(now).unwrap();
        editor.start().unwrap();
        let original = editor.prepared_layout().unwrap();
        let point = editor.location_point(0).unwrap();
        editor
            .domain
            .pointer_pressed(
                &mut editor.viewer,
                point,
                MouseButton::Left,
                Default::default(),
            )
            .unwrap();
        editor
            .domain
            .pointer_moved(&mut editor.viewer, Point::new(point.x() + 30.0, point.y()))
            .unwrap();
        editor
            .domain
            .pointer_released(&mut editor.viewer, point, MouseButton::Left)
            .unwrap();
        assert!(Arc::ptr_eq(&original, &editor.prepared_layout().unwrap()));
        assert_ne!(
            editor.controller().unwrap().selection().anchor,
            editor.controller().unwrap().selection().focus
        );
        editor.locate(0).unwrap();
        let caret = *editor
            .viewer
            .runtime()
            .tree()
            .child_order(editor.viewer.root_layers().feedback())
            .unwrap()
            .last()
            .unwrap();
        assert!(editor.viewer.runtime().tree().is_visible(caret).unwrap());
        let off = editor.viewer.drive().next_wake_deadline().unwrap();
        let interval = off.as_micros() - now.as_micros();
        assert_eq!(interval, 530_000);
        editor
            .viewer
            .drive()
            .advance_time(MonotonicTime::from_micros(off.as_micros() - 1))
            .unwrap();
        assert!(editor.viewer.runtime().tree().is_visible(caret).unwrap());
        assert!(editor.viewer.drive().advance_time(off).unwrap());
        assert!(!editor.viewer.runtime().tree().is_visible(caret).unwrap());
        let on = editor.viewer.drive().next_wake_deadline().unwrap();
        assert!(editor.viewer.drive().advance_time(on).unwrap());
        assert!(editor.viewer.runtime().tree().is_visible(caret).unwrap());
        assert!(Arc::ptr_eq(&original, &editor.prepared_layout().unwrap()));
        editor
            .input(TextInputEvent::InsertText("你".into()))
            .unwrap();
        assert!(!Arc::ptr_eq(&original, &editor.prepared_layout().unwrap()));
        assert_eq!(
            editor
                .viewer
                .drive()
                .next_wake_deadline()
                .unwrap()
                .as_micros(),
            on.as_micros() + interval
        );
        editor.input(TextInputEvent::Cancel).unwrap();
        assert_eq!(editor.viewer.drive().next_wake_deadline(), None);
    }

    #[test]
    fn embedded_contract() {
        super::verify(&crate::Config::parse(&[]).unwrap()).unwrap();
    }
}
