# UML Demo

`uml-demo` is a compound class-diagram example inspired by Draw2D's
`org.eclipse.draw2d.examples.uml.UMLClassDiagram`.

It deliberately lives outside `novadraw` and builds the diagram through public APIs:

- external `Figure` and `FigureContainer` implementations;
- nested `ToolbarLayout` compartments;
- `ConnectionLayerFigure`, anchors, routers, locators, and endpoint decorations;
- solid and dashed UML relations;
- pointer-captured class dragging with automatic connection rerouting;
- reusable labels and inherited styles.

Run the native example:

```sh
cargo run -p uml-demo
```

Drag any class with the primary pointer button. Native and Web use the same
`FigureEventHandler` and deferred Runtime bounds mutation path.

Capture its only scene:

```sh
cargo run -p uml-demo -- --screenshot=order-domain
```

Run the deterministic headless probe:

```sh
cargo run -p uml-demo -- --verify
```
