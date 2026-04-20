#!/usr/bin/env python3

import argparse
import os
import shutil
import subprocess
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Materialize an empty file tree from a git commit for path-search benchmarks."
    )
    parser.add_argument("git_repo", help="Path to a git repository")
    parser.add_argument("output", help="Directory to create")
    parser.add_argument(
        "--rev",
        default="HEAD",
        help="Git revision to materialize (default: HEAD)",
    )
    parser.add_argument(
        "--keep",
        action="store_true",
        help="Keep the output directory if it already exists",
    )
    return parser.parse_args()


def list_paths(repo: str, rev: str) -> list[str]:
    result = subprocess.run(
        ["git", "-C", repo, "ls-tree", "-r", "-z", "--name-only", rev],
        check=True,
        capture_output=True,
    )
    return [os.fsdecode(raw) for raw in result.stdout.split(b"\0") if raw]


def main() -> int:
    args = parse_args()
    repo = Path(args.git_repo)
    out = Path(args.output)

    if not repo.exists():
        raise SystemExit(f"git repo does not exist: {repo}")

    if out.exists() and not args.keep:
        shutil.rmtree(out)
    out.mkdir(parents=True, exist_ok=True)

    count = 0
    for rel in list_paths(str(repo), args.rev):
        target = out / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        target.touch(exist_ok=True)
        count += 1

    print(count)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

