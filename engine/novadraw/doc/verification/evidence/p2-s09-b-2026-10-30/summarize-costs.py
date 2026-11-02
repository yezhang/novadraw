"""Derive session medians from the raw Inspector release probe log."""
import json
import pathlib
import statistics

path = pathlib.Path(__file__).resolve().with_name("inspector-cost.log")
groups = {}
for line in path.read_text().splitlines():
    if not line.startswith("COST "):
        continue
    row = dict(part.split("=", 1) for part in line.split()[1:])
    groups.setdefault(row["name"], []).append(row)
summary = {}
for name, rows in groups.items():
    assert len(rows) == 5
    summary[name] = {
        "sessions": len(rows),
        "samples_per_session": 201,
        "median_p50_ns": statistics.median(int(row["p50_ns"]) for row in rows),
        "median_p95_ns": statistics.median(int(row["p95_ns"]) for row in rows),
        "allocations_per_session": [int(row["allocations"]) for row in rows],
        "allocated_bytes_per_session": [int(row["bytes"]) for row in rows],
    }
assert len(summary) == 12
print(json.dumps(summary, indent=2))
