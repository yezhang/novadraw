# Draw2D Example Assets

`class_obj.png` is a lossless PNG conversion of
`org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples/uml/class_obj.gif` from
`eclipse-gef/gef-classic@4463d9d0ce13c19d10fbe769d29f28b7345a8cba`.

The source image is licensed under EPL-2.0. Its pixel content is unchanged; conversion is required
because the Novadraw image resource decoder does not accept GIF input.

`zoom/class_obj.png`, `zoom/field_private_obj.png`, and `zoom/methpub_obj.png`
are unmodified PNG alternatives shipped beside the SVG icons in the upstream
`org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples/zoom/images/`
directory at the same revision. They retain the upstream EPL-2.0 license.
Using these PNG alternatives does not establish SVG scaling parity.

`brazil.ico` is copied unchanged from
`org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples/printing/brazil.ico`
at the same upstream revision. It retains the upstream EPL-2.0 license and is
decoded directly through the Novadraw image resource API.

`zoom/class_obj.svg`, `zoom/field_private_obj.svg`, and `zoom/methpub_obj.svg`
are unmodified original SVG resources from the upstream zoom/images directory
at the same revision, under EPL-2.0. Zoom now decodes these sources using the
existing Core SVG rasterizer. Rasterizing at intrinsic size does not establish
continuous vector rendering or rerasterization at every zoom/DPI change.
