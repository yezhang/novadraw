# Shared scene assets

These assets belong to Core examples, not the engine's built-in font set.

## Noto Color Emoji

`NotoColorEmoji.ttf` is an unmodified Google Fonts download, retrieved on 2026-11-02:

- Font: <https://fonts.gstatic.com/s/notocoloremoji/v50/Yq6P-KqIXTD0t4D9z1ESnKM3-HpFab5s79iz64w.ttf>
- Download manifest and original license: <https://fonts.google.com/download/list?family=Noto%20Color%20Emoji>
- License: SIL Open Font License 1.1; the original notice and text are preserved in
  [NotoColorEmoji-OFL.txt](NotoColorEmoji-OFL.txt).
- SHA-256: `d8c8a5e2eaecf93bf46be895640139d344ace174e2cdb6b0d988f7e83fd97844`.
- The file contains COLR version 1, CPAL, GSUB, and outline tables. It also contains
  an SVG table; the example uses Vello's COLR path and does not claim SVG-font support.

The `text-image/emoji-color` scene registers the font through the ordinary Runtime
resource service. The same scene and font bytes are available to Native and Web hosts.
The font binary is tracked with Git LFS. No system font lookup or startup download is needed.
