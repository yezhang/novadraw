"""Compare same-sequence uncached/cached retained GPU images, excluding readback from costs."""
import argparse
import json
from pathlib import Path
from PIL import Image, ImageChops


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--cached", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    reference = sorted(args.reference.glob("frame-*.png"))
    cached = sorted(args.cached.glob("frame-*.png"))
    if not reference or [p.name for p in reference] != [p.name for p in cached]:
        raise ValueError("Missing frames or unequal sequences")
    frames = []
    for left, right in zip(reference, cached):
        a = Image.open(left).convert("RGBA")
        b = Image.open(right).convert("RGBA")
        if a.size != b.size:
            raise ValueError(f"Size differs: {left.name}")
        channels = ImageChops.difference(a, b).split()
        peak = channels[0]
        for channel in channels[1:]:
            peak = ImageChops.lighter(peak, channel)
        histogram = peak.histogram()
        changed = sum(histogram[1:])
        over_one = sum(histogram[2:])
        maximum = max(i for i, count in enumerate(histogram) if count)
        total = sum(i*count for channel in channels
                    for i, count in enumerate(channel.histogram()))
        frames.append({"name": left.name, "size": a.size, "changed_pixels": changed,
                       "pixels_over_one": over_one, "max_channel_delta": maximum,
                       "mean_channel_delta": total / (a.width*a.height*4)})
    result = {"frames": frames, "count": len(frames),
              "exact_frames": sum(r["changed_pixels"] == 0 for r in frames),
              "max_channel_delta": max(r["max_channel_delta"] for r in frames),
              "pixels_over_one": sum(r["pixels_over_one"] for r in frames)}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2))
    print(json.dumps({k: v for k, v in result.items() if k != "frames"}))


if __name__ == "__main__":
    main()
