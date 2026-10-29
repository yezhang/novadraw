pub mod alignment;
pub mod model;
pub mod native;
pub mod session;
#[cfg(test)]
mod tests;
pub mod ui;
pub use alignment::Alignment;
use model::{
    Diagram, EdgeStyle, FIRST, MIN_SIZE, MoveShape, RenameShape, ReorderShapes, ReplaceGroups,
    ShapeFactory, ShapeGroup, ShapeKind, ShapePresentation, ShapeTemplate, UpdateShapePresentation,
    WorkspaceSettings,
};
use novadraw::connection::ShortestPathConnectionRouter;
use novadraw::geometry::{Point, Rectangle};
use novadraw::tree::FigureId;
use novadraw_editor::tool::{CreationTool, MarqueeTool};
use novadraw_editor::{
    CreationType, DeleteParts, DeleteRequest, GraphicalViewer, RequestMetadata, RequestModifiers,
};
pub use session::{CanvasAction, Session};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

pub fn session() -> Session {
    let mut session = session_from_model(Diagram::default()).expect("valid initial diagram");
    let first = session
        .viewer
        .part_for_model(FIRST)
        .expect("showcase first shape is projected");
    session
        .viewer
        .replace_selection(first)
        .expect("showcase first shape is selectable");
    session
        .sync_selection_handles()
        .expect("showcase selection handles are valid");
    session
}

fn session_from_model(model: Diagram) -> Result<Session, String> {
    session_from_model_with_workspace(model, WorkspaceSettings::default())
}

fn session_from_model_with_workspace(
    model: Diagram,
    workspace: WorkspaceSettings,
) -> Result<Session, String> {
    let mut session = Session::new(
        GraphicalViewer::new(
            model,
            ShapeFactory::new(workspace.clone()),
            Rectangle::new(0.0, 0.0, 640.0, 480.0),
        )
        .map_err(|e| e.to_string())?,
        workspace,
    );
    update_router(&mut session)?;
    Ok(session)
}

fn obstacles(session: &Session) -> Vec<FigureId> {
    session
        .viewer
        .model()
        .order
        .iter()
        .filter(|id| session.viewer.model().presentation(**id).visible)
        .filter_map(|id| {
            session
                .viewer
                .part_for_model(*id)
                .and_then(|part| session.viewer.parts().get(part).map(|p| p.primary_figure()))
        })
        .collect()
}
fn update_router(session: &mut Session) -> Result<(), String> {
    session
        .viewer
        .replace_connection_layer_router(Box::new(ShortestPathConnectionRouter::new(obstacles(
            session,
        ))))
        .map_err(|e| e.to_string())
}

#[derive(Clone, Debug)]
pub enum UiCommand {
    Undo,
    Redo,
    Rectangle,
    Ellipse,
    Delete,
    Align(Alignment),
    SelectTool,
    Marquee,
    Connect(EdgeStyle),
    Drop(ShapeKind, Point),
    Select(u64),
    SelectGroup(u64),
    ApplyProperties,
    Rename(String),
    SetAppearance(ShapePresentation),
    ToggleVisibility(u64),
    ToggleLock(u64),
    BringForward,
    SendBackward,
    Group,
    Ungroup,
    ToggleGrid,
    ToggleSnap,
    ZoomOut,
    ZoomReset,
    ZoomIn,
    ZoomFit,
    Save,
    SaveAs,
    Open,
    DiscardAndOpen,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolMode {
    #[default]
    Selection,
    Marquee,
    Rectangle,
    Ellipse,
    SolidConnection,
    DashedConnection,
}

impl ToolMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Selection => "Select",
            Self::Marquee => "Marquee",
            Self::Rectangle => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::SolidConnection => "Solid link",
            Self::DashedConnection => "Dashed link",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutlineItemKind {
    Rectangle,
    Ellipse,
    Connection {
        style: EdgeStyle,
        source: u64,
        target: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineItem {
    pub id: u64,
    pub title: String,
    pub kind: OutlineItemKind,
    pub visible: bool,
    pub locked: bool,
    pub group: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupItem {
    pub id: u64,
    pub title: String,
    pub members: Vec<u64>,
}

pub struct AppState {
    pub pending: VecDeque<UiCommand>,
    pub canvas: CanvasAction,
    pub disabled: bool,
    pub can_align: bool,
    pub can_delete: bool,
    pub can_group: bool,
    pub can_ungroup: bool,
    pub can_bring_forward: bool,
    pub can_send_backward: bool,
    pub tool_mode: ToolMode,
    pub outline: Vec<OutlineItem>,
    pub groups: Vec<GroupItem>,
    pub selected: Vec<u64>,
    pub properties: [f64; 4],
    pub name_draft: String,
    pub presentation_draft: Option<ShapePresentation>,
    pub geometry_dirty: bool,
    pub appearance_dirty: bool,
    pub path_input: String,
    pub current_path: Option<PathBuf>,
    pub canvas_bounds: Rectangle,
    pub(crate) property_subject: Option<(u64, Rectangle)>,
    name_subject: Option<(u64, String)>,
    presentation_subject: Option<(u64, ShapePresentation)>,
    route_obstacles: Vec<FigureId>,
    pub message: Option<String>,
}
impl Default for AppState {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            canvas: CanvasAction::default(),
            disabled: false,
            can_align: false,
            can_delete: false,
            can_group: false,
            can_ungroup: false,
            can_bring_forward: false,
            can_send_backward: false,
            tool_mode: ToolMode::default(),
            outline: Vec::new(),
            groups: Vec::new(),
            selected: Vec::new(),
            properties: Default::default(),
            name_draft: String::new(),
            presentation_draft: None,
            geometry_dirty: false,
            appearance_dirty: false,
            path_input: "shapes.json".into(),
            current_path: None,
            canvas_bounds: Rectangle::new(0.0, 0.0, 0.0, 0.0),
            property_subject: None,
            name_subject: None,
            presentation_subject: None,
            route_obstacles: Vec::new(),
            message: None,
        }
    }
}

/// Single application action queue; rebuild drains each request exactly once.
pub fn sync(app: &mut AppState, session: &mut Session) -> Result<bool, String> {
    let mut changed = false;
    while let Some(command) = app.pending.pop_front() {
        let result = apply_command(app, session, command);
        if let Err(error) = result {
            app.canvas = session.snapshot(Some(error.to_string()));
            return Err(error.to_string());
        }
        changed = true;
    }
    let next_obstacles = obstacles(session);
    if app.route_obstacles != next_obstacles {
        update_router(session)?;
        app.route_obstacles = next_obstacles;
        changed = true;
    }
    changed |= session.sync_selection_handles()?;
    reconcile_tool_mode(app, session);
    project(app, session, app.canvas_bounds);
    if changed {
        app.canvas = session.snapshot(None);
    }
    Ok(changed)
}

fn apply_command(
    app: &mut AppState,
    session: &mut Session,
    command: UiCommand,
) -> Result<(), String> {
    app.message = None;
    match command {
        UiCommand::Undo => session
            .domain
            .undo(&mut session.viewer)
            .map_err(|e| e.to_string()),
        UiCommand::Redo => session
            .domain
            .redo(&mut session.viewer)
            .map_err(|e| e.to_string()),
        UiCommand::SelectTool => {
            session
                .domain
                .cancel_tool(&mut session.viewer)
                .map_err(|e| e.to_string())?;
            app.tool_mode = ToolMode::Selection;
            Ok(())
        }
        UiCommand::Marquee => {
            session
                .domain
                .set_active_tool(&mut session.viewer, MarqueeTool::new())
                .map_err(|e| e.to_string())?;
            app.tool_mode = ToolMode::Marquee;
            Ok(())
        }
        command @ (UiCommand::Rectangle | UiCommand::Ellipse) => {
            let kind = if matches!(command, UiCommand::Rectangle) {
                ShapeKind::Rectangle
            } else {
                ShapeKind::Ellipse
            };
            session
                .domain
                .set_active_tool(&mut session.viewer, CreationTool::new(ShapeTemplate(kind)))
                .map_err(|e| e.to_string())?;
            app.tool_mode = match kind {
                ShapeKind::Rectangle => ToolMode::Rectangle,
                ShapeKind::Ellipse => ToolMode::Ellipse,
            };
            Ok(())
        }
        UiCommand::Connect(style) => {
            session
                .domain
                .activate_connection_creation(
                    &mut session.viewer,
                    CreationType::new(style.name()).unwrap(),
                )
                .map_err(|e| e.to_string())?;
            app.tool_mode = match style {
                EdgeStyle::Solid => ToolMode::SolidConnection,
                EdgeStyle::Dashed => ToolMode::DashedConnection,
            };
            Ok(())
        }
        UiCommand::Drop(kind, location) => {
            session
                .domain
                .drop_template(
                    &mut session.viewer,
                    &ShapeTemplate(kind),
                    location,
                    RequestModifiers::default(),
                )
                .map_err(|e| e.to_string())?;
            app.tool_mode = ToolMode::Selection;
            Ok(())
        }
        UiCommand::Select(id) => {
            session
                .domain
                .cancel_tool(&mut session.viewer)
                .map_err(|e| e.to_string())?;
            let part = session
                .viewer
                .part_for_model(id)
                .or_else(|| {
                    session
                        .viewer
                        .connection_part_for_model(id)
                        .map(|part| part.edit_part())
                })
                .ok_or("outline item no longer exists")?;
            session
                .viewer
                .replace_selection(part)
                .map_err(|e| e.to_string())?;
            app.tool_mode = ToolMode::Selection;
            Ok(())
        }
        UiCommand::SelectGroup(id) => {
            session
                .domain
                .cancel_tool(&mut session.viewer)
                .map_err(|e| e.to_string())?;
            let members = session
                .viewer
                .model()
                .groups
                .get(&id)
                .ok_or("group no longer exists")?
                .members
                .clone();
            let parts = members
                .iter()
                .filter_map(|member| session.viewer.part_for_model(*member))
                .collect::<Vec<_>>();
            session
                .viewer
                .set_selection(&parts)
                .map_err(|e| e.to_string())?;
            app.tool_mode = ToolMode::Selection;
            Ok(())
        }
        UiCommand::Delete => {
            let parts = session
                .viewer
                .selection()
                .items()
                .iter()
                .copied()
                .filter(|part| {
                    session
                        .viewer
                        .parts()
                        .get(*part)
                        .and_then(|part| part.model_id())
                        .is_none_or(|id| !session.viewer.model().is_locked(id))
                })
                .collect::<Vec<_>>();
            if parts.is_empty() {
                return Ok(());
            }
            let request = DeleteRequest::new(
                parts,
                session
                    .domain
                    .next_interaction_revision()
                    .map_err(|e| e.to_string())?,
            );
            let metadata = RequestMetadata::new(request.revision(), request.parts().to_vec());
            session
                .domain
                .execute::<DeleteParts, _>(&mut session.viewer, &metadata, &request)
                .map_err(|e| e.to_string())
        }
        UiCommand::Align(alignment) => {
            if app.disabled {
                return Ok(());
            }
            alignment::align(session, alignment)
        }
        UiCommand::ApplyProperties => {
            let (id, _) = app
                .property_subject
                .ok_or("select a shape to edit geometry")?;
            let values = app.properties;
            if !values.iter().all(|value| value.is_finite()) {
                return Err("geometry values must be finite".into());
            }
            if values[2] < MIN_SIZE || values[3] < MIN_SIZE {
                return Err(format!("width and height must be at least {MIN_SIZE}"));
            }
            let before = session
                .viewer
                .model()
                .shapes
                .get(&id)
                .ok_or("shape no longer exists")?
                .bounds;
            let after = Rectangle::new(values[0], values[1], values[2], values[3]);
            if before == after {
                return Ok(());
            }
            session
                .domain
                .execute_command(
                    &mut session.viewer,
                    Box::new(MoveShape { id, before, after }),
                )
                .map_err(|e| e.to_string())
        }
        UiCommand::Rename(after) => {
            let (id, _) = app.property_subject.ok_or("select a shape to rename")?;
            let before = session.viewer.model().name(id);
            if before == after.trim() {
                return Ok(());
            }
            session
                .domain
                .execute_command(
                    &mut session.viewer,
                    Box::new(RenameShape { id, before, after }),
                )
                .map_err(|e| e.to_string())
        }
        UiCommand::SetAppearance(after) => {
            let (id, _) = app.property_subject.ok_or("select a shape to style")?;
            let before = session.viewer.model().presentation(id);
            session
                .domain
                .execute_command(
                    &mut session.viewer,
                    Box::new(UpdateShapePresentation { id, before, after }),
                )
                .map_err(|e| e.to_string())
        }
        UiCommand::ToggleVisibility(id) => {
            let before = session.viewer.model().presentation(id);
            let after = ShapePresentation {
                visible: !before.visible,
                ..before
            };
            session
                .domain
                .execute_command(
                    &mut session.viewer,
                    Box::new(UpdateShapePresentation { id, before, after }),
                )
                .map_err(|e| e.to_string())
        }
        UiCommand::ToggleLock(id) => {
            let before = session.viewer.model().presentation(id);
            let after = ShapePresentation {
                locked: !before.locked,
                ..before
            };
            session
                .domain
                .execute_command(
                    &mut session.viewer,
                    Box::new(UpdateShapePresentation { id, before, after }),
                )
                .map_err(|e| e.to_string())
        }
        command @ (UiCommand::BringForward | UiCommand::SendBackward) => {
            let id = app
                .selected
                .last()
                .copied()
                .ok_or("select a shape to reorder")?;
            if session.viewer.model().is_locked(id) {
                return Ok(());
            }
            let before = session.viewer.model().order.clone();
            let Some(index) = before.iter().position(|candidate| *candidate == id) else {
                return Ok(());
            };
            let target = if matches!(command, UiCommand::BringForward) {
                index.checked_add(1).filter(|next| *next < before.len())
            } else {
                index.checked_sub(1)
            };
            let Some(target) = target else {
                return Ok(());
            };
            let mut after = before.clone();
            after.swap(index, target);
            session
                .domain
                .execute_command(
                    &mut session.viewer,
                    Box::new(ReorderShapes { before, after }),
                )
                .map_err(|e| e.to_string())
        }
        UiCommand::Group => {
            let members = app
                .selected
                .iter()
                .copied()
                .filter(|id| session.viewer.model().shapes.contains_key(id))
                .collect::<Vec<_>>();
            if members.len() < 2 {
                return Ok(());
            }
            let before = session.viewer.model().groups.clone();
            let mut after = before.clone();
            after.retain(|_, group| {
                group.members.retain(|id| !members.contains(id));
                !group.members.is_empty()
            });
            let id = session.viewer.model().next_identity();
            after.insert(
                id,
                ShapeGroup {
                    name: format!("Group #{id}"),
                    members,
                },
            );
            session
                .domain
                .execute_command(
                    &mut session.viewer,
                    Box::new(ReplaceGroups { before, after }),
                )
                .map_err(|e| e.to_string())
        }
        UiCommand::Ungroup => {
            let before = session.viewer.model().groups.clone();
            let mut after = before.clone();
            after.retain(|_, group| {
                !group
                    .members
                    .iter()
                    .any(|member| app.selected.contains(member))
            });
            if before == after {
                return Ok(());
            }
            session
                .domain
                .execute_command(
                    &mut session.viewer,
                    Box::new(ReplaceGroups { before, after }),
                )
                .map_err(|e| e.to_string())
        }
        UiCommand::ToggleGrid => {
            session.set_grid_visible(!session.workspace.get().show_grid);
            Ok(())
        }
        UiCommand::ToggleSnap => {
            session.set_snap_enabled(!session.workspace.get().snap_to_grid);
            Ok(())
        }
        UiCommand::ZoomOut => session.zoom_by_step(-1),
        UiCommand::ZoomReset => session.set_zoom(1.0),
        UiCommand::ZoomIn => session.zoom_by_step(1),
        UiCommand::ZoomFit => session.fit_visible_shapes(),
        command @ (UiCommand::Save | UiCommand::SaveAs) => {
            let path = if matches!(command, UiCommand::Save) {
                app.current_path
                    .clone()
                    .unwrap_or_else(|| PathBuf::from(&app.path_input))
            } else {
                PathBuf::from(&app.path_input)
            };
            let data = session.viewer.model().to_json()?;
            write_document(&path, data.as_bytes())?;
            session
                .domain
                .mark_save_location()
                .map_err(|e| e.to_string())?;
            app.path_input = path.to_string_lossy().into_owned();
            app.current_path = Some(path);
            app.message = Some("Saved".into());
            Ok(())
        }
        command @ (UiCommand::Open | UiCommand::DiscardAndOpen) => {
            if session.domain.is_dirty() && matches!(command, UiCommand::Open) {
                return Err("Unsaved changes: save first, or use Discard & open".into());
            }
            let path = PathBuf::from(&app.path_input);
            let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let model = Diagram::from_json(&json)?;
            let replacement = session_from_model_with_workspace(model, session.workspace.clone())?;
            session.cancel()?;
            *session = replacement;
            app.route_obstacles.clear();
            app.property_subject = None;
            app.name_subject = None;
            app.presentation_subject = None;
            app.tool_mode = ToolMode::Selection;
            app.current_path = Some(path);
            app.message = Some("Opened".into());
            Ok(())
        }
    }
}

fn write_document(path: &Path, data: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or("choose a document file path")?
        .to_string_lossy();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temporary = parent.join(format!(".{name}.{}.{}.tmp", std::process::id(), nonce));
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(data)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(|e| e.to_string())
}

fn reconcile_tool_mode(app: &mut AppState, session: &Session) {
    app.tool_mode = match session.domain.active_tool_key().name() {
        "marquee" => ToolMode::Marquee,
        "creation" => match app.tool_mode {
            ToolMode::Rectangle | ToolMode::Ellipse => app.tool_mode,
            _ => ToolMode::Selection,
        },
        "connection-creation" => match app.tool_mode {
            ToolMode::SolidConnection | ToolMode::DashedConnection => app.tool_mode,
            _ => ToolMode::SolidConnection,
        },
        _ => ToolMode::Selection,
    };
}

fn project(app: &mut AppState, session: &Session, bounds: Rectangle) {
    app.canvas_bounds = bounds;
    app.can_align = alignment::can_align(session);
    app.selected = session
        .viewer
        .selection()
        .items()
        .iter()
        .filter_map(|id| session.viewer.parts().get(*id).and_then(|p| p.model_id()))
        .collect();
    let model = session.viewer.model();
    let selected_shapes = app
        .selected
        .iter()
        .copied()
        .filter(|id| model.shapes.contains_key(id))
        .collect::<Vec<_>>();
    app.can_delete = app
        .selected
        .iter()
        .any(|id| model.shapes.get(id).is_none_or(|_| !model.is_locked(*id)));
    app.can_group =
        selected_shapes.len() >= 2 && selected_shapes.iter().all(|id| !model.is_locked(*id));
    app.can_ungroup = model.groups.values().any(|group| {
        group
            .members
            .iter()
            .any(|member| selected_shapes.contains(member))
    });
    let primary_index = selected_shapes
        .last()
        .and_then(|id| model.order.iter().position(|candidate| candidate == id));
    let primary_unlocked = selected_shapes
        .last()
        .is_some_and(|id| !model.is_locked(*id));
    app.can_bring_forward =
        primary_unlocked && primary_index.is_some_and(|index| index + 1 < model.order.len());
    app.can_send_backward = primary_unlocked && primary_index.is_some_and(|index| index > 0);
    app.groups = model
        .groups
        .iter()
        .map(|(id, group)| GroupItem {
            id: *id,
            title: group.name.clone(),
            members: group.members.clone(),
        })
        .collect();
    app.outline = model
        .order
        .iter()
        .map(|id| {
            let kind = match model.shapes[id].kind {
                ShapeKind::Rectangle => OutlineItemKind::Rectangle,
                ShapeKind::Ellipse => OutlineItemKind::Ellipse,
            };
            let presentation = model.presentation(*id);
            OutlineItem {
                id: *id,
                title: model.name(*id),
                kind,
                visible: presentation.visible,
                locked: presentation.locked,
                group: model
                    .groups
                    .iter()
                    .find_map(|(group_id, group)| group.members.contains(id).then_some(*group_id)),
            }
        })
        .chain(model.edges.iter().map(|(id, edge)| OutlineItem {
            id: *id,
            title: format!(
                "{} #{id}",
                match edge.style {
                    EdgeStyle::Solid => "Solid",
                    EdgeStyle::Dashed => "Dashed",
                }
            ),
            kind: OutlineItemKind::Connection {
                style: edge.style,
                source: edge.source,
                target: edge.target,
            },
            visible: model.presentation(edge.source).visible
                && model.presentation(edge.target).visible,
            locked: false,
            group: None,
        }))
        .collect();
    let subject = app
        .selected
        .last()
        .and_then(|id| model.shapes.get(id).map(|shape| (*id, shape.bounds)));
    if app.property_subject != subject {
        app.property_subject = subject;
        app.properties =
            subject.map_or_else(Default::default, |(_, b)| [b.x, b.y, b.width, b.height]);
        app.geometry_dirty = false;
    }
    let name_subject = subject.map(|(id, _)| (id, model.name(id)));
    if app.name_subject != name_subject {
        app.name_subject.clone_from(&name_subject);
        app.name_draft = name_subject.map_or_else(String::new, |(_, name)| name);
    }
    let presentation_subject = subject.map(|(id, _)| (id, model.presentation(id)));
    if app.presentation_subject != presentation_subject {
        app.presentation_subject = presentation_subject;
        app.presentation_draft = presentation_subject.map(|(_, presentation)| presentation);
        app.appearance_dirty = false;
    }
}
