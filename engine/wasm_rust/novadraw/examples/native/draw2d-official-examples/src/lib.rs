//! Source-faithful ports of the runnable `org.eclipse.draw2d.examples` entries.

#[cfg(test)]
mod controls;
mod examples;

use novadraw::runtime::Runtime;

pub type SceneEntry = (&'static str, Box<dyn FnMut() -> Runtime>);

/// Representative official inputs for Native visual verification.
pub fn graph_verification_entries() -> Vec<SceneEntry> {
    examples::graph_verification_entries()
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
}

pub struct OfficialExample {
    pub id: &'static str,
    pub name: &'static str,
    pub upstream_path: &'static str,
    pub status: ParityStatus,
    pub gaps: &'static [&'static str],
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
            factory: Some(factory),
        }
    }

    const fn blocked(
        id: &'static str,
        name: &'static str,
        upstream_path: &'static str,
        gaps: &'static [&'static str],
    ) -> Self {
        Self {
            id,
            name,
            upstream_path,
            status: ParityStatus::Blocked,
            gaps,
            factory: None,
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
        OfficialExample::in_progress(
            "connection-and-anchor",
            "ConnectionAndAnchorExample",
            "org/eclipse/draw2d/examples/connections/ConnectionAndAnchorExample.java",
            &[
                "independent Figure/anchor/locator fixture and drag tests exist; upstream visual comparison remains pending",
            ],
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
                "Canvas shape outlines/colors, Flow measurement and resize remain under verification",
            ],
            examples::flow_layout_example,
        ),
        OfficialExample::in_progress(
            "grid-layout",
            "GridLayoutExample",
            "org/eclipse/draw2d/examples/layouts/GridLayoutExample.java",
            &[
                "Canvas GridData measurement, span/grab and dynamic resize remain under verification",
            ],
            examples::grid_layout_example,
        ),
        OfficialExample::in_progress(
            "toolbar-layout",
            "ToolbarLayoutExample",
            "org/eclipse/draw2d/examples/layouts/ToolbarLayoutExample.java",
            &[
                "Canvas shape outlines, Toolbar measurement and stretch/reset behavior remain under verification",
            ],
            examples::toolbar_layout_example,
        ),
        OfficialExample::in_progress(
            "path",
            "PathExample",
            "org/eclipse/draw2d/examples/path/PathExample.java",
            &[
                "Canvas path, rotation, selection and range zoom are ported; SchemeBorder and font-derived underline metrics remain pending",
            ],
            examples::path_example,
        ),
        OfficialExample::blocked(
            "printing",
            "PrintExample",
            "org/eclipse/draw2d/examples/printing/PrintExample.java",
            &[
                "official Canvas Figure composition and graphics output consumption remain to be ported",
            ],
        ),
        OfficialExample::in_progress(
            "scroll-pane",
            "ScrollPaneExample",
            "org/eclipse/draw2d/examples/scrollpane/ScrollPaneExample.java",
            &[
                "InternalFrame drag/resize, GroupBoxBorder, ExampleUtil composition and dynamic resize cursor transitions are ported",
                "Core ScrollBar child composition/theme and upstream visual comparison remain pending",
            ],
            examples::scroll_pane_example,
        ),
        OfficialExample::blocked(
            "inherit-background",
            "InheritBackgroundExample",
            "org/eclipse/draw2d/examples/swt/InheritBackgroundExample.java",
            &["transparent Canvas background-source composition remains to be implemented"],
        ),
        OfficialExample::blocked(
            "shape-styles",
            "ShapeStylesExample",
            "org/eclipse/draw2d/examples/swt/ShapeStylesExample.java",
            &[
                "backend-neutral StrokeStyle has no per-shape ON/OFF/DEFAULT antialias mode",
                "Render IR and Vello lowering cannot reproduce the SWT.OFF matrix cells",
            ],
        ),
        OfficialExample::complete(
            "bidi-block",
            "BidiBlockExample",
            "org/eclipse/draw2d/examples/text/BidiBlockExample.java",
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
        OfficialExample::blocked(
            "thumbnail",
            "ThumbnailExample",
            "org/eclipse/draw2d/examples/thumbnail/ThumbnailExample.java",
            &["live secondary projection of one mutable Figure tree is absent"],
        ),
        OfficialExample::in_progress(
            "simple-tree",
            "SimpleTreeExample",
            "org/eclipse/draw2d/examples/tree/SimpleTreeExample.java",
            &[
                "official PageNode composition and normal/hanging contour layouts are ported; canvas background, upstream fonts and visual comparison remain pending",
            ],
            examples::simple_tree_example,
        ),
        OfficialExample::in_progress(
            "tree",
            "TreeExample",
            "org/eclipse/draw2d/examples/tree/TreeExample.java",
            &[
                "official nineteen-node fixture, full branch insertion with parent style and commit receipt, selected label white/inherited reset, alignment/style/spacing, deletion and nonanimated collapse are ported",
                "Canvas selected gradient, animation geometry and viewport tracking remain incomplete",
            ],
            examples::tree_example,
        ),
        OfficialExample::in_progress(
            "uml-class-diagram",
            "UMLClassDiagram",
            "org/eclipse/draw2d/examples/uml/UMLClassDiagram.java",
            &[
                "owner-side endpoint label placement is implemented; header/member font metrics and DogEar border still require visual parity verification",
            ],
            examples::uml_class_diagram,
        ),
        OfficialExample::in_progress(
            "zoom",
            "ZoomExample",
            "org/eclipse/draw2d/examples/zoom/ZoomExample.java",
            &[
                "ScrollBar composition/range/repeat are implemented; SchemeBorder still requires parity verification",
                "original SVG resources and Canvas rendering remain under verification",
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
            11
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
