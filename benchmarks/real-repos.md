# Real Repo Benchmarks

These numbers were collected on April 20, 2026 with:

- `applefind` at commit `34f8098`
- `fff.nvim` at commit `91545f8`
- a real shallow checkout of `torvalds/linux` at `da6b5aae`
- the same working tree used for both engines

This page complements [`benchmarks/popular-repos.md`](popular-repos.md):

- `popular-repos.md` isolates path-search behavior by materializing empty-file
  trees from large repos
- this page uses a real repo checkout so we can measure the same two surfaces
  `fff` exposes in practice: fuzzy file search and exact plain-text grep

## Linux Checkout Notes

The checkout was created with:

```bash
git clone --depth 1 https://github.com/torvalds/linux.git /tmp/linux
```

On a default macOS case-insensitive filesystem, a few Linux paths collide by
case. Both engines see the same working tree, so the comparison remains fair,
but the collected file count is slightly below the full tracked-path count.

## File Search

Path-search numbers on the real Linux checkout came from:

```bash
python3 applefind/scripts/compare_fff.py --root /tmp/linux --fff-repo /tmp/fff.nvim --iters 5
```

| query | applefind | fff | apple hits | fff hits | apple candidates | cand pct | overlap@10 | top1 same |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `mod` | 202us | 3.69ms | 750 | 91747 | 750 | 0.80% | 0 | false |
| `controller` | 432us | 4.03ms | 476 | 1133 | 476 | 0.51% | 3 | false |
| `user_authentication` | 1.19ms | 11.90ms | 0 | 8 | 0 | 0.00% | 0 | false |
| `contrlr` | 11.03ms | 4.71ms | 1 | 10457 | 1 | 0.00% | 0 | false |
| `src/lib` | 4us | 4.62ms | 0 | 14125 | 0 | 0.00% | 0 | false |
| `st` | 2.90ms | 3.31ms | 18271 | 91171 | 93774 | 100.00% | 0 | false |
| `test` | 3.72ms | 4.41ms | 7404 | 83217 | 7404 | 7.90% | 0 | false |
| `drivers/net` | 540us | 6.42ms | 7023 | 30716 | 7023 | 7.49% | 0 | false |
| `.rs` | 761us | 3.81ms | 4325 | 93647 | 4325 | 4.61% | 0 | false |

These numbers are useful, but they also show the current limitation clearly:
latency is strong, while fuzzy-search ranking parity with `fff` is still not
there on broad or typo-heavy queries.

## Content Search

Exact content-grep numbers on the same Linux checkout came from:

```bash
python3 applefind/scripts/compare_fff_content.py --root /tmp/linux --query-set linux --iters 3 --limit 200
```

| query | applefind | fff | speedup | candidates | apple matches | fff matches |
|---|---:|---:|---:|---:|---:|---:|
| `module_init` | 3.77ms | 130.72ms | 34.62x | 14543 | 200 | 200 |
| `copy_from_user` | 1.57ms | 59.09ms | 37.63x | 1629 | 200 | 200 |
| `spin_lock_irqsave` | 1.36ms | 64.35ms | 47.23x | 3800 | 200 | 203 |
| `EXPORT_SYMBOL_GPL` | 1.28ms | 62.18ms | 48.40x | 3716 | 200 | 202 |
| `of_match_ptr` | 12.37ms | 235.26ms | 19.02x | 1553 | 200 | 200 |
| `dma_alloc_coherent` | 3.15ms | 518.62ms | 164.52x | 1369 | 200 | 201 |

Build times for the validated script run:

- `applefind`: `25.925364084s`
- `fff`: `715.029791ms`

That upfront cost is the current trade: the prototype pays a heavy one-time
build cost to get much lower per-query latency. Until the content index is
persistent and incremental, these numbers are best understood as a resident
index story, not a one-shot grep replacement story.
