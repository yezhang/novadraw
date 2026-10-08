# Novadraw

`novadraw` is the platform-independent Core crate for the 2D Figure runtime.

## Start With The Prelude

```rust
use novadraw::prelude::*;

let mut tree = FigureTree::new();
let root = tree
    .builder()
    .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 640.0, 480.0)))
    .expect("valid FigureTree construction");
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
- `advanced`: low-level state intended for diagnostics and specialized integrations.

Low-level protocols are intentionally not available at the crate root:

```compile_fail
use novadraw::FigureNode;
```

```compile_fail
use novadraw::NodeState;
```

```compile_fail
use novadraw::LayoutState;
```

```compile_fail
use novadraw::EventDispatcher;
```

```compile_fail
use novadraw::InteractionState;
```

```compile_fail
use novadraw::PendingMutations;
```

```compile_fail
use novadraw::NotificationQueue;
```

```compile_fail
use novadraw::UpdateManager;
```

```compile_fail
use novadraw::RuntimeNamespace;
```

```compile_fail
use novadraw::RenderCommand;
```

```compile_fail
use novadraw::RouteOutput;
```

```compile_fail
use novadraw::SceneQuery;
```

```compile_fail
use novadraw::TrackedSceneQuery;
```

```compile_fail
use novadraw::FigurePreparation;
```

```compile_fail
use novadraw::PreparedFigureUpdate;
```

```compile_fail
use novadraw::ResourceRegistry;
```

```compile_fail
use novadraw::RenderSubmission;
```

Specialist protocols are also excluded from the root and prelude:

```compile_fail
use novadraw::NdCanvas;
```

```compile_fail
use novadraw::RenderBackend;
```

```compile_fail
use novadraw::ConnectionRouter;
```

```compile_fail
use novadraw::RangeModel;
```

Property keys type-check their old and new values:

```compile_fail
use novadraw::FigureId;
use novadraw::event::{PropertyKey, TypedPropertyChange};

const ENABLED: PropertyKey<bool> = PropertyKey::new("example.figure", "enabled");

fn invalid_change(figure: FigureId) {
    let _ = TypedPropertyChange::new(figure, ENABLED, false, 1.0_f64);
}
```

Use their explicit modules when required:

```rust
use novadraw::advanced::{FigureNode, UpdateManager};
use novadraw::connection::{ConnectionRouter, RouteOutput};
use novadraw::container::RangeModel;
use novadraw::graphics::NdCanvas;
use novadraw::render::command::RenderCommand;
use novadraw::render::{RenderBackend, RenderSubmission};
use novadraw::runtime::ResourceRegistry;

fn accepts_low_level_types(
    _node: Option<FigureNode>,
    _updates: Option<UpdateManager>,
    _command: Option<RenderCommand>,
    _route: Option<RouteOutput>,
    _submission: Option<RenderSubmission>,
    _resources: Option<ResourceRegistry>,
    _canvas: Option<NdCanvas>,
    _backend: Option<&dyn RenderBackend>,
    _router: Option<&dyn ConnectionRouter>,
    _range: Option<&dyn RangeModel>,
) {
}
```

## Optional Packages

The Core crate does not select a renderer, window system, or editor framework. Applications compose
the capabilities they need:

- `novadraw-editor`: model-driven graphical editor framework;
- `novadraw-inspector`: read-only diagnostics;
- `novadraw-backend-vello`: native and Web Vello renderer;
- `novadraw-platform-winit`: desktop input and host adapter;
- `novadraw-platform-web`: browser input and host adapter.
