# applefind

Prototype Apple-focused file search core.

Current direction:

- Apple-focused path search
- indexed candidate generation first
- exact/fuzzy-ish scoring over the reduced set
- benchmark CLI and `fff` comparison harness

The point is to measure the first architectural bet:

> candidate generation first, exact scoring second

instead of scoring the entire corpus on every query.

## Commands

```bash
cargo run --release --manifest-path applefind/Cargo.toml -- bench --synthetic 250000 --iters 100
```

```bash
cargo run --release --manifest-path applefind/Cargo.toml -- search --root . controller
```

Build an empty-file corpus from any git tree:

```bash
python3 applefind/scripts/materialize_git_tree.py /tmp/chromium-src-meta /tmp/chromium-empty
```

Compare `applefind` to `fff` on the same corpus:

```bash
python3 applefind/scripts/compare_fff.py --root /tmp/chromium-empty --fff-repo /tmp/fff.nvim --iters 10
```

## Current Matching

This version combines:

- basename exact
- basename prefix
- basename contains
- path contains
- basename acronym hits
- bounded typo fallback
- fuzzy-ish ordered subsequence fallback for basename terms

Tokens containing `/` or `\` are treated as path tokens. Other tokens are
matched against basename first, then full path.

It is still not a drop-in semantic clone of `fff`'s fuzzy engine. The current
goal is to prove the systems architecture on large corpora, then tighten fuzzy
behavior without losing the indexed speed path.

## Chromium Result

Using Chromium `src.git` metadata at commit `2f4ad52b`, materialized as an
empty-file tree and collected with the same hidden/ignore behavior as `fff`,
both engines indexed `447,250` files.

`applefind`:

- load: `1.33s`
- build: `1.77s`

`fff`:

- load: `1.75s`

Selected query results:

| query | applefind | fff |
|---|---:|---:|
| `mod` | `1.17ms` | `28.68ms` |
| `controller` | `612us` | `55.79ms` |
| `user_authentication` | `98us` | `104.67ms` |
| `contrlr` | `15.30ms` | `41.76ms` |
| `src/lib` | `59us` | `51.78ms` |
| `st` | `3.43ms` | `19.65ms` |
| `test` | `3.66ms` | `29.01ms` |

This is the current state, not the final claim. The selective and path-like
queries are already much faster. Broad fuzzy queries still need better matching
and better pruning.

## Next Steps

1. Tighten fuzzy semantics to match `fff` more closely.
2. Improve broad-query pruning so `st`-class queries also pull ahead.
3. Add a direct compare binary instead of parsing profiler output.
4. Add a resident daemon and incremental query refinement.
5. Add a Metal backend for content search, not path search.
