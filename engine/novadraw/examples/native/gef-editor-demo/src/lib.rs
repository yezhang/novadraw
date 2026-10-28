pub mod model;
#[cfg(test)]
mod tests;
use model::{Diagram, ShapeFactory};
use novadraw::geometry::Rectangle;
use novadraw_editor::{
    CreateChild, CreateRequest, CreationType, DeleteParts, DeleteRequest, GraphicalViewer,
    RequestMetadata, RequestModifiers,
};
use novadraw_platform_masonry::session::{CanvasAction, EditorSession};
use novadraw_platform_masonry::view::editor_canvas;
use std::collections::VecDeque;
use xilem::{
    WidgetView, palette,
    style::Style,
    view::{FlexExt, flex_col, flex_row, label, sized_box, text_button},
};

pub type Session = EditorSession<Diagram, ShapeFactory>;

const CANVAS_BORDER_WIDTH: f64 = 1.0;

pub fn session() -> Session {
    EditorSession::new(
        GraphicalViewer::new(
            Diagram::default(),
            ShapeFactory,
            Rectangle::new(0.0, 0.0, 640.0, 480.0),
        )
        .expect("valid demo projection"),
    )
}

#[derive(Clone, Copy)]
pub enum UiCommand {
    Undo,
    Redo,
    Rectangle,
    Ellipse,
    Delete,
}

#[derive(Default)]
pub struct AppState {
    pub pending: VecDeque<UiCommand>,
    pub canvas: CanvasAction,
    pub disabled: bool,
}

/// Single application action queue; rebuild drains each request exactly once.
pub fn sync(app: &mut AppState, session: &mut Session) -> Result<bool, String> {
    let mut changed = false;
    while let Some(command) = app.pending.pop_front() {
        let result = match command {
            UiCommand::Undo => session.domain.undo(&mut session.viewer),
            UiCommand::Redo => session.domain.redo(&mut session.viewer),
            UiCommand::Rectangle | UiCommand::Ellipse => {
                let kind = if matches!(command, UiCommand::Rectangle) {
                    "rectangle"
                } else {
                    "ellipse"
                };
                let offset = session.viewer.model().shapes.len() as f64 * 24.0;
                let request = CreateRequest::new(
                    session.viewer.contents(),
                    CreationType::new(kind).unwrap(),
                    Rectangle::new(120.0 + offset, 100.0 + offset, 120.0, 80.0),
                    RequestModifiers::default(),
                    session
                        .domain
                        .next_interaction_revision()
                        .map_err(|error| error.to_string())?,
                );
                let metadata = RequestMetadata::new(request.revision(), vec![request.parent()]);
                session
                    .domain
                    .execute::<CreateChild, _>(&mut session.viewer, &metadata, &request)
            }
            UiCommand::Delete => {
                let request = DeleteRequest::new(
                    session.viewer.selection().items().to_vec(),
                    session
                        .domain
                        .next_interaction_revision()
                        .map_err(|error| error.to_string())?,
                );
                let metadata = RequestMetadata::new(request.revision(), request.parts().to_vec());
                session
                    .domain
                    .execute::<DeleteParts, _>(&mut session.viewer, &metadata, &request)
            }
        };
        if let Err(error) = result {
            app.canvas = session.snapshot(Some(error.to_string()));
            return Err(error.to_string());
        }
        changed = true;
    }
    if changed {
        app.canvas = session.snapshot(None);
    }
    Ok(changed)
}

pub fn canvas_view(app: &AppState) -> impl WidgetView<AppState> + use<> {
    editor_canvas(
        |_: &mut AppState| session(),
        sync,
        |app: &mut AppState, action: CanvasAction| {
            app.canvas = action;
        },
    )
    .disabled(app.disabled)
}

pub fn app_view(app: &mut AppState) -> impl WidgetView<AppState> + use<> {
    flex_col((
        label("Shapes editor"),
        flex_row((
            text_button("Rectangle", |app: &mut AppState| {
                app.pending.push_back(UiCommand::Rectangle)
            })
            .disabled(app.disabled),
            text_button("Ellipse", |app: &mut AppState| {
                app.pending.push_back(UiCommand::Ellipse)
            })
            .disabled(app.disabled),
            text_button("Delete", |app: &mut AppState| {
                app.pending.push_back(UiCommand::Delete)
            })
            .disabled(app.disabled),
            text_button("Undo", |app: &mut AppState| {
                app.pending.push_back(UiCommand::Undo)
            })
            .disabled(!app.canvas.can_undo || app.disabled),
            text_button("Redo", |app: &mut AppState| {
                app.pending.push_back(UiCommand::Redo)
            })
            .disabled(!app.canvas.can_redo || app.disabled),
            text_button(
                if app.disabled { "Enable" } else { "Disable" },
                |app: &mut AppState| app.disabled = !app.disabled,
            ),
        )),
        label(
            app.canvas
                .error
                .as_deref()
                .unwrap_or("Drag shapes. Shift-click selects. Escape cancels a drag."),
        ),
        sized_box(canvas_view(app))
            .border(
                palette::css::GRAY,
                xilem::masonry::layout::Length::px(CANVAS_BORDER_WIDTH),
            )
            .flex(1.0),
    ))
}
