#!/usr/bin/env python3
"""
Helper for running the benchmark and dhat profiles in this repo.

Examples:
  # run all criterion benches
  ./tools/bench.py run

  # run only pipeline bench with extra criterion args
  ./tools/bench.py run --bench pipeline -- --baseline main

  # run all dhat profiles
  ./tools/bench.py dhat

  # list known benches
  ./tools/bench.py list
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path
from collections.abc import Mapping
from dataclasses import dataclass

REPO_ROOT = Path(__file__).resolve().parent.parent

CRITERION_BENCHES = {
    "pipeline": "criterion_pipeline",
    "transformers": "criterion_transformers",
    "io": "criterion_io",
}

DHAT_BENCHES = {
    "build": "dhat_build",
    "transformers": "dhat_transformers",
    "compress": "dhat_compress",
}


@dataclass
class RunArgs:
    bench: str
    native: bool
    extra: list[str]


@dataclass
class DhatArgs:
    bench: str
    extra: list[str]


class Args(argparse.Namespace):
    command: str
    bench: str
    native: bool
    extra: list[str]

    def __init__(self) -> None:
        super().__init__()
        self.command = ""
        self.bench = ""
        self.native = False
        self.extra = []


def run_cmd(cmd: list[str]) -> int:
    print(f"+ {' '.join(cmd)}")
    return subprocess.call(cmd, cwd=REPO_ROOT)


def parse_args() -> Args:
    parser = argparse.ArgumentParser(description="Benchmark orchestrator for ssg.")
    sub = parser.add_subparsers(dest="command", required=True)

    run_p = sub.add_parser("run", help="Run criterion benches")
    _ = run_p.add_argument(
        "--bench",
        choices=["all", *CRITERION_BENCHES.keys()],
        default="all",
        help="Select which criterion bench group to run.",
    )
    _ = run_p.add_argument(
        "--native",
        action="store_true",
        help="Set RUSTFLAGS='-C target-cpu=native' for this run.",
    )
    _ = run_p.add_argument(
        "extra",
        nargs=argparse.REMAINDER,
        help="Extra args passed after -- to cargo/criterion (e.g. -- --baseline main).",
    )

    dhat_p = sub.add_parser("dhat", help="Run dhat profile benches")
    _ = dhat_p.add_argument(
        "--bench",
        choices=["all", *DHAT_BENCHES.keys()],
        default="all",
        help="Select which dhat bench to run.",
    )
    _ = dhat_p.add_argument(
        "extra",
        nargs=argparse.REMAINDER,
        help="Extra args passed after -- to the bench (e.g. -- --profile).",
    )

    _ = sub.add_parser("list", help="List known benches")

    parsed = parser.parse_args(namespace=Args())
    return parsed


def benches_to_run(selected: str, table: Mapping[str, str]) -> list[str]:
    if selected == "all":
        return list(table.values())
    return [table[selected]]


def handle_run(args: RunArgs) -> int:
    env: dict[str, str] = os.environ.copy()
    native: bool = bool(args.native)
    bench_choice: str = args.bench
    extra: list[str] = list(args.extra)

    if native:
        env["RUSTFLAGS"] = env.get("RUSTFLAGS", "") + " -C target-cpu=native"

    benches = list(benches_to_run(bench_choice, CRITERION_BENCHES))
    if not benches:
        print("No criterion benches selected.", file=sys.stderr)
        return 1

    code = 0
    for bench in benches:
        cmd = ["cargo", "bench", "--bench", bench]
        cmd.extend(extra)
        print(f"\n=== Running {bench} ===")
        code |= subprocess.call(cmd, cwd=REPO_ROOT, env=env)
    return code


def handle_dhat(args: DhatArgs) -> int:
    bench_choice: str = args.bench
    extra: list[str] = list(args.extra)

    benches = list(benches_to_run(bench_choice, DHAT_BENCHES))
    if not benches:
        print("No dhat benches selected.", file=sys.stderr)
        return 1

    code = 0
    for bench in benches:
        # dhat benches should receive --profile to emit json traces.
        cmd = ["cargo", "bench", "--bench", bench, "--"]
        # ensure --profile is present unless user already set something.
        if not extra:
            cmd.append("--profile")
        cmd.extend(extra)
        print(f"\n=== Running {bench} ===")
        code |= subprocess.call(cmd, cwd=REPO_ROOT)
    return code


def handle_list() -> int:
    print("Criterion benches:")
    for k, v in CRITERION_BENCHES.items():
        print(f"  {k:12} -> {v}")
    print("\nDhat benches:")
    for k, v in DHAT_BENCHES.items():
        print(f"  {k:12} -> {v}")
    return 0


def main() -> int:
    args = parse_args()
    command: str = args.command
    if command == "run":
        run_args = RunArgs(
            bench=str(args.bench),
            native=bool(args.native),
            extra=list(args.extra),
        )
        return handle_run(run_args)
    if command == "dhat":
        dhat_args = DhatArgs(
            bench=str(args.bench),
            extra=list(args.extra),
        )
        return handle_dhat(dhat_args)
    if command == "list":
        return handle_list()
    return 1


if __name__ == "__main__":
    sys.exit(main())
