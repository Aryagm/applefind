# Popular Repo Benchmarks

These numbers were collected on April 20, 2026 with:

- `applefind` at commit `b8fd1ed` plus the publish-prep changes in this working tree
- `fff.nvim` at commit `91545f8`
- metadata-only clones materialized into empty-file trees
- the same hidden/ignore behavior applied to both engines

Why empty-file trees? This is a path-search benchmark. Materializing the git
tree shape without file contents isolates path indexing and matching latency
from file-size effects.

## Corpus Summary

| repo | commit | tracked paths | collected entries | load time | build time |
|---|---:|---:|---:|---:|---:|
| Chromium | `2f4ad52b` | 493,672 | 447,250 | `862.47ms` | `1.68s` |
| Linux | `da6b5aa` | 93,887 | 93,412 | `164.90ms` | `217.71ms` |
| Rust | `91367b0` | 59,211 | 58,936 | `108.65ms` | `189.96ms` |
| VS Code | `7f7a471` | 14,635 | 13,106 | `82.20ms` | `49.72ms` |

`load time` and `build time` come from:

```bash
applefind bench --root <corpus> --iters 10
```

The `fff` comparisons come from:

```bash
python3 applefind/scripts/compare_fff.py --root <corpus> --fff-repo /tmp/fff.nvim --iters 10
```

## Chromium

Both engines indexed `447,250` files.

| query | applefind | fff | applefind candidates | applefind hits |
|---|---:|---:|---:|---:|
| `mod` | 1.26ms | 25.91ms | 25721 | 25721 |
| `controller` | 632us | 56.47ms | 5777 | 5774 |
| `user_authentication` | 101us | 109.23ms | 2 | 2 |
| `contrlr` | 14.44ms | 43.26ms | 10884 | 7292 |
| `src/lib` | 59us | 48.31ms | 141 | 141 |
| `st` | 3.40ms | 22.00ms | 309934 | 309934 |
| `test` | 3.15ms | 31.11ms | 232845 | 232757 |
| `drivers/net` | 17us | 68.28ms | 0 | 0 |
| `.rs` | 210us | 29.49ms | 6411 | 6411 |

## Linux

Both engines indexed the same materialized Linux tree. `applefind` collected
`93,412` entries after ignore filtering.

| query | applefind | fff | applefind candidates | applefind hits |
|---|---:|---:|---:|---:|
| `mod` | 26us | 3.67ms | 743 | 743 |
| `controller` | 59us | 4.01ms | 476 | 476 |
| `user_authentication` | 1.31ms | 11.75ms | 0 | 0 |
| `contrlr` | 3us | 4.68ms | 1 | 1 |
| `src/lib` | 4us | 4.62ms | 0 | 0 |
| `st` | 636us | 3.84ms | 18024 | 18024 |
| `test` | 384us | 5.30ms | 7261 | 7260 |
| `drivers/net` | 527us | 7.58ms | 7032 | 7032 |
| `.rs` | 143us | 4.03ms | 4325 | 4325 |

## Rust

Both engines indexed the same materialized Rust tree. `applefind` collected
`58,936` entries after ignore filtering.

| query | applefind | fff | applefind candidates | applefind hits |
|---|---:|---:|---:|---:|
| `mod` | 44us | 2.63ms | 1548 | 1548 |
| `controller` | 1.40ms | 4.70ms | 28 | 0 |
| `user_authentication` | 1.67ms | 9.79ms | 0 | 0 |
| `contrlr` | 896us | 4.26ms | 325 | 32 |
| `src/lib` | 31us | 4.13ms | 7 | 6 |
| `st` | 972us | 1.97ms | 58936 | 56990 |
| `test` | 1.08ms | 2.52ms | 58936 | 51391 |
| `drivers/net` | 0us | 4.36ms | 0 | 0 |
| `.rs` | 660us | 2.25ms | 36704 | 36704 |

## VS Code

Both engines indexed the same materialized VS Code tree. `applefind` collected
`13,106` entries after ignore filtering.

| query | applefind | fff | applefind candidates | applefind hits |
|---|---:|---:|---:|---:|
| `mod` | 20us | 1.65ms | 546 | 546 |
| `controller` | 25us | 1.94ms | 100 | 100 |
| `user_authentication` | 363us | 2.75ms | 2 | 0 |
| `contrlr` | 450us | 2.18ms | 491 | 116 |
| `src/lib` | 9us | 1.36ms | 18 | 10 |
| `st` | 268us | 1.40ms | 6077 | 6077 |
| `test` | 244us | 1.87ms | 4363 | 4362 |
| `drivers/net` | 0us | 1.52ms | 0 | 0 |
| `.rs` | 4us | 1.42ms | 76 | 76 |

## Notes

- The strongest results are on selective and path-like queries because the
  candidate planner can discard most of the repo before scoring.
- The current engine uses a hybrid approach: index-first candidate pruning, then
  fuzzy reranking on the reduced set for normal file-search queries.
- Broad queries still need work. When candidate counts approach the full corpus,
  the win shrinks or disappears.
- `applefind` is not yet a semantic clone of `fff`. A few result counts differ
  on typo-heavy or broad fuzzy queries. The current claim is latency and
  architecture, not identical ranking.
- Use `compare-fff-quality` for overlap and mismatch examples when you want to
  measure result quality instead of latency.
