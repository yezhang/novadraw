# Animation Example

`animation-app` contains two shared Native/Web scenes.

`Orthogonal_Model` demonstrates the P2-M01 orthogonal animation model:

- one Tween reused across rectangle, ellipse, and rounded-rectangle targets;
- Tween, Keyframes, and Spring motions targeting the same presentation channel type;
- Parallel, Stagger, Repeat, and Reverse composition without source-state mutation;
- a captured bounds transition whose committed Figure bounds are final before animation starts;
- a Runtime-owned temporary visual with independent transform and opacity tracks.

`Runtime_Mechanisms` exercises the complete mechanism stack:

- committed-fact Behavior/Trigger;
- atomic layout and Viewport transactions;
- compatible route interpolation and incompatible route crossfade;
- continuous procedural dash flow;
- arc-length pulse with endpoint-decoration handoff;
- deterministic completion, cancellation, temporary cleanup, and reduced-motion cleanup.

Run the native example:

```sh
cargo run -p animation-app
```

Run the deterministic headless verification:

```sh
cargo run -p animation-app -- --verify
```

Run one verification case:

```sh
cargo run -p animation-app -- --verify --scenario=source-presentation-separation
cargo run -p animation-app -- --verify --scenario=viewport-and-route-transition
```
