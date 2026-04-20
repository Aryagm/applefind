#!/usr/bin/env python3

import argparse
import os
import re
import subprocess
from pathlib import Path


APPLEFIND_ROW = re.compile(
    r"^(?P<query>.+?)\s+(?P<scan>\S+)\s+(?P<indexed>\S+)\s+(?P<speedup>\S+)\s+(?P<candidates>\d+)\s+(?P<hits>\d+)$"
)
FFF_ROW = re.compile(
    r"^(?P<name>.{1,21})\s+\|\s+(?P<iters>\d+)\s+\|\s+(?P<total>\S+)\s+\|\s+(?P<avg>\d+)µs\s+\|\s+(?P<matches>\d+)$"
)

FFF_TO_QUERY = {
    "short_common": "mod",
    "medium_specific": "controller",
    "long_rare": "user_authentication",
    "typo_resistant": "contrlr",
    "path_like": "src/lib",
    "two_char": "st",
    "partial_word": "test",
    "deep_path": "drivers/net",
    "extension": ".rs",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Compare applefind and fff on the same corpus.")
    parser.add_argument("--root", required=True, help="Corpus root to benchmark")
    parser.add_argument(
        "--applefind-manifest",
        default=str(Path(__file__).resolve().parents[1] / "Cargo.toml"),
        help="Path to applefind Cargo.toml",
    )
    parser.add_argument("--fff-repo", required=True, help="Path to fff.nvim checkout")
    parser.add_argument("--iters", type=int, default=10, help="Iterations for applefind")
    return parser.parse_args()


def run(cmd: list[str], cwd: str | None = None) -> str:
    result = subprocess.run(cmd, cwd=cwd, check=True, text=True, capture_output=True)
    return result.stdout + result.stderr


def parse_applefind(output: str) -> dict[str, dict[str, str]]:
    rows: dict[str, dict[str, str]] = {}
    for line in output.splitlines():
        match = APPLEFIND_ROW.match(line.rstrip())
        if not match:
            continue
        query = match.group("query").strip()
        rows[query] = {
            "applefind": match.group("indexed"),
            "applefind_speedup": match.group("speedup"),
            "candidates": match.group("candidates"),
            "hits": match.group("hits"),
        }
    return rows


def parse_fff(output: str) -> dict[str, dict[str, str]]:
    rows: dict[str, dict[str, str]] = {}
    for line in output.splitlines():
        match = FFF_ROW.match(line.rstrip())
        if not match:
            continue
        name = match.group("name").strip()
        query = FFF_TO_QUERY.get(name)
        if query is None:
            continue
        micros = int(match.group("avg"))
        rows[query] = {
            "fff": f"{micros / 1000.0:.2f}ms" if micros >= 1000 else f"{micros}us",
            "fff_matches": match.group("matches"),
        }
    return rows


def format_markdown(applefind: dict[str, dict[str, str]], fff: dict[str, dict[str, str]]) -> str:
    queries = [
        "mod",
        "controller",
        "user_authentication",
        "contrlr",
        "src/lib",
        "st",
        "test",
        "drivers/net",
        ".rs",
    ]
    lines = ["| query | applefind | fff | applefind candidates | applefind hits |", "|---|---:|---:|---:|---:|"]
    for query in queries:
        af = applefind.get(query, {})
        ff = fff.get(query, {})
        lines.append(
            f"| `{query}` | {af.get('applefind', '-')} | {ff.get('fff', '-')} | {af.get('candidates', '-')} | {af.get('hits', '-')} |"
        )
    return "\n".join(lines)


def main() -> int:
    args = parse_args()
    root = Path(args.root).resolve()
    manifest = Path(args.applefind_manifest).resolve()
    fff_repo = Path(args.fff_repo).resolve()

    applefind_output = run(
        [
            "cargo",
            "run",
            "--release",
            "--manifest-path",
            str(manifest),
            "--",
            "bench",
            "--root",
            str(root),
            "--iters",
            str(args.iters),
        ]
    )

    run(["cargo", "build", "--release", "-p", "fff-nvim", "--bin", "bench_search_only"], cwd=str(fff_repo))
    big_repo = fff_repo / "big-repo"
    if big_repo.exists() or big_repo.is_symlink():
        if big_repo.is_dir() and not big_repo.is_symlink():
            raise SystemExit(f"{big_repo} exists and is a directory")
        big_repo.unlink()
    big_repo.symlink_to(root)
    try:
        fff_output = run([str(fff_repo / "target/release/bench_search_only")], cwd=str(fff_repo))
    finally:
        if big_repo.is_symlink():
            big_repo.unlink()

    applefind_rows = parse_applefind(applefind_output)
    fff_rows = parse_fff(fff_output)
    print(format_markdown(applefind_rows, fff_rows))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
