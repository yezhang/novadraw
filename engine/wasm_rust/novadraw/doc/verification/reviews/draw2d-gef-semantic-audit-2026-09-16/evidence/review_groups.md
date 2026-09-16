## Group 1: Figure topology, identity, lifecycle, search, events, focus, notifications

按语义与调用链分组；共享 Runtime/Graph/接口跨组。

- novadraw/src/lib.rs
- novadraw-apps/src/app.rs
- novadraw-apps/src/input.rs
- novadraw-apps/src/lib.rs
- novadraw-apps/src/platform.rs
- novadraw-apps/src/prelude.rs
- novadraw-apps/src/verification.rs
- novadraw-demo-scenes/src/border.rs
- novadraw-demo-scenes/src/clip.rs
- novadraw-demo-scenes/src/connection.rs
- novadraw-demo-scenes/src/event.rs
- novadraw-demo-scenes/src/focus.rs
- novadraw-demo-scenes/src/freeform.rs
- novadraw-demo-scenes/src/layout.rs
- novadraw-demo-scenes/src/lib.rs
- novadraw-demo-scenes/src/ndcanvas.rs
- novadraw-demo-scenes/src/scroll_pane.rs
- novadraw-demo-scenes/src/shape.rs
- novadraw-demo-scenes/src/style.rs
- novadraw-demo-scenes/src/text.rs
- novadraw-demo-scenes/src/transform.rs
- novadraw-demo-scenes/src/update.rs
- novadraw-demo-scenes/src/viewport.rs
- novadraw-demo-scenes/src/widget.rs
- novadraw-scene/src/graph/bounds_test.rs
- novadraw-scene/src/graph/mod.rs
- novadraw-scene/src/graph/search.rs
- novadraw-scene/src/graph/update_integration_test.rs
- novadraw-scene/src/host/mod.rs
- novadraw-scene/src/host/scene_host.rs
- novadraw-scene/src/identity.rs
- novadraw-scene/src/lib.rs
- novadraw-scene/src/log.rs
- novadraw-scene/src/runtime/accessibility.rs
- novadraw-scene/src/runtime/context.rs
- novadraw-scene/src/runtime/event/mod.rs
- novadraw-scene/src/runtime/focus.rs
- novadraw-scene/src/runtime/interaction.rs
- novadraw-scene/src/runtime/mod.rs
- novadraw-scene/src/runtime/mutation/mod.rs
- novadraw-scene/src/runtime/runtime.rs
- novadraw-scene/src/runtime/tooltip.rs
- novadraw-scene/src/style.rs
- novadraw-scene/tests/d1_focus_contract.rs
- novadraw-scene/tests/d1_tree_search_contract.rs
- novadraw-scene/tests/d3_runtime_listener.rs
- novadraw-scene/tests/d3_runtime_mutation.rs
- novadraw-scene/tests/d4_component_update.rs
- novadraw-scene/tests/d4_constrained_measurement.rs
- novadraw-scene/tests/d4_notification_epoch.rs
- novadraw-scene/tests/m2_product_existence.rs
- novadraw-scene/tests/m4_coordinate_contract.rs
- novadraw-scene/tests/m6_event_contract.rs
- novadraw-scene/tests/p2_dispatch_outcome_contract.rs
- novadraw-scene/tests/runtime_resize_contract.rs

## Group 2: Layout, validation, UpdateManager, damage, viewport, freeform, zoom

按语义与调用链分组；共享 Runtime/Graph/接口跨组。

- novadraw-scene/src/container/layer.rs
- novadraw-scene/src/container/mod.rs
- novadraw-scene/src/container/range_model.rs
- novadraw-scene/src/container/scalable.rs
- novadraw-scene/src/container/scroll_pane.rs
- novadraw-scene/src/container/viewport.rs
- novadraw-scene/src/container/zoom.rs
- novadraw-scene/src/graph/mod.rs
- novadraw-scene/src/layout/border_layout.rs
- novadraw-scene/src/layout/fill_layout.rs
- novadraw-scene/src/layout/flow_layout.rs
- novadraw-scene/src/layout/freeform_layout.rs
- novadraw-scene/src/layout/grid_layout.rs
- novadraw-scene/src/layout/mod.rs
- novadraw-scene/src/layout/stack_layout.rs
- novadraw-scene/src/layout/toolbar_layout.rs
- novadraw-scene/src/layout/xy_layout.rs
- novadraw-scene/src/lib.rs
- novadraw-scene/src/runtime/runtime.rs
- novadraw-scene/src/runtime/update/deferred.rs
- novadraw-scene/src/runtime/update/listener.rs
- novadraw-scene/src/runtime/update/mod.rs
- novadraw-scene/src/runtime/update/repair.rs
- novadraw-scene/tests/d2_freeform_contract.rs
- novadraw-scene/tests/d2_layer_contract.rs
- novadraw-scene/tests/m5_layout_contract.rs
- novadraw-scene/tests/m8_viewport_contract.rs

## Group 3: Geometry, Graphics, recursive paint, backend, resources, text, widgets

按语义与调用链分组；共享 Runtime/Graph/接口跨组。

- novadraw-core/src/color.rs
- novadraw-core/src/lib.rs
- novadraw-geometry/src/lib.rs
- novadraw-geometry/src/point_list.rs
- novadraw-geometry/src/precision.rs
- novadraw-geometry/src/rect.rs
- novadraw-geometry/src/transform.rs
- novadraw-geometry/src/translatable.rs
- novadraw-geometry/src/vec2.rs
- novadraw-geometry/tests/m1_product_existence.rs
- novadraw-math/src/lib.rs
- novadraw-math/src/mat3.rs
- novadraw-math/src/vec3.rs
- novadraw-render/src/backend/mod.rs
- novadraw-render/src/backend/vello/mod.rs
- novadraw-render/src/command.rs
- novadraw-render/src/context.rs
- novadraw-render/src/lib.rs
- novadraw-render/src/submission.rs
- novadraw-render/src/text.rs
- novadraw-render/src/traits.rs
- novadraw-render/tests/m10_text_extension_contract.rs
- novadraw-render/tests/m1_product_existence.rs
- novadraw-render/tests/r8_extension_boundaries.rs
- novadraw-scene/src/figure/border/bevel_border.rs
- novadraw-scene/src/figure/border/compound_border.rs
- novadraw-scene/src/figure/border/etched_border.rs
- novadraw-scene/src/figure/border/line_border.rs
- novadraw-scene/src/figure/border/margin_border.rs
- novadraw-scene/src/figure/border/mod.rs
- novadraw-scene/src/figure/border/rectangle_border.rs
- novadraw-scene/src/figure/border/title_bar_border.rs
- novadraw-scene/src/figure/ellipse.rs
- novadraw-scene/src/figure/image.rs
- novadraw-scene/src/figure/label.rs
- novadraw-scene/src/figure/mod.rs
- novadraw-scene/src/figure/polygon.rs
- novadraw-scene/src/figure/polyline.rs
- novadraw-scene/src/figure/rectangle.rs
- novadraw-scene/src/figure/root.rs
- novadraw-scene/src/figure/rounded_rectangle.rs
- novadraw-scene/src/figure/triangle.rs
- novadraw-scene/src/figure/widget.rs
- novadraw-scene/src/graph/mod.rs
- novadraw-scene/src/graph/render_recursive.rs
- novadraw-scene/src/lib.rs
- novadraw-scene/src/runtime/resource.rs
- novadraw-scene/src/runtime/runtime.rs
- novadraw-scene/tests/m10_accessibility_contract.rs
- novadraw-scene/tests/m10_label_contract.rs
- novadraw-scene/tests/m10_reusable_shape_border_contract.rs
- novadraw-scene/tests/m10_tooltip_contract.rs
- novadraw-scene/tests/m10_widget_contract.rs

## Group 4: Connection Figure, Anchor, Router, Locator and runtime atomicity

按语义与调用链分组；共享 Runtime/Graph/接口跨组。

- novadraw-scene/src/connection/anchor.rs
- novadraw-scene/src/connection/figure.rs
- novadraw-scene/src/connection/locator.rs
- novadraw-scene/src/connection/mod.rs
- novadraw-scene/src/connection/query.rs
- novadraw-scene/src/connection/router.rs
- novadraw-scene/src/connection/runtime.rs
- novadraw-scene/src/graph/mod.rs
- novadraw-scene/src/lib.rs
- novadraw-scene/src/runtime/runtime.rs
- novadraw-scene/tests/m9_connection_contract.rs
- novadraw-scene/tests/m9_connection_runtime.rs

## Group 5: Model, CommandStack, EditPart, registries and projection

按语义与调用链分组；共享 Runtime/Graph/接口跨组。

- novadraw-editor/src/command/mod.rs
- novadraw-editor/src/model/mod.rs
- novadraw-editor/src/part/mod.rs
- novadraw-editor/src/viewer/mod.rs
- novadraw-editor/tests/g1_command_stack_contract.rs
- novadraw-editor/tests/g1_model_contract.rs
- novadraw-editor/tests/g2_viewer_projection_contract.rs
- novadraw-editor/tests/g5_connection_projection_contract.rs
- novadraw-scene/src/graph/mod.rs
- novadraw-scene/src/lib.rs
- novadraw-scene/src/runtime/runtime.rs

## Group 6: Domain, Tool, Request, EditPolicy, targeting, feedback and auto-expose

按语义与调用链分组；共享 Runtime/Graph/接口跨组。

- novadraw-editor/src/autoexpose.rs
- novadraw-editor/src/domain.rs
- novadraw-editor/src/feedback/mod.rs
- novadraw-editor/src/lib.rs
- novadraw-editor/src/part/mod.rs
- novadraw-editor/src/policy/mod.rs
- novadraw-editor/src/request/mod.rs
- novadraw-editor/src/selection/mod.rs
- novadraw-editor/src/tool/mod.rs
- novadraw-editor/src/viewer/mod.rs
- novadraw-editor/tests/g3_selection_contract.rs
- novadraw-editor/tests/g3_viewer_interaction_contract.rs
- novadraw-editor/tests/g4_editing_loop_contract.rs
- novadraw-editor/tests/g5_connection_creation_contract.rs
- novadraw-scene/src/graph/mod.rs
- novadraw-scene/src/lib.rs
- novadraw-scene/src/runtime/runtime.rs
