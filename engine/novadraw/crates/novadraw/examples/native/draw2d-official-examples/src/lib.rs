//! Source-faithful ports of the runnable `org.eclipse.draw2d.examples` entries.

#[cfg(test)]
mod controls;
mod examples;
pub mod inherit_background;

/// Runs fixed-time Tree animation evidence on a native Vello device.
pub fn verify_tree_animation(window: std::sync::Arc<winit::window::Window>) {
    examples::tree_animation::verify_native(window);
}

/// Verifies the actual retained Vello target for layout resize, erasure and caret hover.
pub fn verify_layout_consumers(window: std::sync::Arc<winit::window::Window>) {
    examples::consumer_verification::verify_native(window);
}

use novadraw::runtime::Runtime;

pub type SceneEntry = (&'static str, Box<dyn FnMut() -> Runtime>);

/// Stable, one-based labels and parity state from the complete official catalog.
pub fn official_scene_title(name: &str) -> String {
    let entries = catalog();
    entries
        .iter()
        .position(|entry| entry.name == name)
        .map_or_else(
            || name.to_string(),
            |index| {
                let entry = &entries[index];
                format!(
                    "[{:02}/{} · {}] {name}",
                    index + 1,
                    entries.len(),
                    entry.status.title_label()
                )
            },
        )
}

/// Representative official inputs for Native visual verification.
pub fn graph_verification_entries() -> Vec<SceneEntry> {
    examples::graph_verification_entries()
}

/// Initial, dragged/resized and closed shared-view Native evidence.
pub fn thumbnail_verification_entries() -> Vec<novadraw_example_support::panel::PanelSceneEntry> {
    examples::thumbnail_verification_entries()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParityStatus {
    Complete,
    InProgress,
    NotImplemented,
    Blocked,
}

impl ParityStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::InProgress => "in_progress",
            Self::NotImplemented => "not_implemented",
            Self::Blocked => "blocked",
        }
    }

    pub const fn title_label(self) -> &'static str {
        match self {
            Self::Complete => "已完成",
            Self::InProgress => "进行中",
            Self::NotImplemented => "未实现",
            Self::Blocked => "阻塞",
        }
    }
}

pub struct OfficialExample {
    pub id: &'static str,
    pub name: &'static str,
    pub upstream_path: &'static str,
    pub status: ParityStatus,
    pub gaps: &'static [&'static str],
    /// Dedicated Native host for entries requiring multiple independent Canvases.
    pub native_binary: Option<&'static str>,
    factory: Option<fn() -> Runtime>,
}

impl OfficialExample {
    const fn in_progress(
        id: &'static str,
        name: &'static str,
        upstream_path: &'static str,
        gaps: &'static [&'static str],
        factory: fn() -> Runtime,
    ) -> Self {
        Self {
            id,
            name,
            upstream_path,
            status: ParityStatus::InProgress,
            gaps,
            native_binary: None,
            factory: Some(factory),
        }
    }

    const fn complete(
        id: &'static str,
        name: &'static str,
        upstream_path: &'static str,
        factory: fn() -> Runtime,
    ) -> Self {
        Self {
            id,
            name,
            upstream_path,
            status: ParityStatus::Complete,
            gaps: &[],
            native_binary: None,
            factory: Some(factory),
        }
    }
}

pub fn catalog() -> Vec<OfficialExample> {
    vec![
        OfficialExample::complete(
            "hello-world",
            "HelloWorld",
            "org/eclipse/draw2d/examples/HelloWorld.java",
            examples::hello_world,
        ),
        OfficialExample::complete(
            "obstruction",
            "ObstructionExample",
            "org/eclipse/draw2d/examples/cg/ObstructionExample.java",
            examples::obstruction,
        ),
        OfficialExample::complete(
            "connection-and-anchor",
            "ConnectionAndAnchorExample",
            "org/eclipse/draw2d/examples/connections/ConnectionAndAnchorExample.java",
            examples::connection_and_anchor,
        ),
        OfficialExample::complete(
            "compound-graph",
            "CompoundGraphDemo",
            "org/eclipse/draw2d/examples/graph/CompoundGraphDemo.java",
            examples::compound_graph,
        ),
        OfficialExample::complete(
            "directed-graph",
            "DirectedGraphDemo",
            "org/eclipse/draw2d/examples/graph/DirectedGraphDemo.java",
            examples::directed_graph,
        ),
        OfficialExample::complete(
            "ellipse-hitting",
            "EllipseHitting",
            "org/eclipse/draw2d/examples/hittest/EllipseHitting.java",
            examples::ellipse_hitting,
        ),
        OfficialExample::in_progress(
            "flow-layout",
            "FlowLayoutExample",
            "org/eclipse/draw2d/examples/layouts/FlowLayoutExample.java",
            &[
                "Official shape spacing/alignment/wrap, host viewport resize, stretch/restore and Native GPU erasure are verified",
                "Supplemental Label/FlowPage, retained erasure, macOS window resize and representative mouse controls are verified; cross-platform pixels remain pending; Flow control-cycle screen RGB differs by at most 1",
            ],
            examples::flow_layout_example,
        ),
        OfficialExample::in_progress(
            "grid-layout",
            "GridLayoutExample",
            "org/eclipse/draw2d/examples/layouts/GridLayoutExample.java",
            &[
                "Canvas now follows host viewport resize; spanning/grabbing shape shrink/grow/reset geometry is verified",
                "Real-panel hints/indent/span/grab, selection release, zero-space restore and Native GPU clearing are verified",
                "Text remeasurement, span/indent/height hints and retained erasure are verified; macOS window drag, mouse Add/Clear/selection/span/grab/Fill and restored pixels are verified; cross-platform pixels remain pending",
            ],
            examples::grid_layout_example,
        ),
        OfficialExample::in_progress(
            "toolbar-layout",
            "ToolbarLayoutExample",
            "org/eclipse/draw2d/examples/layouts/ToolbarLayoutExample.java",
            &[
                "Official shape outlines and stretch/orientation/shrink/reset geometry are verified",
                "Real-panel alignment, host viewport and zero-space restore, and Native GPU erasure are verified",
                "Supplemental Label/FlowPage and retained erasure are verified; macOS window drag, mouse orientation/alignment/stretch/spacing and restored pixels are verified; cross-platform pixels remain pending",
            ],
            examples::toolbar_layout_example,
        ),
        OfficialExample::complete(
            "path",
            "PathExample",
            "org/eclipse/draw2d/examples/path/PathExample.java",
            examples::path_example,
        ),
        OfficialExample::in_progress(
            "printing",
            "PrintExample",
            "org/eclipse/draw2d/examples/printing/PrintExample.java",
            &[
                "Canvas composition, original ICO and graphics output are verified; the host now injects available system faces and converts SWT point heights to logical pixels",
                "macOS adopts Helvetica and Times New Roman; unavailable Perpetua falls back explicitly, while cross-platform font adoption and pixels remain pending",
            ],
            examples::print_example,
        ),
        OfficialExample::in_progress(
            "scroll-pane",
            "ScrollPaneExample",
            "org/eclipse/draw2d/examples/scrollpane/ScrollPaneExample.java",
            &[
                "InternalFrame drag/resize, GroupBoxBorder, ExampleUtil composition and dynamic resize cursor transitions are ported",
                "Core ScrollBar composition and macOS title drag, corner resize, vertical thumb/arrows and auto-hide are verified; wheel, eight-direction cursors, full theme and upstream pixels remain pending",
            ],
            examples::scroll_pane_example,
        ),
        OfficialExample {
            id: "inherit-background",
            name: "InheritBackgroundExample",
            upstream_path: "org/eclipse/draw2d/examples/swt/InheritBackgroundExample.java",
            status: ParityStatus::Complete,
            gaps: &[],
            native_binary: Some("inherit-background"),
            factory: Some(inherit_background::canvas),
        },
        OfficialExample::in_progress(
            "shape-styles",
            "ShapeStylesExample",
            "org/eclipse/draw2d/examples/swt/ShapeStylesExample.java",
            &[
                "Eight axes and five shapes exercise public Graphics ON/OFF/DEFAULT; low miter and the explicit Cocoa zero-width profile are implemented",
                "Upstream native raster, system colors and cross-platform visual comparison remain pending; universal device hairline is an optional separate delta",
            ],
            examples::shape_styles_example,
        ),
        OfficialExample::in_progress(
            "bidi-block",
            "BidiBlockExample",
            "org/eclipse/draw2d/examples/text/BidiBlockExample.java",
            &[
                "Independent ten-block flow trees and shared vertical range are implemented",
                "Inherited Canvas mirroring, caret and alignment are implemented; upstream raster and cross-platform presentation still require acceptance",
            ],
            examples::bidi_block_example,
        ),
        OfficialExample::complete(
            "caret",
            "CaretExample",
            "org/eclipse/draw2d/examples/text/CaretExample.java",
            examples::caret_example,
        ),
        OfficialExample::complete(
            "multi-line-label",
            "MultiLineLabelExample",
            "org/eclipse/draw2d/examples/text/MultiLineLabelExample.java",
            examples::multi_line_label_example,
        ),
        OfficialExample::complete(
            "text-flow",
            "TextFlowExample",
            "org/eclipse/draw2d/examples/text/TextFlowExample.java",
            examples::text_flow_example,
        ),
        OfficialExample::complete(
            "text-flow-large",
            "TextFlowLargeExample",
            "org/eclipse/draw2d/examples/text/TextFlowLargeExample.java",
            examples::text_flow_large_example,
        ),
        OfficialExample::complete(
            "text-surrounding-figure",
            "TextSurroundingFigureExample",
            "org/eclipse/draw2d/examples/text/TextSurroundingFigureExample.java",
            examples::text_surrounding_figure_example,
        ),
        OfficialExample::complete(
            "thumbnail",
            "ThumbnailExample",
            "org/eclipse/draw2d/examples/thumbnail/ThumbnailExample.java",
            examples::thumbnail_example,
        ),
        OfficialExample::complete(
            "simple-tree",
            "SimpleTreeExample",
            "org/eclipse/draw2d/examples/tree/SimpleTreeExample.java",
            examples::simple_tree_example,
        ),
        OfficialExample::complete(
            "tree",
            "TreeExample",
            "org/eclipse/draw2d/examples/tree/TreeExample.java",
            examples::tree_example,
        ),
        OfficialExample::complete(
            "uml-class-diagram",
            "UMLClassDiagram",
            "org/eclipse/draw2d/examples/uml/UMLClassDiagram.java",
            examples::uml_class_diagram,
        ),
        OfficialExample::in_progress(
            "zoom",
            "ZoomExample",
            "org/eclipse/draw2d/examples/zoom/ZoomExample.java",
            &[
                "ScrollBar composition/range/repeat and SchemeBorder are implemented",
                "retained SVG zoom/DPI raster density, shared-resource revision and Native GPU restore are verified; the host adopts available Arial bold at the upstream point size",
                "macOS thumb endpoints, arrow restore, capture across the panel and window resize are verified; cross-display DPI, other-platform fonts and full visual parity remain pending",
            ],
            examples::zoom_example,
        ),
    ]
}

pub fn runnable_entries() -> Vec<SceneEntry> {
    catalog()
        .into_iter()
        .filter_map(|entry| {
            entry
                .factory
                .map(|factory| (entry.name, Box::new(factory) as Box<dyn FnMut() -> Runtime>))
        })
        .collect()
}

pub fn panel_entries() -> Vec<novadraw_example_support::panel::PanelSceneEntry> {
    catalog()
        .into_iter()
        .filter(|entry| entry.native_binary.is_none())
        .filter_map(|entry| {
            entry.factory.map(|factory| {
                let name = entry.name;
                (
                    name,
                    Box::new(move || {
                        examples::panel_scene(name).unwrap_or_else(|| (factory(), None))
                    })
                        as Box<dyn FnMut() -> novadraw_example_support::panel::PanelScene>,
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn hosted_panels_preserve_canvas_without_mock_widgets() {
        for name in [
            "GridLayoutExample",
            "FlowLayoutExample",
            "ToolbarLayoutExample",
            "TreeExample",
        ] {
            let (runtime, panel) = examples::panel_scene(name).expect("panel scene");
            assert!(panel.is_some());
            let root = runtime.tree().contents().expect("Canvas");
            let mut pending = vec![root];
            while let Some(id) = pending.pop() {
                let node = runtime.tree().node(id).expect("Canvas figure");
                assert!(
                    ![
                        "Button",
                        "CheckBox",
                        "RadioButton",
                        "Scale",
                        "Text",
                        "Combo",
                        "Group"
                    ]
                    .contains(&node.figure_name()),
                    "{name}: mock {}",
                    node.figure_name()
                );
                if let Ok(children) = runtime.tree().child_order(id) {
                    pending.extend(children);
                }
            }
        }
    }

    #[test]
    fn catalog_is_the_exact_official_main_denominator() {
        let catalog = catalog();
        assert_eq!(catalog.len(), 25);
        assert_eq!(
            catalog
                .iter()
                .filter(|entry| entry.status == ParityStatus::Complete)
                .count(),
            17
        );

        let mut ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for entry in catalog {
            assert!(ids.insert(entry.id), "duplicate id: {}", entry.id);
            assert!(
                paths.insert(entry.upstream_path),
                "duplicate path: {}",
                entry.upstream_path
            );
            assert_eq!(
                matches!(
                    entry.status,
                    ParityStatus::Complete | ParityStatus::InProgress
                ),
                entry.factory.is_some()
            );
            assert_eq!(
                entry.status == ParityStatus::Complete,
                entry.gaps.is_empty()
            );
        }
    }

    #[test]
    fn scene_titles_include_stable_index_and_chinese_parity_status() {
        assert_eq!(
            official_scene_title("HelloWorld"),
            "[01/25 · 已完成] HelloWorld"
        );
        assert_eq!(
            official_scene_title("FlowLayoutExample"),
            "[07/25 · 进行中] FlowLayoutExample"
        );
        assert_eq!(official_scene_title("unknown"), "unknown");
        assert_eq!(ParityStatus::NotImplemented.title_label(), "未实现");
        assert_eq!(ParityStatus::Blocked.title_label(), "阻塞");
    }

    #[test]
    fn every_runnable_entry_is_owned_by_this_crate() {
        for mut entry in runnable_entries() {
            assert!(
                entry.1().tree().contents().is_some(),
                "{} has no contents",
                entry.0
            );
        }
    }
}
