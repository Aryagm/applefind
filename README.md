# applefind

Fast indexed path search core in Rust.

`applefind` is built around one bet:

> generate a small candidate set first, then score it

instead of fuzzy-scoring the entire repo on every query. The current engine
indexes basename/path characters, bigrams, trigrams, path components, and
acronyms, intersects those postings per token, and only then runs exact,
path-like, acronym, typo, or subsequence scoring on the reduced set.

This is a portable Rust core. It runs on macOS, Linux, and Windows. Apple-
specific acceleration is a future path for content search, not a dependency for
the current path search engine.

## What It Does Today

- repo-scale path collection with `ignore`
- indexed candidate generation over path metadata
- `neo_frizbee` reranking over reduced fuzzy candidate sets
- bounded typo and acronym matching
- experimental exact content index with trigram candidate pruning
- benchmark CLI for scan vs indexed comparisons
- `fff.nvim` comparison harness on identical corpora

## Quick Start

Run a synthetic benchmark:

```bash
cargo run --release --manifest-path applefind/Cargo.toml -- bench --synthetic 250000 --iters 100
```

Search the current repo:

```bash
cargo run --release --manifest-path applefind/Cargo.toml -- search --root . controller
```

Search the current repo in exact mode:

```bash
cargo run --release --manifest-path applefind/Cargo.toml -- search --root . --mode exact controller
```

Materialize any git tree as an empty-file corpus:

```bash
python3 applefind/scripts/materialize_git_tree.py /tmp/rust-meta /tmp/rust-empty
```

Compare `applefind` to `fff` on the same corpus:

```bash
python3 applefind/scripts/compare_fff.py --root /tmp/rust-empty --fff-repo /tmp/fff.nvim --iters 10
```

Compare exact-mode `applefind` to `fff` on the exact query subset:

```bash
python3 applefind/scripts/compare_fff.py --root /tmp/rust-empty --fff-repo /tmp/fff.nvim --iters 10 --applefind-mode exact --query-set exact
```

Compare result quality directly against `fff`:

```bash
cargo run --release --manifest-path applefind/Cargo.toml --features compare-fff --bin compare-fff-quality -- --root /tmp/rust-empty --format plain
```

Compare exact content grep against `fff` plain-text grep:

```bash
cargo run --release --manifest-path applefind/Cargo.toml --features compare-fff --bin compare-fff-grep -- --root /tmp/fff.nvim --iters 5 --limit 200
```

Format exact content grep results as markdown:

```bash
python3 applefind/scripts/compare_fff_content.py --root /tmp/linux --query-set linux --iters 3 --limit 200
```

Compare exact content grep directly against `ripgrep`:

```bash
python3 applefind/scripts/compare_rg_content.py --root /tmp/linux --query-set linux --iters 3 --limit 200
```

## Benchmark Snapshot

Fresh runs on April 20, 2026:

| repo | collected files | applefind | fff |
|---|---:|---:|---:|
| Chromium `2f4ad52b` | 447,250 | `controller` `632us` | `56.47ms` |
| Linux `da6b5aa` | 93,412 | `drivers/net` `527us` | `7.58ms` |
| Rust `91367b0` | 58,936 | `src/lib` `31us` | `4.13ms` |
| VS Code `7f7a471` | 13,106 | `controller` `25us` | `1.94ms` |

Full tables, corpus counts, and methodology live in
[`benchmarks/popular-repos.md`](benchmarks/popular-repos.md) and
[`benchmarks/real-repos.md`](benchmarks/real-repos.md).

## Why It Is Faster

The current speedup comes from a few simple systems choices:

1. `applefind` does not start by scoring every path. It starts by asking which
   paths could possibly match.
2. Query planning is cheap. For most queries it becomes a handful of posting
   lookups plus sorted-set intersections over IDs.
3. Full scoring only runs on survivors. On Chromium, a query like `controller`
   falls from `447,250` paths to `5,777` candidates before scoring.
4. Result selection avoids a full sort. The engine uses
   `select_nth_unstable_by` for top-k truncation and only sorts the retained
   frontier.
5. Fuzzy ranking can use `neo_frizbee`, but only after the candidate planner has
   already cut the corpus down.
6. Large candidate sets score in parallel with `rayon`.

That is why the win is strongest on selective and path-like queries. Broad
one-character or very common queries still collapse toward scan behavior, and
that is where the next round of work belongs.

## Current Matching

The current matcher combines:

- basename exact
- basename prefix
- basename contains
- path contains
- basename acronym hits
- bounded typo fallback
- ordered subsequence fallback for basename terms

Tokens containing `/` or `\` are treated as path tokens. Other tokens are
matched against basename first, then full path.

There is also an explicit `exact` mode for literal file/path search. In exact
mode the planner stays literal and the scorer only uses basename/path exact,
prefix, and substring matches; typo and acronym fallbacks are disabled.

This is not a drop-in semantic clone of `fff` yet. The benchmark claim to make
today is architectural:

> indexed candidate generation can cut interactive path-search latency by a lot
> on large corpora

not:

> this already reproduces every `fff` ranking decision

The repo now includes a dedicated quality harness for that gap. On path-like and
selective queries the overlap is often reasonable, and the current scorer is
closer to `fff` than the original exact-only ranking. Broad fuzzy queries are
still far off `fff`.

## Web Demo

There is a small static demo in [`docs/index.html`](docs/index.html). It runs a
browser-sized port of the query planner and scorer over a curated sample corpus
so people can try the interaction model without building the CLI.

Open `docs/index.html` directly in a browser, or serve the repo root:

```bash
python3 -m http.server
```

The demo is for feel and explanation. The benchmark numbers in this repo come
from the native Rust CLI.

## Exact Grep Prototype

The repo now also includes an experimental exact content index in
[`src/content.rs`](src/content.rs). It is a literal grep engine:

- file-level trigram candidate pruning
- exact line extraction
- lowercase-query smart-case approximation for ASCII-heavy codebases

This is still narrower than a full grep product. Regex search, content-side
fuzzy matching, persistent on-disk indexes, and incremental updates are still
open work.

## Publishability Gaps Still Open

- broad fuzzy queries still need better pruning
- matching semantics still need to close more of the gap with `fff`
- there is no persistent resident index yet
- regex grep, fuzzy grep, and incremental content indexing are future work

## Development

Run the test suite:

```bash
cargo test --manifest-path applefind/Cargo.toml
```

Run formatting and lints:

```bash
cargo fmt --manifest-path applefind/Cargo.toml --check
cargo clippy --manifest-path applefind/Cargo.toml --all-targets -- -D warnings
```
