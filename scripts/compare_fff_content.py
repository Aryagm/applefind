#!/usr/bin/env python3

import argparse
import re
import subprocess
from pathlib import Path


BUILD_ROW = re.compile(r"^(?P<label>apple build|fff build)\s+:\s+(?P<value>\S+)$")
RESULT_ROW = re.compile(
    r"^(?P<query>\S+)\s+(?P<applefind>\S+)\s+(?P<fff>\S+)\s+(?P<speedup>\S+)\s+(?P<candidates>\d+)\s+(?P<apple_matches>\d+)\s+(?P<fff_matches>\d+)$"
)

QUERY_SETS: dict[str, list[str]] = {
    "default": [
        "QueryParser",
        "parse_grep_query",
        "fuzzy_search",
        "GrepMode",
        "TODO",
    ],
    "linux": [
        "module_init",
        "copy_from_user",
        "spin_lock_irqsave",
        "EXPORT_SYMBOL_GPL",
        "of_match_ptr",
        "dma_alloc_coherent",
    ],
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Compare applefind exact content grep against fff plain-text grep."
    )
    parser.add_argument("--root", required=True, help="Corpus root to benchmark")
    parser.add_argument(
        "--applefind-manifest",
        default=str(Path(__file__).resolve().parents[1] / "Cargo.toml"),
        help="Path to applefind Cargo.toml",
    )
    parser.add_argument("--iters", type=int, default=5, help="Iterations per query")
    parser.add_argument("--limit", type=int, default=200, help="Maximum returned matches")
    parser.add_argument(
        "--query-set",
        choices=sorted(QUERY_SETS.keys()),
        default="default",
        help="Named query set to run when no explicit --query values are provided",
    )
    parser.add_argument(
        "--query",
        action="append",
        default=[],
        help="Explicit query to benchmark; can be repeated",
    )
    return parser.parse_args()


def run(cmd: list[str]) -> str:
    result = subprocess.run(cmd, check=True, text=True, capture_output=True)
    return result.stdout + result.stderr


def parse_output(output: str) -> tuple[dict[str, str], list[dict[str, str]]]:
    builds: dict[str, str] = {}
    rows: list[dict[str, str]] = []

    for line in output.splitlines():
        line = line.rstrip()
        if not line:
            continue

        build_match = BUILD_ROW.match(line)
        if build_match:
            builds[build_match.group("label")] = build_match.group("value")
            continue

        row_match = RESULT_ROW.match(line)
        if row_match:
            rows.append(row_match.groupdict())

    return builds, rows


def format_markdown(root: Path, iters: int, limit: int, builds: dict[str, str], rows: list[dict[str, str]]) -> str:
    lines = [
        f"repo: `{root}`",
        f"iterations: `{iters}`",
        f"limit: `{limit}`",
        f"apple build: `{builds.get('apple build', '-')}`",
        f"fff build: `{builds.get('fff build', '-')}`",
        "",
        "| query | applefind | fff | speedup | candidates | apple matches | fff matches |",
        "|---|---:|---:|---:|---:|---:|---:|",
    ]

    for row in rows:
        lines.append(
            f"| `{row['query']}` | {row['applefind']} | {row['fff']} | {row['speedup']} | {row['candidates']} | {row['apple_matches']} | {row['fff_matches']} |"
        )

    return "\n".join(lines)


def main() -> int:
    args = parse_args()
    root = Path(args.root).resolve()
    manifest = Path(args.applefind_manifest).resolve()
    queries = args.query or QUERY_SETS[args.query_set]

    cmd = [
        "cargo",
        "run",
        "--release",
        "--manifest-path",
        str(manifest),
        "--features",
        "compare-fff",
        "--bin",
        "compare-fff-grep",
        "--",
        "--root",
        str(root),
        "--iters",
        str(args.iters),
        "--limit",
        str(args.limit),
    ]
    for query in queries:
        cmd.extend(["--query", query])

    output = run(cmd)
    builds, rows = parse_output(output)
    print(format_markdown(root, args.iters, args.limit, builds, rows))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
