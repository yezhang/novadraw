#![cfg(all(feature = "serde", feature = "testing"))]

use novadraw_ui::{
    docking::{
        Action, DockContent, DockError, Floating, Layout, Node, Permissions, Workspace,
        testing::DockDriver,
    },
    layout::{SplitAxis, SplitSize},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
struct Panel(String);

fn panel(name: &str) -> Panel {
    Panel(name.into())
}

fn tabs(names: &[&str]) -> Node<Panel> {
    Node::Tabs {
        panels: names.iter().map(|name| panel(name)).collect(),
        active: 0,
    }
}

fn layout() -> Layout<Panel> {
    Layout {
        root: Some(Node::Split {
            axis: SplitAxis::Horizontal,
            size: SplitSize::Ratio(0.7),
            first: Box::new(tabs(&["canvas"])),
            second: Box::new(tabs(&["plugin.layers", "plugin.inspector"])),
        }),
        floating: Vec::new(),
    }
}

fn permissions(panel: &Panel) -> Permissions {
    if panel.0 == "canvas" {
        Permissions::PROTECTED
    } else {
        Permissions::default()
    }
}

#[derive(Default)]
struct ExternalContent {
    drawn: Vec<Panel>,
}

impl DockContent<Panel> for ExternalContent {
    fn body(&mut self, ui: &mut egui::Ui, panel: &Panel) {
        self.drawn.push(panel.clone());
        ui.label(&panel.0);
    }

    fn logical_label(&self, panel: &Panel) -> String {
        panel.0.clone()
    }

    fn permissions(&self, panel: &Panel) -> Permissions {
        permissions(panel)
    }
}

#[test]
fn application_panel_identity_renders_without_internal_registration() {
    let driver = DockDriver::new(layout(), ExternalContent::default());
    assert!(driver.content.drawn.contains(&panel("canvas")));
    assert!(driver.content.drawn.contains(&panel("plugin.layers")));
    driver.layout.validate().unwrap();
}

#[test]
fn rejected_protected_canvas_action_rolls_back_entire_batch() {
    let mut layout = layout();
    let original = layout.clone();
    let result = layout.apply_all(
        [
            Action::Close {
                panel: panel("plugin.layers"),
            },
            Action::Close {
                panel: panel("canvas"),
            },
        ],
        permissions,
    );
    assert!(result.is_err());
    assert_eq!(layout, original);
}

#[test]
fn external_panel_hide_restore_roundtrips_persisted_workspace() {
    let mut workspace = Workspace::new(layout());
    workspace
        .set_visible(panel("plugin.layers"), false, None, permissions)
        .unwrap();
    let bytes = serde_json::to_vec(&workspace).unwrap();
    let mut restored: Workspace<Panel> = serde_json::from_slice(&bytes).unwrap();
    restored.validate().unwrap();
    restored
        .set_visible(
            panel("plugin.layers"),
            true,
            Some(panel("plugin.inspector")),
            permissions,
        )
        .unwrap();
    restored.validate().unwrap();
    assert!(
        restored
            .layout
            .panels()
            .into_iter()
            .any(|id| id == &panel("plugin.layers"))
    );
}

#[test]
fn duplicate_native_window_identity_is_rejected() {
    let first = Floating::new(tabs(&["plugin.one"]), [10.0, 10.0, 200.0, 200.0]);
    let mut second = Floating::new(tabs(&["plugin.two"]), [220.0, 10.0, 200.0, 200.0]);
    second.id = first.id;
    let layout = Layout {
        root: None,
        floating: vec![first, second],
    };
    assert_eq!(layout.validate(), Err(DockError::DuplicateWindow));
}
