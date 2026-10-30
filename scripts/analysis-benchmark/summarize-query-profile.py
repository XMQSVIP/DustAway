"""Summarize opt-in query phase timings; these are diagnostics, not acceptance runs."""
import json
import math
import pathlib
import statistics
import sys

path = pathlib.Path(sys.argv[1])
events = [json.loads(line) for line in path.read_text(encoding="utf-8-sig").splitlines() if line.strip()]
summary = {}
for kind in ["reader", "page", "count"]:
    rows = [event for event in events if event.get("query_timing") == kind]
    phases = {}
    for row in rows:
        for phase, micros in row["phases"]:
            phases.setdefault(phase, []).append(micros / 1000)
    summary[kind] = {
        "samples": len(rows),
        "phases_ms": {
            phase: {
                "median": statistics.median(values),
                "p95": sorted(values)[math.ceil(len(values) * .95) - 1],
                "max": max(values),
            }
            for phase, values in phases.items()
        },
        "slowest": sorted(rows, key=lambda row: row["total_us"], reverse=True)[:5],
    }
output = json.dumps(summary, ensure_ascii=False, indent=2) + "\n"
path.with_suffix(".summary.json").write_text(output, encoding="utf-8")
print(output)
