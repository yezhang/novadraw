use std::path::Path;

use image::RgbaImage;

const CHANNEL_TOLERANCE: u8 = 5;

fn near(image: &RgbaImage, dpi: u32, x: u32, y: u32, expected: [u8; 3]) {
    let actual = image.get_pixel(x * dpi, y * dpi).0;
    assert!(
        actual[..3]
            .iter()
            .zip(expected)
            .all(|(&a, e)| a.abs_diff(e) <= CHANNEL_TOLERANCE),
        "pixel ({x},{y}) at DPI {dpi}: {actual:?}, expected {expected:?}"
    );
}

pub fn verify(path: &Path, dpi: u32) {
    let image = image::open(path).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (800 * dpi, 600 * dpi));
    near(&image, dpi, 140, 50, [127, 0, 128]);
    near(&image, dpi, 380, 50, [255, 191, 191]);
    near(&image, dpi, 610, 50, [255, 0, 0]);
    near(&image, dpi, 630, 50, [0, 0, 255]);
    near(&image, dpi, 430, 180, [255, 255, 255]);
    near(&image, dpi, 345, 140, [255, 255, 255]);
    near(&image, dpi, 360, 120, [0, 255, 0]);
    near(&image, dpi, 380, 230, [0, 255, 0]);
    near(&image, dpi, 380, 260, [255, 255, 255]);
    near(&image, dpi, 350, 110, [255, 255, 255]);
    near(&image, dpi, 710, 260, [255, 0, 0]);
    near(&image, dpi, 740, 300, [0, 255, 0]);
    near(&image, dpi, 620, 160, [198, 0, 57]);
    near(&image, dpi, 43, 300, [0, 0, 0]);
    near(&image, dpi, 52, 300, [255, 255, 255]);
    near(&image, dpi, 63, 300, [0, 0, 0]);
    near(&image, dpi, 345, 300, [0, 0, 0]);
    near(&image, dpi, 358, 300, [255, 255, 255]);

    // Opaque glyph interiors must sample the Canvas brush, without run-origin drift.
    for (top, bottom) in [(125, 185), (195, 255)] {
        let mut checked = 0;
        for y in top * dpi..bottom * dpi {
            for x in 40 * dpi..280 * dpi {
                let [r, g, b, _] = image.get_pixel(x, y).0;
                if g <= 2 && (i32::from(r) + i32::from(b) - 255).abs() <= 3 {
                    let t = (f64::from(x) / f64::from(dpi) - 40.0) / 240.0;
                    let expected_blue = (255.0 * t).round() as u8;
                    assert!(
                        b.abs_diff(expected_blue) <= CHANNEL_TOLERANCE,
                        "glyph brush drift at ({x},{y}), DPI {dpi}: blue={b}, expected={expected_blue}"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 100 * dpi, "missing glyph pixels in row {top}");
    }
}

pub fn verify_sequence(directory: &Path, dpi: u32) {
    let load = |phase| {
        image::open(directory.join(format!("native-{dpi}-{phase}.png")))
            .unwrap()
            .to_rgba8()
    };
    let baseline = load("baseline");
    for phase in ["rejected", "cancelled", "miter-restored", "full"] {
        assert_eq!(
            baseline,
            load(phase),
            "retained pixels differ after {phase}, DPI {dpi}"
        );
    }
    let shown = load("shown");
    let moved = load("moved");
    assert_ne!(baseline, shown, "feedback missing");
    assert_ne!(shown, moved, "feedback did not move");
    near(&shown, dpi, 120, 440, [255, 153, 0]);
    near(&moved, dpi, 120, 440, [0, 0, 0]);
    near(&moved, dpi, 180, 440, [255, 153, 0]);
    near(&baseline, dpi, 620, 415, [255, 255, 255]);
    near(&load("miter-large"), dpi, 620, 415, [0, 0, 0]);
}
