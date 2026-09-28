# Novadraw

`novadraw` is the standard public entry point for the backend-neutral 2D Figure runtime and
model-driven editor framework.

## Start With The Prelude

```rust
use novadraw::prelude::*;

let mut tree = FigureTree::new();
let root = tree
    .builder()
    .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 640.0, 480.0)));
let runtime = Runtime::new(tree);

assert_eq!(
    runtime.tree().figure_bounds(root),
    Some(Rectangle::new(0.0, 0.0, 640.0, 480.0)),
);
```

The crate root and `prelude` contain only common, stable entry points. Specialized APIs live in
named modules:

- `geometry`: geometry values, precision, and transforms;
- `graphics`: `NdCanvas`, path construction, and stroke styles;
- `figure`: Figure extension traits, built-in figures, borders, and styles;
- `layout`: layout extension contracts and built-in layouts;
- `container`: viewport, scrolling, zoom, layering, and freeform containers;
- `connection`: anchors, routers, locators, and connection runtime contracts;
- `event`: input, listener, focus, tooltip, and accessibility contracts;
- `runtime`: scoped mutation, resources, and frame preparation;
- `render`: backend-neutral commands, submissions, resources, and text;
- `editor`: the model-driven GEF-style editor framework;
- `advanced`: low-level state intended for diagnostics and specialized integrations.

Low-level protocols are intentionally not available at the crate root:

```compile_fail
use novadraw::{FigureNode, RenderCommand, UpdateManager};
```

Use their explicit modules when required:

```rust
use novadraw::advanced::{FigureNode, UpdateManager};
use novadraw::render::command::RenderCommand;

fn accepts_low_level_types(
    _node: Option<FigureNode>,
    _updates: Option<UpdateManager>,
    _command: Option<RenderCommand>,
) {
}
```

## Backend Features

The default build is platform-independent and does not select a renderer.

| Feature | Enables |
|---|---|
| `native-vello` | Vello renderer with native surface support |
| `web-vello` | Vello renderer with Web canvas support |

With either feature enabled, the renderer is available as
`novadraw::backend::vello::VelloRenderer`. Platform adapters such as Winit and DOM event bridges
remain outside the core runtime.

Applications that need deeper implementation access may depend directly on a specialized
`novadraw-*` crate. Such dependencies are explicit extension choices, not the default application
path.
