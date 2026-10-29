"""Prints the benchmark results as a markdown table, from criterion's own
estimate files: median time per benchmark, and for each group the ratio of
the best form-net entry to the best competitor.

Usage: python benches/report.py <criterion dir>   (CARGO_TARGET_DIR/criterion)
"""

import json
import os
import sys


def median_ns(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)["median"]["point_estimate"]


def fmt(ns):
    for unit, scale in (("s", 1e9), ("ms", 1e6), ("µs", 1e3)):
        if ns >= scale:
            return f"{ns / scale:.3g} {unit}"
    return f"{ns:.3g} ns"


def main(root):
    groups = {}
    for dirpath, _dirs, files in os.walk(root):
        if "estimates.json" in files and os.path.basename(dirpath) == "new":
            bench_dir = os.path.dirname(dirpath)
            rel = os.path.relpath(bench_dir, root).replace("\\", "/")
            group, _, bench = rel.rpartition("/")
            with open(os.path.join(dirpath, "benchmark.json"), encoding="utf-8") as f:
                meta = json.load(f)
            name = meta.get("function_id") or bench
            group = meta.get("group_id") or group
            groups.setdefault(group, {})[name] = median_ns(os.path.join(dirpath, "estimates.json"))
    print("| Group | Benchmark | Median | form-net vs best competitor |")
    print("|---|---|---|---|")
    for group in sorted(groups):
        entries = groups[group]
        ours = [v for k, v in entries.items() if k.startswith("form-net")]
        theirs = [v for k, v in entries.items() if not k.startswith("form-net")]
        ratio = ""
        if ours and theirs:
            ratio = f"{min(theirs) / min(ours):.2f}x"
        for i, (name, ns) in enumerate(sorted(entries.items(), key=lambda kv: kv[1])):
            print(f"| {group if i == 0 else ''} | {name} | {fmt(ns)} | {ratio if i == 0 else ''} |")


if __name__ == "__main__":
    main(sys.argv[1])
