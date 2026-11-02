"""Summarize raw paired observations; no sum of stage percentiles."""
import json
import math
import pathlib
import statistics

root = pathlib.Path(__file__).resolve().parent


def p95(values):
    return sorted(values)[math.ceil(len(values) * 0.95) - 1]


result = {}
for path in sorted(root.glob("session-*.jsonl")):
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    session = {}
    for name in sorted({r["workload"] for r in rows if r["kind"] == "sample"}):
        cases = {}
        for variant in ("fresh", "clone-rewrap"):
            samples = [r for r in rows if r["kind"] == "sample"
                       and r["workload"] == name and r["path"] == variant]
            assert len(samples) == 96
            cases[variant] = {
                "samples": len(samples),
                "total_p50_ms": statistics.median(r["total_ns"] for r in samples) / 1e6,
                "total_p95_ms": p95([r["total_ns"] for r in samples]) / 1e6,
                "stages": {
                    stage: {
                        "p95_ms": p95([r[stage]["ns"] for r in samples]) / 1e6,
                        "median_requested_bytes": statistics.median(
                            r[stage]["requested_bytes"] for r in samples),
                        "median_allocations": statistics.median(
                            r[stage]["allocations"] for r in samples),
                    }
                    for stage in ("prepare", "wrap", "align", "export", "provider_move")
                },
            }
        cases["total_p95_reduction_percent"] = 100 * (
            1 - cases["clone-rewrap"]["total_p95_ms"] / cases["fresh"]["total_p95_ms"])
        session[name] = cases
    result[path.name] = session
output = root / "summary.json"
output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
for name in next(iter(result.values())):
    print(name)
    for session, data in result.items():
        case = data[name]
        print(session, "p95", round(case["fresh"]["total_p95_ms"], 3),
              "->", round(case["clone-rewrap"]["total_p95_ms"], 3),
              "ms", round(case["total_p95_reduction_percent"], 1), "%")
