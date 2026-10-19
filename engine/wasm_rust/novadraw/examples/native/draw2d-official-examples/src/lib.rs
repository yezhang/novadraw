//! Source-faithful ports of the runnable `org.eclipse.draw2d.examples` entries.

mod controls;
mod examples;

use novadraw::runtime::Runtime;

pub type SceneEntry = (&'static str, Box<dyn FnMut() -> Runtime>);

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
    const fn not_implemented(
        id: &'static str,
        name: &'static str,
        upstream_path: &'static str,
        gaps: &'static [&'static str],
    ) -> Self {
        Self {
            id,
            name,
            upstream_path,
            status: ParityStatus::NotImplemented,
            gaps,
            factory: None,
        }
    }

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
        OfficialExample::not_implemented(
            "obstruction",
            "ObstructionExample",
            "org/eclipse/draw2d/examples/cg/ObstructionExample.java",
            &[
                "requires source-faithful dynamic obstacle and endpoint lifecycle",
                "official visibility-graph paths and shared-corner spacing differ from Core's orthogonal ShortestPathConnectionRouter; the official algorithm must be ported",
            ],
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
        OfficialExample::blocked(
            "compound-graph",
            "CompoundGraphDemo",
            "org/eclipse/draw2d/examples/graph/CompoundGraphDemo.java",
            &["P2-L01 CompoundDirectedGraph layout is not implemented"],
        ),
        OfficialExample::blocked(
            "directed-graph",
            "DirectedGraphDemo",
            "org/eclipse/draw2d/examples/graph/DirectedGraphDemo.java",
            &["P2-L01 DirectedGraph layout is not implemented"],
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
                "official shape outlines/colors, native Scale keyboard semantics and resizable canvas layout remain under verification",
            ],
            examples::flow_layout_example,
        ),
        OfficialExample::in_progress(
            "grid-layout",
            "GridLayoutExample",
            "org/eclipse/draw2d/examples/layouts/GridLayoutExample.java",
            &[
                "editable SWT Combo and Text selection/caret semantics are not yet equivalent; shell.pack resizing remains to be ported",
            ],
            examples::grid_layout_example,
        ),
        OfficialExample::in_progress(
            "toolbar-layout",
            "ToolbarLayoutExample",
            "org/eclipse/draw2d/examples/layouts/ToolbarLayoutExample.java",
            &[
                "official shape outlines, stretch/reset behavior and native Scale keyboard semantics remain under verification",
            ],
            examples::toolbar_layout_example,
        ),
        OfficialExample::blocked(
            "path",
            "PathExample",
            "org/eclipse/draw2d/examples/path/PathExample.java",
            &[
                "requires ScrollBar-driven scaling",
                "requires fragment-level SWT TextLayout styling",
            ],
        ),
        OfficialExample::blocked(
            "printing",
            "PrintExample",
            "org/eclipse/draw2d/examples/printing/PrintExample.java",
            &["P2-O01 printing/export output targets are deferred"],
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
            &["platform-native parent background inheritance has no Winit contract"],
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
        OfficialExample::blocked(
            "bidi-block",
            "BidiBlockExample",
            "org/eclipse/draw2d/examples/text/BidiBlockExample.java",
            &[
                "per-block base direction and ALWAYS_LEFT/ALWAYS_RIGHT are absent",
                "shared vertical RangeModel across two canvases is absent",
            ],
        ),
        OfficialExample::blocked(
            "caret",
            "CaretExample",
            "org/eclipse/draw2d/examples/text/CaretExample.java",
            &["requires a Native caret overlay and live hit inspector"],
        ),
        OfficialExample::blocked(
            "multi-line-label",
            "MultiLineLabelExample",
            "org/eclipse/draw2d/examples/text/MultiLineLabelExample.java",
            &["MultiLineLabel widget and native Text comparison are absent"],
        ),
        OfficialExample::blocked(
            "text-flow",
            "TextFlowExample",
            "org/eclipse/draw2d/examples/text/TextFlowExample.java",
            &["P2-T03 fragment fonts, colors, baselines, and nested InlineFlow are deferred"],
        ),
        OfficialExample::blocked(
            "text-flow-large",
            "TextFlowLargeExample",
            "org/eclipse/draw2d/examples/text/TextFlowLargeExample.java",
            &["requires mutable fragment text and fragment-level fonts"],
        ),
        OfficialExample::blocked(
            "text-surrounding-figure",
            "TextSurroundingFigureExample",
            "org/eclipse/draw2d/examples/text/TextSurroundingFigureExample.java",
            &["FlowAdapter-style inline Figure subtrees are absent"],
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
                "official nineteen-node fixture, alignment/style/spacing, deletion and nonanimated collapse are ported",
                "child insertion, selected gradient/label colors, animation viewport tracking and native panel layout remain incomplete; Scale uses integer values and increment=5",
            ],
            examples::tree_example,
        ),
        OfficialExample::in_progress(
            "uml-class-diagram",
            "UMLClassDiagram",
            "org/eclipse/draw2d/examples/uml/UMLClassDiagram.java",
            &[
                "ConnectionEndpointLocator owner-side and child-size placement is absent; header/member font metrics and DogEar border still require visual parity verification",
            ],
            examples::uml_class_diagram,
        ),
        OfficialExample::in_progress(
            "zoom",
            "ZoomExample",
            "org/eclipse/draw2d/examples/zoom/ZoomExample.java",
            &[
                "ScrollBar repeat firing, five child Figure composition and SchemeBorder remain to be ported",
                "SVG icons are currently represented by upstream PNG alternatives; native visual comparison is pending",
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn catalog_is_the_exact_official_main_denominator() {
        let catalog = catalog();
        assert_eq!(catalog.len(), 25);
        assert_eq!(
            catalog
                .iter()
                .filter(|entry| entry.status == ParityStatus::Complete)
                .count(),
            2
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
