# Text App

M10.2 LabelFigure visual verification.

```bash
cargo run -p text-app
cargo run -p text-app -- --screenshot-all
cargo run -p text-app -- --screenshot=Ellipsis
cargo run -p text-app -- --screenshot=Image_Resources
```

The four scenarios cover explicit font registration, CJK glyph shaping, measured ellipsis,
image placement, inherited Figure style, TitleBarBorder, PNG/SVG decoding, and
Pending/Ready/Failed image resource states. Screenshots are written to
`target/visual-verification/screenshots/`.
