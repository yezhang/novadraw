# Animation Example

`animation-app` contains four shared Native/Web scenes.

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
- continuous procedural dash flow with a route-oriented target arrow;
- arc-length pulse with endpoint-decoration handoff;
- deterministic completion, cancellation, temporary cleanup, and reduced-motion cleanup.

`Choreography_Lab` presents the animation API as a composable vocabulary:

- Tween, Keyframes, Spring, Decay, and finite Procedural motions on the same transform target;
- Sequence, Stagger, and Delay composed into one repeated plan;
- independent transform and opacity tracks on one Figure;
- Restart and Reverse finite loops beside a continuous Procedural loop;
- deterministic sampling that keeps committed Figure geometry unchanged.

`Runtime_Control` exposes policies that matter after a plan is running:

- stable `selected` facts start Figure-scoped behaviors;
- Replace continuously takes over from the current presentation sample while Ignore retains the
  current owner;
- a moving presentation remains independently clickable through its fixed committed hit target;
- Pause excludes hidden time from a local timeline while Finish settles immediately.

Run the native example:

```sh
cargo run -p animation-app
```

Press `R` to rebuild the current scene and replay every animation from its deterministic initial
state. Use the left/right arrow keys to switch among `Orthogonal_Model`,
`Runtime_Mechanisms`, `Choreography_Lab`, and `Runtime_Control`.

Run the deterministic headless verification:

```sh
cargo run -p animation-app -- --verify
```

Run one verification case:

```sh
cargo run -p animation-app -- --verify --scenario=source-presentation-separation
cargo run -p animation-app -- --verify --scenario=viewport-and-route-transition
cargo run -p animation-app -- --verify --scenario=dashed-arrow-flow
cargo run -p animation-app -- --verify --scenario=complete-motion-family
cargo run -p animation-app -- --verify --scenario=composition-grammar
cargo run -p animation-app -- --verify --scenario=behavior-interruption-policies
cargo run -p animation-app -- --verify --scenario=interaction-geometry-policy
cargo run -p animation-app -- --verify --scenario=suspension-policies
```
