# Text App

M10.2 LabelFigure visual verification.

```bash
cargo run -p text-app
cargo run -p text-app -- --screenshot-all
cargo run -p text-app -- --screenshot=Ellipsis
cargo run -p text-app -- --screenshot=Image_Resources
cargo run -p text-app -- --screenshot=Emoji_Color
```

The eight scenarios cover explicit font registration, CJK glyph shaping, measured ellipsis,
image placement, inherited Figure style, TitleBarBorder, PNG/SVG decoding, and
Pending/Ready/Failed image resource states. Screenshots are written to
`target/visual-verification/screenshots/`.

`Emoji_Color` explicitly registers the bundled Noto Color Emoji font and uses the existing
Parley / Vello glyph path. It shows faces, ZWJ sequences, skin tones, flags, VS16
presentation, and a wrapping Chinese / English paragraph with emoji fallback.
Select the eighth scene with `End`, or use the screenshot command above.
Font provenance and license are in the [shared scene assets](../../scenes/assets/README.md).
This example exercises COLRv1; it does not validate every emoji font format or the
single-color outline-provider path.
