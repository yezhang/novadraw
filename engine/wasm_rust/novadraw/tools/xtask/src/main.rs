use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{Context, Result, bail};

struct Step {
    program: &'static str,
    args: &'static [&'static str],
}

struct Suite {
    id: &'static str,
    description: &'static str,
    steps: &'static [Step],
}

const SUITES: &[Suite] = &[
    Suite {
        id: "text.long-layout",
        description: "Exact paragraph reuse, source mapping, font invalidation and original fixture CPU smoke",
        steps: &[
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "draw2d-official-examples",
                    "--lib",
                    "long_text_outline_full_partial_gpu_pixels",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "long_text_paint_contract",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--lib",
                    "text::outline::paint_index::tests",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "long_text_layout_contract",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "p2_t03_structured_flow_contract",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "build",
                    "--release",
                    "-p",
                    "draw2d-official-examples",
                    "--bin",
                    "textflow-perf",
                ],
            },
            Step {
                program: "python3",
                args: &[
                    "scripts/textflow_performance.py",
                    "smoke",
                    "target/verification/long-text/smoke",
                ],
            },
        ],
    },
    Suite {
        id: "render.low-miter",
        description: "Low miter values, Cocoa zero-width sample and ON/OFF bevel pixels",
        steps: &[
            Step {
                program: "cargo",
                args: &["test", "-p", "novadraw", "--test", "p2_graphics_contract"],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "draw2d-official-examples",
                    "--lib",
                    "shape_styles",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "draw2d-official-examples",
                    "--lib",
                    "low_miter_pixels_equal",
                    "--",
                    "--ignored",
                ],
            },
        ],
    },
    Suite {
        id: "text.underlines",
        description: "Actual resolved font underline fragments, external consumers and retained pixels",
        steps: &[
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "p2_text_underline_contract",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "font_decoration_image_contract",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "draw2d-official-examples",
                    "--lib",
                    "path_example",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "draw2d-official-examples",
                    "--lib",
                    "underline_resize_and_replace",
                    "--",
                    "--ignored",
                ],
            },
        ],
    },
    Suite {
        id: "text.layout-direction",
        description: "Inherited direction, styled text, flow interaction and shared Bidi panes",
        steps: &[
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "p2_inherited_direction_contract",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "p2_t03_styled_text_contract",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "p2_t03_structured_flow_contract",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "p2_t03_external_flow_engine",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "draw2d-official-examples",
                    "--lib",
                    "bidi_block",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "draw2d-official-examples",
                    "--lib",
                    "bidi_direction_mutation_partial",
                    "--",
                    "--ignored",
                ],
            },
        ],
    },
    Suite {
        id: "render.antialias",
        description: "Per-operation AA public contracts, Native Msaa16 pixels and ShapeStyles axes",
        steps: &[
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "per_operation_antialias_contract",
                ],
            },
            Step {
                program: "cargo",
                args: &["test", "-p", "novadraw-backend-vello", "--lib", "antialias"],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "draw2d-official-examples",
                    "--lib",
                    "shape_styles",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "draw2d-official-examples",
                    "--lib",
                    "shape_styles_native_matrix_pixels",
                    "--",
                    "--ignored",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "check",
                    "-p",
                    "novadraw-backend-vello",
                    "--lib",
                    "--target",
                    "wasm32-unknown-unknown",
                    "--features",
                    "web",
                ],
            },
        ],
    },
    Suite {
        id: "shape.independent-corners",
        description: "Independent corners, rectangle edge colors, legacy shapes and Native GPU seams",
        steps: &[
            Step {
                program: "cargo",
                args: &["test", "-p", "novadraw", "--lib", "free_end_tests"],
            },
            Step {
                program: "cargo",
                args: &["test", "-p", "novadraw", "--lib", "contour_cache_tests"],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "independent_shape_corners",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "m10_reusable_shape_border_contract",
                ],
            },
            Step {
                program: "cargo",
                args: &["test", "-p", "novadraw-backend-vello", "--lib", "antialias"],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw-backend-vello",
                    "--features",
                    "native",
                    "--test",
                    "independent_shape_pixels",
                    "--",
                    "--ignored",
                ],
            },
            Step {
                program: "cargo",
                args: &["check", "-p", "composite-shapes-app"],
            },
            Step {
                program: "cargo",
                args: &[
                    "check",
                    "-p",
                    "novadraw",
                    "--lib",
                    "--target",
                    "wasm32-unknown-unknown",
                ],
            },
        ],
    },
    Suite {
        id: "render.antialias-cost",
        description: "Release CPU preparation/encoding and cache payload costs for 25-cell AA scenes",
        steps: &[Step {
            program: "cargo",
            args: &[
                "test",
                "--release",
                "-p",
                "novadraw-backend-vello",
                "--lib",
                "antialias_release_cost",
                "--",
                "--ignored",
                "--nocapture",
            ],
        }],
    },
    Suite {
        id: "example.zoom-input",
        description: "Floating panel ownership and native Zoom ScrollBar capture lifecycle",
        steps: &[
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw-example-support",
                    "--features",
                    "egui-panel",
                    "--lib",
                    "panel::tests",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "draw2d-official-examples",
                    "--lib",
                    "zoom_example",
                ],
            },
        ],
    },
    Suite {
        id: "animation.tree",
        description: "Sampled geometry, reservations, Tree operations and Native retained pixels",
        steps: &[
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw-platform-winit",
                    "--lib",
                    "pointer::tests",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw-example-support",
                    "--features",
                    "egui-panel",
                    "--lib",
                    "shared_host_mouse_button_path_emits_double_click",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw-example-support",
                    "--features",
                    "egui-panel",
                    "--lib",
                    "panel::tests",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "p2_animation_geometry_contract",
                    "--test",
                    "p2_animation_sample_reservation_contract",
                    "--test",
                    "m8_viewport_contract",
                ],
            },
            Step {
                program: "cargo",
                args: &["test", "-p", "draw2d-official-examples", "--lib", "tree"],
            },
            Step {
                program: "cargo",
                args: &[
                    "run",
                    "-p",
                    "draw2d-official-examples",
                    "--bin",
                    "tree-animation-proof",
                ],
            },
        ],
    },
    Suite {
        id: "canvas.native",
        description: "Native GPU pixels and two-Canvas Vello/Novadraw integration",
        steps: &[
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "winit-canvas",
                    "--features",
                    "gpu",
                    "--test",
                    "gpu_contract",
                    "--",
                    "--ignored",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw-backend-vello",
                    "--test",
                    "texture_contract",
                    "--",
                    "--ignored",
                ],
            },
            Step {
                program: "cargo",
                args: &["run", "-p", "winit-canvas-demo", "--", "--verify"],
            },
        ],
    },
    Suite {
        id: "core.query-contracts",
        description: "Checked query identity, numerical failure, and recovery contracts",
        steps: &[
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "p2_coordinate_query_failure",
                    "--test",
                    "p2_route_dependency_recovery",
                    "--test",
                    "m4_coordinate_contract",
                    "--test",
                    "m9_connection_contract",
                    "--test",
                    "p2_query_failure_semantics",
                    "--test",
                    "p2_tree_query_identity",
                ],
            },
            Step {
                program: "cargo",
                args: &["test", "-p", "novadraw", "--lib", "s07_"],
            },
        ],
    },
    Suite {
        id: "core.facade",
        description: "Public API, dependency, hot-path, and backend feature boundaries",
        steps: &[
            Step {
                program: "./scripts/check_architecture.sh",
                args: &[],
            },
            Step {
                program: "cargo",
                args: &["test", "-p", "novadraw", "--doc"],
            },
            Step {
                program: "cargo",
                args: &[
                    "check",
                    "-p",
                    "novadraw-backend-vello",
                    "--features",
                    "native",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "check",
                    "-p",
                    "novadraw-backend-vello",
                    "--target",
                    "wasm32-unknown-unknown",
                    "--features",
                    "web",
                ],
            },
        ],
    },
    Suite {
        id: "core.resources",
        description: "Core tokens, Native/Web providers, and Vello image lowering",
        steps: &[
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "tc11_resource_provider_contract",
                    "--test",
                    "image_source_rectangle_contract",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw-platform-winit",
                    "--test",
                    "resource_provider_contract",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "check",
                    "-p",
                    "novadraw-platform-web",
                    "--target",
                    "wasm32-unknown-unknown",
                    "--features",
                    "resource",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw-backend-vello",
                    "--features",
                    "native",
                    "--lib",
                    "image_source_region_maps_to_destination_and_requires_a_destination_clip",
                ],
            },
        ],
    },
    Suite {
        id: "core.render-failure",
        description: "Structured backend failure, recovery, and Native/Web host boundaries",
        steps: &[
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "tc12_render_failure_contract",
                ],
            },
            Step {
                program: "cargo",
                args: &["test", "-p", "novadraw-backend-vello", "--lib"],
            },
            Step {
                program: "cargo",
                args: &["check", "-p", "node-editor-demo"],
            },
            Step {
                program: "cargo",
                args: &[
                    "check",
                    "-p",
                    "web-validation",
                    "--target",
                    "wasm32-unknown-unknown",
                ],
            },
        ],
    },
    Suite {
        id: "core.graphics",
        description: "Core Graphics, text services, publication, and Vello lowering",
        steps: &[
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "tc01_graphics_boundary",
                    "--test",
                    "p2_graphics_contract",
                    "--test",
                    "p2_text_graphics_contract",
                ],
            },
            Step {
                program: "cargo",
                args: &["test", "-p", "novadraw-backend-vello", "--lib"],
            },
        ],
    },
    Suite {
        id: "extensions",
        description: "External Core/Editor consumers and open Editor protocols",
        steps: &[
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--test",
                    "d4_component_update",
                    "--test",
                    "d4_constrained_measurement",
                    "--test",
                    "m10_text_extension_contract",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw-editor",
                    "--test",
                    "ga4_external_visual_component",
                ],
            },
            Step {
                program: "./scripts/check_editor_open_protocol.sh",
                args: &[],
            },
        ],
    },
    Suite {
        id: "runtime.apps",
        description: "Deterministic Update, Event, Viewport, UML, and Animation applications",
        steps: &[
            Step {
                program: "cargo",
                args: &[
                    "run",
                    "-p",
                    "update-app",
                    "--",
                    "--verify",
                    "--report=target/verification/reports/update-app.json",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "run",
                    "-p",
                    "event-app",
                    "--",
                    "--verify",
                    "--report=target/verification/reports/event-app.json",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "run",
                    "-p",
                    "scroll-pane-demo",
                    "--",
                    "--verify",
                    "--report=target/verification/reports/scroll-pane-demo.json",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "run",
                    "-p",
                    "uml-demo",
                    "--",
                    "--verify",
                    "--report=target/verification/reports/uml-demo.json",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "run",
                    "-p",
                    "animation-app",
                    "--",
                    "--verify",
                    "--report=target/verification/reports/animation-app.json",
                ],
            },
        ],
    },
    Suite {
        id: "editor.contracts",
        description: "Complete Editor unit, integration, and documentation contracts",
        steps: &[
            Step {
                program: "cargo",
                args: &["test", "-p", "novadraw-editor"],
            },
            Step {
                program: "cargo",
                args: &["check", "-p", "node-editor-demo"],
            },
        ],
    },
    Suite {
        id: "editor.replay",
        description: "G3, G4, and G5 headless workflows in one process",
        steps: &[Step {
            program: "cargo",
            args: &[
                "run",
                "-p",
                "node-editor-demo",
                "--",
                "--headless-replay=all",
                "--report=target/verification/reports/node-editor-all.json",
            ],
        }],
    },
    Suite {
        id: "platform.text-input",
        description: "Winit/Web text input bridges and wasm host compilation",
        steps: &[
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw-platform-winit",
                    "--features",
                    "editor",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw-platform-web",
                    "--features",
                    "editor",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "check",
                    "-p",
                    "novadraw-platform-web",
                    "--features",
                    "editor",
                    "--target",
                    "wasm32-unknown-unknown",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "check",
                    "-p",
                    "web-validation",
                    "--lib",
                    "--target",
                    "wasm32-unknown-unknown",
                ],
            },
        ],
    },
    Suite {
        id: "graphics.native",
        description: "Native Vello offscreen pixels and retained surface submission",
        steps: &[
            Step {
                program: "cargo",
                args: &["run", "-p", "ndcanvas-app", "--bin", "graphics-proof"],
            },
            Step {
                program: "cargo",
                args: &[
                    "run",
                    "-p",
                    "ndcanvas-app",
                    "--bin",
                    "graphics-proof",
                    "--",
                    "--present",
                ],
            },
        ],
    },
    Suite {
        id: "graphics.host-background",
        description: "Host provider admission, frame recovery and Vello composition pixels",
        steps: &[
            Step {
                program: "cargo",
                args: &["test", "-p", "novadraw", "--test", "p2_host_background"],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw",
                    "--lib",
                    "background_submission::tests",
                ],
            },
            Step {
                program: "cargo",
                args: &[
                    "test",
                    "-p",
                    "novadraw-backend-vello",
                    "--test",
                    "host_background_pixels",
                    "--",
                    "--ignored",
                ],
            },
        ],
    },
    Suite {
        id: "example.inherit-background",
        description: "Official dual Canvas Host gradient, scrolling, movement and repair pixels",
        steps: &[Step {
            program: "cargo",
            args: &[
                "run",
                "-p",
                "draw2d-official-examples",
                "--bin",
                "inherit-background",
                "--",
                "--verify",
            ],
        }],
    },
    Suite {
        id: "web.build",
        description: "Release Wasm and wasm-bindgen browser artifacts",
        steps: &[Step {
            program: "./scripts/build_web_validation.sh",
            args: &[],
        }],
    },
    Suite {
        id: "web.performance",
        description: "Chrome/WebGPU evidence validator and visible benchmark",
        steps: &[
            Step {
                program: "./scripts/test_webgpu_browser_performance.sh",
                args: &[],
            },
            Step {
                program: "./scripts/run_webgpu_browser_performance.sh",
                args: &[],
            },
        ],
    },
];

const PARITY_LEDGERS: &[&str] = &[
    "doc/parity/draw2d/api-coverage.md",
    "doc/parity/gef/api-coverage.md",
];
const MARKDOWN_ROOTS: &[&str] = &["doc", "book"];
const TYPED_MARKDOWN_ROOTS: &[&str] = &["doc"];
const COMMAND_REFERENCE_FILES: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    "verification/README.md",
    "doc/verification/00-index.md",
    "book/contributor/src/01-contribution-workflow.md",
    "book/contributor/src/07-verification-and-review.md",
    "book/contributor/src/appendix-code-and-spec-map.md",
    "book/user/src/09-verification-and-extension.md",
];
const ALLOWED_DOCUMENT_TYPES: &[&str] = &[
    "archive",
    "architecture-decision",
    "architecture-review",
    "documentation-index",
    "implementation-verification",
    "manual-verification",
    "migration-guide",
    "normative-design",
    "parity-contract",
    "product-strategy",
    "proposal",
    "reference-analysis",
    "roadmap",
    "verification",
    "verification-plan",
    "verification-record",
];
const ALLOWED_PARITY_STATUSES: &[&str] =
    &["specified", "partial", "verified", "deferred", "rejected"];

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .context("xtask must live at tools/xtask")
}

fn resolve_suite(selector: &str) -> Result<&'static Suite> {
    if let Some(suite) = SUITES.iter().find(|suite| suite.id == selector) {
        return Ok(suite);
    }
    let matches = SUITES
        .iter()
        .filter(|suite| suite.id.starts_with(&format!("{selector}.")))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [suite] => Ok(*suite),
        [] => bail!("unknown suite `{selector}`; run `cargo xtask list`"),
        _ => bail!(
            "suite selector `{selector}` is ambiguous: {}",
            matches
                .iter()
                .map(|suite| suite.id)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn run_suite(root: &Path, suite: &Suite) -> Result<()> {
    println!("Suite {}: {}", suite.id, suite.description);
    for step in suite.steps {
        let rendered = std::iter::once(step.program)
            .chain(step.args.iter().copied())
            .collect::<Vec<_>>()
            .join(" ");
        println!("\n==> $ {rendered}");
        let status = Command::new(step.program)
            .args(step.args)
            .current_dir(root)
            .status()
            .with_context(|| format!("failed to start `{rendered}`"))?;
        if !status.success() {
            bail!("`{rendered}` failed with {status}");
        }
    }
    Ok(())
}

fn validate_docs(root: &Path) -> Result<usize> {
    let markdown_files = markdown_files_for_roots(root, MARKDOWN_ROOTS)?;
    for path in &markdown_files {
        validate_markdown_links(root, path)?;
    }

    let allowed_types = ALLOWED_DOCUMENT_TYPES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for path in markdown_files_for_roots(root, TYPED_MARKDOWN_ROOTS)? {
        let document_type = read_document_type(&path)?;
        if !allowed_types.contains(document_type.as_str()) {
            bail!(
                "{} uses unsupported document type `{document_type}`",
                path.display()
            );
        }
    }

    for relative in COMMAND_REFERENCE_FILES {
        validate_xtask_references(&root.join(relative))?;
    }

    validate_parity_statuses(root)?;
    validate_index_section(
        root,
        "doc/design/00-index.md",
        "核心设计",
        "doc/design",
        &["normative-design"],
    )?;
    validate_index_section(
        root,
        "doc/design/00-index.md",
        "非规范提案",
        "doc/design",
        &["proposal"],
    )?;
    Ok(markdown_files.len())
}

fn validate_parity_statuses(root: &Path) -> Result<()> {
    let allowed = ALLOWED_PARITY_STATUSES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for relative in PARITY_LEDGERS {
        let path = root.join(relative);
        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let mut checked_rows = 0;
        for (index, line) in source.lines().enumerate() {
            if !line.starts_with("| `") {
                continue;
            }
            let columns = line
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect::<Vec<_>>();
            if columns.len() != 5 {
                continue;
            }
            checked_rows += 1;
            let status = columns[3].trim_matches('`');
            if !allowed.contains(status) {
                bail!(
                    "{}:{} uses unsupported parity status `{status}`",
                    path.display(),
                    index + 1
                );
            }
        }
        if checked_rows == 0 {
            bail!("{} contains no parity status rows", path.display());
        }
    }
    Ok(())
}

fn markdown_files_for_roots(root: &Path, roots: &[&str]) -> Result<BTreeSet<PathBuf>> {
    let mut files = BTreeSet::new();
    for relative in roots {
        let path = root.join(relative);
        if !path.exists() {
            bail!("documentation root `{relative}` does not exist");
        }
        collect_markdown_files(&path, &mut files)?;
    }
    Ok(files)
}

fn collect_markdown_files(path: &Path, files: &mut BTreeSet<PathBuf>) -> Result<()> {
    if path.is_file() {
        if path.extension().is_some_and(|extension| extension == "md") {
            files.insert(path.to_path_buf());
        }
        return Ok(());
    }
    for entry in fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))? {
        let entry = entry.with_context(|| format!("failed to read {}", path.display()))?;
        collect_markdown_files(&entry.path(), files)?;
    }
    Ok(())
}

fn read_document_type(path: &Path) -> Result<String> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    source
        .lines()
        .find_map(|line| {
            line.strip_prefix("类型：`")
                .and_then(|value| value.strip_suffix('`'))
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .with_context(|| format!("{} is missing a `类型：`...`` marker", path.display()))
}

fn validate_markdown_links(root: &Path, path: &Path) -> Result<()> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    for (line_index, line) in source.lines().enumerate() {
        for raw_target in markdown_links(line) {
            let Some(target) = local_markdown_target(raw_target) else {
                continue;
            };
            let resolved = path
                .parent()
                .expect("markdown file has a parent")
                .join(target);
            if !resolved.exists() {
                let relative = path.strip_prefix(root).unwrap_or(path);
                bail!(
                    "{}:{} references missing local path `{target}`",
                    relative.display(),
                    line_index + 1
                );
            }
        }
    }
    Ok(())
}

fn markdown_links(line: &str) -> Vec<&str> {
    let mut links = Vec::new();
    let mut remaining = line;
    while let Some(start) = remaining.find("](") {
        let target = &remaining[start + 2..];
        let Some(end) = target.find(')') else {
            break;
        };
        links.push(&target[..end]);
        remaining = &target[end + 1..];
    }
    links
}

fn local_markdown_target(raw: &str) -> Option<&str> {
    let target = raw
        .split_whitespace()
        .next()?
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>');
    if target.is_empty()
        || target.starts_with('#')
        || target.starts_with("mailto:")
        || target.contains("://")
    {
        return None;
    }
    target
        .split(['#', '?'])
        .next()
        .filter(|target| !target.is_empty())
}

fn validate_xtask_references(path: &Path) -> Result<()> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    for (line_index, line) in source.lines().enumerate() {
        let mut remaining = line;
        while let Some(start) = remaining.find("cargo xtask ") {
            let reference = &remaining[start + "cargo xtask ".len()..];
            validate_xtask_reference(reference)
                .with_context(|| format!("{}:{}", path.display(), line_index + 1))?;
            remaining = reference.get(1..).unwrap_or_default();
        }
    }
    Ok(())
}

fn validate_xtask_reference(reference: &str) -> Result<()> {
    let tokens = reference
        .split_whitespace()
        .map(clean_command_token)
        .filter(|token| !token.is_empty())
        .take(2)
        .collect::<Vec<_>>();
    let Some(command) = tokens.first().copied() else {
        bail!("empty `cargo xtask` reference");
    };
    match command {
        "list" | "docs" => Ok(()),
        "verify" => {
            let selector = tokens
                .get(1)
                .copied()
                .context("`cargo xtask verify` is missing a suite ID")?;
            if selector == "<suite-id>" {
                Ok(())
            } else {
                resolve_suite(selector).map(|_| ())
            }
        }
        _ => bail!("removed xtask subcommand `{command}`"),
    }
}

fn clean_command_token(token: &str) -> &str {
    let token = token.trim_start_matches(['`', '"', '\'']);
    let end = token
        .find([
            '`', '"', '\'', ',', ';', ':', ')', ']', '}', '|', '，', '。', '；', '：', '、', '）',
        ])
        .unwrap_or(token.len());
    &token[..end]
}

fn validate_index_section(
    root: &Path,
    index: &str,
    heading: &str,
    target_root: &str,
    allowed_types: &[&str],
) -> Result<()> {
    let index_path = root.join(index);
    let source = fs::read_to_string(&index_path)
        .with_context(|| format!("failed to read {}", index_path.display()))?;
    let marker = format!("## {heading}");
    let mut in_section = false;
    let mut section_source = String::new();
    for line in source.lines() {
        if line.starts_with("## ") {
            if in_section {
                break;
            }
            in_section = line.trim() == marker;
            continue;
        }
        if in_section {
            section_source.push_str(line);
            section_source.push('\n');
        }
    }
    if !in_section {
        bail!("{index} is missing section `{marker}`");
    }

    let target_root = fs::canonicalize(root.join(target_root))
        .with_context(|| format!("failed to resolve {target_root}"))?;
    let allowed_types = allowed_types.iter().copied().collect::<BTreeSet<_>>();
    let mut checked = 0;
    for raw_target in markdown_links(&section_source) {
        let Some(target) = local_markdown_target(raw_target) else {
            continue;
        };
        let target_path = index_path
            .parent()
            .expect("documentation index has a parent")
            .join(target);
        if target_path
            .extension()
            .is_none_or(|extension| extension != "md")
            || !target_path.exists()
        {
            continue;
        }
        let canonical = fs::canonicalize(&target_path)
            .with_context(|| format!("failed to resolve {}", target_path.display()))?;
        if !canonical.starts_with(&target_root) {
            continue;
        }
        checked += 1;
        let document_type = read_document_type(&target_path)?;
        if !allowed_types.contains(document_type.as_str()) {
            bail!("{index} section `{heading}` classifies {target} as `{document_type}`");
        }
    }
    if checked == 0 {
        bail!("{index} section `{heading}` contains no matching documents");
    }
    Ok(())
}

fn print_help() {
    println!(
        "Novadraw verification\n\n\
         Usage:\n\
           cargo xtask list\n\
           cargo xtask docs\n\
           cargo xtask verify <suite-id>"
    );
}

fn run() -> Result<()> {
    let root = workspace_root()?;
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [command] if command == "list" => {
            for suite in SUITES {
                println!("{:<24} {}", suite.id, suite.description);
            }
        }
        [command] if command == "docs" => {
            let count = validate_docs(&root)?;
            println!("PASS {count} Markdown files and current xtask references");
        }
        [command, selector] if command == "verify" => {
            run_suite(&root, resolve_suite(selector)?)?;
        }
        [] | [_]
            if args
                .first()
                .is_none_or(|arg| arg == "help" || arg == "--help") =>
        {
            print_help();
        }
        _ => {
            print_help();
            bail!("invalid arguments");
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suite_ids_are_unique() {
        let ids = SUITES.iter().map(|suite| suite.id).collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), SUITES.len());
    }

    #[test]
    fn suite_prefix_must_be_unique() {
        assert!(resolve_suite("core").is_err());
        assert_eq!(resolve_suite("editor.replay").unwrap().id, "editor.replay");
    }

    #[test]
    fn removed_commands_are_rejected_in_current_docs() {
        assert!(validate_xtask_reference("verify core.facade").is_ok());
        assert!(validate_xtask_reference("verify <suite-id>").is_ok());
        assert!(validate_xtask_reference("check --full").is_err());
        assert!(validate_xtask_reference("run workspace.test").is_err());
        assert!(validate_xtask_reference("manual g5.4").is_err());
    }

    #[test]
    fn repository_documentation_is_valid() {
        let root = workspace_root().unwrap();
        validate_docs(&root).unwrap();
    }

    #[test]
    fn markdown_link_parser_keeps_local_targets_and_ignores_web_links() {
        let links = markdown_links("[local](guide.md#section) [web](https://example.com)");
        assert_eq!(local_markdown_target(links[0]), Some("guide.md"));
        assert_eq!(local_markdown_target(links[1]), None);
    }
}
