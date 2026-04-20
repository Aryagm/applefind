const SAMPLE_PATHS = [
  "README.md",
  "docs/controller-notes.md",
  "docs/benchmarks/popular_repos.md",
  "docs/demo/index.html",
  "src/lib.rs",
  "src/lib/controller.ts",
  "src/auth/user_controller.rs",
  "src/auth/session_manager.rs",
  "src/chat/controller_view.swift",
  "src/platform/search/path_index.rs",
  "src/platform/search/query.rs",
  "src/platform/search/search_demo.rs",
  "src/platform/search/search_service.rs",
  "src/platform/search/tests/path_search_test.rs",
  "src/vs/workbench/contrib/files/browser/filesView.ts",
  "src/vs/workbench/contrib/search/browser/searchView.ts",
  "src/vs/workbench/services/extensions/common/extensionHostManager.ts",
  "src/vs/workbench/services/search/node/ripgrepSearchProvider.ts",
  "src/vs/workbench/test/browser/workbenchTestServices.ts",
  "content/browser/RenderFrameHostImpl.cc",
  "content/browser/renderer_host/render_widget_host_impl.cc",
  "content/common/navigation_params.h",
  "drivers/net/ethernet/intel/ice/ice_main.c",
  "drivers/net/wireless/ath/ath10k/core.c",
  "drivers/net/wireless/ath/ath11k/testmode.c",
  "tools/testing/selftests/bpf/test_maps.c",
  "tools/testing/selftests/net/nettest.sh",
  "library/std/src/io/mod.rs",
  "library/std/src/lib.rs",
  "library/test/src/lib.rs",
  "compiler/rustc_driver_impl/src/lib.rs",
  "compiler/rustc_hir/src/hir.rs",
  "compiler/rustc_hir_analysis/src/collect.rs",
  "compiler/rustc_middle/src/ty/context.rs",
  "compiler/rustc_parse/src/parser/mod.rs",
  "compiler/rustc_query_system/src/lib.rs",
  "extensions/github-authentication/src/github.ts",
  "extensions/markdown-language-features/src/tableOfContentsProvider.ts",
  "extensions/typescript-language-features/src/languageFeatures/completions.ts",
  "base/files/file_path.cc",
  "components/autofill/core/browser/autofill_manager.cc",
  "components/sync/service/sync_service_impl.cc",
  "app/controller/main_controller.swift",
  "app/controller/session_controller.swift",
  "app/model/user_authentication_state.ts",
  "packages/search-engine/src/index.ts",
  "packages/search-engine/src/query_planner.ts",
  "packages/search-engine/tests/query_planner.test.ts",
  "pkg/api/user_authentication.proto",
  "pkg/api/user_authentication_test.go",
  "pkg/gateway/client.go",
  "services/storage/blob_store.rs",
  "services/storage/blob_store_test.rs",
  "services/telemetry/controller_metrics.go",
  "services/telemetry/event_stream.rs",
  "modules/compression/metalzip_rt.swift",
  "modules/mlx/kv_cache_loader.swift",
  "modules/mlx/search_kernel.metal",
  "modules/mlx/weight_streaming.metal",
  "experiments/fuzzy/controller_alignment.rs",
  "experiments/fuzzy/contrlr_bench.rs",
  "test/controller_smoke_test.py",
  "test/mlx/search_kernel_test.swift",
  "test/search/index_bench_test.py",
  "vendor/sqlite/src/shell.c",
];

function bytesOf(text) {
  const out = [];
  for (let i = 0; i < text.length; i += 1) {
    out.push(text.charCodeAt(i) & 0xff);
  }
  return out;
}

function packBigram(a, b) {
  return (a << 8) | b;
}

function packTrigram(a, b, c) {
  return (a << 16) | (b << 8) | c;
}

function extractBigrams(bytes) {
  const out = [];
  for (let i = 0; i + 1 < bytes.length; i += 1) {
    out.push(packBigram(bytes[i], bytes[i + 1]));
  }
  return out;
}

function extractTrigrams(bytes) {
  const out = [];
  for (let i = 0; i + 2 < bytes.length; i += 1) {
    out.push(packTrigram(bytes[i], bytes[i + 1], bytes[i + 2]));
  }
  return out;
}

function pushPosting(map, key, id) {
  const existing = map.get(key);
  if (existing) {
    existing.push(id);
  } else {
    map.set(key, [id]);
  }
}

function uniqueSorted(values) {
  return Array.from(new Set(values)).sort((a, b) => a - b);
}

function extractComponents(path) {
  return Array.from(new Set(path.split(/[\\/]+/).filter(Boolean))).sort();
}

function stripExtension(name) {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

function extractTerms(text) {
  const out = [];
  let current = "";
  let prevIsLower = false;

  for (const ch of text) {
    if (!/[A-Za-z0-9]/.test(ch)) {
      if (current) {
        out.push(current);
        current = "";
      }
      prevIsLower = false;
      continue;
    }

    const isUpper = ch >= "A" && ch <= "Z";
    if (isUpper && prevIsLower && current) {
      out.push(current);
      current = "";
    }

    current += ch.toLowerCase();
    prevIsLower = ch >= "a" && ch <= "z";
  }

  if (current) {
    out.push(current);
  }

  return out;
}

function computeAcronym(text) {
  let out = "";
  for (const term of extractTerms(text)) {
    out += term[0] || "";
  }
  return out;
}

function unionSorted(left, right) {
  const out = [];
  let i = 0;
  let j = 0;

  while (i < left.length && j < right.length) {
    if (left[i] < right[j]) {
      out.push(left[i]);
      i += 1;
    } else if (left[i] > right[j]) {
      out.push(right[j]);
      j += 1;
    } else {
      out.push(left[i]);
      i += 1;
      j += 1;
    }
  }

  while (i < left.length) {
    out.push(left[i]);
    i += 1;
  }

  while (j < right.length) {
    out.push(right[j]);
    j += 1;
  }

  return out;
}

function intersectSorted(left, right) {
  const out = [];
  let i = 0;
  let j = 0;

  while (i < left.length && j < right.length) {
    if (left[i] < right[j]) {
      i += 1;
    } else if (left[i] > right[j]) {
      j += 1;
    } else {
      out.push(left[i]);
      i += 1;
      j += 1;
    }
  }

  return out;
}

function requiredBigramOverlap(total) {
  return Math.min(total, Math.max(2, total - 2));
}

function boundedEditDistance(left, right, maxDistance) {
  if (Math.abs(left.length - right.length) > maxDistance) {
    return null;
  }

  let previous = Array.from({ length: right.length + 1 }, (_, i) => i);
  let current = new Array(right.length + 1).fill(0);

  for (let i = 0; i < left.length; i += 1) {
    current[0] = i + 1;
    let rowMin = current[0];

    for (let j = 0; j < right.length; j += 1) {
      const cost = left[i] === right[j] ? 0 : 1;
      const del = previous[j + 1] + 1;
      const ins = current[j] + 1;
      const rep = previous[j] + cost;
      const value = Math.min(del, ins, rep);
      current[j + 1] = value;
      rowMin = Math.min(rowMin, value);
    }

    if (rowMin > maxDistance) {
      return null;
    }

    [previous, current] = [current, previous];
  }

  return previous[right.length] <= maxDistance ? previous[right.length] : null;
}

function fuzzySubsequenceScore(haystack, needle) {
  if (!needle || needle.length > haystack.length) {
    return null;
  }

  let needleIndex = 0;
  let lastMatch = null;
  let gaps = 0;
  let currentStreak = 0;
  let bestStreak = 0;

  for (let i = 0; i < haystack.length; i += 1) {
    if (needleIndex >= needle.length) {
      break;
    }

    if (haystack[i].toLowerCase() === needle[needleIndex].toLowerCase()) {
      if (lastMatch !== null) {
        if (i === lastMatch + 1) {
          currentStreak += 1;
        } else {
          gaps += i - lastMatch - 1;
          currentStreak = 1;
        }
      } else {
        currentStreak = 1;
      }

      bestStreak = Math.max(bestStreak, currentStreak);
      lastMatch = i;
      needleIndex += 1;
    }
  }

  if (needleIndex !== needle.length) {
    return null;
  }

  return (
    12000 +
    needle.length * 700 +
    bestStreak * 350 -
    gaps * 40 -
    (haystack.length - needle.length) * 30
  );
}

function scoreAcronymMatch(basenameOriginal, needle) {
  if (needle.length < 2) {
    return null;
  }

  const acronym = computeAcronym(stripExtension(basenameOriginal));
  if (acronym === needle) {
    return 40000 - acronym.length * 8;
  }

  if (acronym.startsWith(needle)) {
    return 32000 - acronym.length * 8;
  }

  return null;
}

function scoreTypoBasenameMatch(basenameOriginal, needle) {
  if (needle.length < 4) {
    return null;
  }

  const stem = stripExtension(basenameOriginal);
  let best = null;

  for (const term of extractTerms(stem)) {
    if (term === needle) {
      return 48000 - term.length * 16;
    }

    if (Math.abs(term.length - needle.length) > 2) {
      const fuzzyScore = fuzzySubsequenceScore(term, needle);
      if (fuzzyScore !== null) {
        best = best === null ? fuzzyScore : Math.max(best, fuzzyScore);
      }
      continue;
    }

    const distance = boundedEditDistance(term, needle, 2);
    if (distance !== null) {
      let score = null;
      if (distance === 0) {
        score = 48000 - term.length * 16;
      } else if (distance === 1) {
        score = 20000 - term.length * 8;
      } else if (distance === 2) {
        score = 14000 - term.length * 8;
      }

      if (score !== null) {
        best = best === null ? score : Math.max(best, score);
        continue;
      }
    }

    const fuzzyScore = fuzzySubsequenceScore(term, needle);
    if (fuzzyScore !== null) {
      best = best === null ? fuzzyScore : Math.max(best, fuzzyScore);
    }
  }

  return best;
}

function parseQuery(raw) {
  return {
    raw,
    tokens: raw
      .split(/\s+/)
      .map((part) => part.trim())
      .filter(Boolean)
      .map((part) => ({
        text: part.toLowerCase(),
        field: /[\\/]/.test(part) ? "path" : "basename_or_path",
      })),
  };
}

function buildIndex(paths) {
  const entries = paths.map((path) => {
    const lower = path.toLowerCase();
    const basenameOffset = Math.max(lower.lastIndexOf("/"), lower.lastIndexOf("\\")) + 1;
    const basenameOriginalOffset = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\")) + 1;
    return {
      path,
      lower,
      basenameOffset,
      basenameOriginalOffset,
      depth: lower.split(/[\\/]/).length - 1,
    };
  });

  const basenameChars = Array.from({ length: 256 }, () => []);
  const pathChars = Array.from({ length: 256 }, () => []);
  const basenameBigrams = new Map();
  const basenameTrigrams = new Map();
  const basenameAcronyms = new Map();
  const pathBigrams = new Map();
  const pathTrigrams = new Map();
  const pathComponents = new Map();

  entries.forEach((entry, id) => {
    const basename = entry.lower.slice(entry.basenameOffset);
    const basenameOriginal = entry.path.slice(entry.basenameOriginalOffset);
    const basenameBytes = bytesOf(basename);
    const lowerBytes = bytesOf(entry.lower);

    const basenameSeen = new Set();
    const pathSeen = new Set();
    for (const byte of basenameBytes) {
      if (!basenameSeen.has(byte)) {
        basenameSeen.add(byte);
        basenameChars[byte].push(id);
      }
    }
    for (const byte of lowerBytes) {
      if (!pathSeen.has(byte)) {
        pathSeen.add(byte);
        pathChars[byte].push(id);
      }
    }

    for (const key of uniqueSorted(extractBigrams(basenameBytes))) {
      pushPosting(basenameBigrams, key, id);
    }
    for (const key of uniqueSorted(extractTrigrams(basenameBytes))) {
      pushPosting(basenameTrigrams, key, id);
    }
    for (const key of uniqueSorted(extractBigrams(lowerBytes))) {
      pushPosting(pathBigrams, key, id);
    }
    for (const key of uniqueSorted(extractTrigrams(lowerBytes))) {
      pushPosting(pathTrigrams, key, id);
    }
    for (const component of extractComponents(entry.lower)) {
      pushPosting(pathComponents, component, id);
    }

    const acronym = computeAcronym(stripExtension(basenameOriginal));
    if (acronym.length >= 2) {
      pushPosting(basenameAcronyms, acronym, id);
    }
  });

  return {
    entries,
    basenameChars,
    basenameBigrams,
    basenameTrigrams,
    basenameAcronyms,
    pathChars,
    pathBigrams,
    pathTrigrams,
    pathComponents,
  };
}

function candidatesForField(index, bytes, basename) {
  if (bytes.length === 0) {
    return [];
  }

  const chars = basename ? index.basenameChars : index.pathChars;
  const bigrams = basename ? index.basenameBigrams : index.pathBigrams;
  const trigrams = basename ? index.basenameTrigrams : index.pathTrigrams;
  const text = String.fromCharCode(...bytes);
  const components = !basename && /[\\/]/.test(text) ? extractComponents(text) : [];

  if (bytes.length === 1) {
    return chars[bytes[0]].slice();
  }

  if (bytes.length === 2) {
    return (bigrams.get(packBigram(bytes[0], bytes[1])) || []).slice();
  }

  const candidateSets = [];
  for (const component of components) {
    const posting = index.pathComponents.get(component);
    if (!posting) {
      return [];
    }
    candidateSets.push(posting);
  }

  const keys = uniqueSorted(extractTrigrams(bytes));
  if (keys.length === 0 && candidateSets.length === 0) {
    return [];
  }

  for (const key of keys) {
    const posting = trigrams.get(key);
    if (!posting) {
      return [];
    }
    candidateSets.push(posting);
  }

  candidateSets.sort((a, b) => a.length - b.length);
  let acc = candidateSets[0].slice();
  for (let i = 1; i < candidateSets.length; i += 1) {
    acc = intersectSorted(acc, candidateSets[i]);
    if (acc.length === 0) {
      break;
    }
  }
  return acc;
}

function approximateCandidates(index, bytes, basename) {
  const keys = uniqueSorted(extractBigrams(bytes));
  if (keys.length === 0) {
    return [];
  }

  const postings = basename ? index.basenameBigrams : index.pathBigrams;
  const required = requiredBigramOverlap(keys.length);
  const counts = new Map();

  for (const key of keys) {
    const posting = postings.get(key);
    if (!posting) {
      continue;
    }
    for (const id of posting) {
      counts.set(id, (counts.get(id) || 0) + 1);
    }
  }

  return Array.from(counts.entries())
    .filter(([, count]) => count >= required)
    .map(([id]) => id)
    .sort((a, b) => a - b);
}

function candidatesForToken(index, token) {
  const bytes = bytesOf(token.text);
  if (token.field === "path") {
    return candidatesForField(index, bytes, false);
  }

  const basename = candidatesForField(index, bytes, true);
  const path = candidatesForField(index, bytes, false);
  const exact = unionSorted(basename, path);
  const acronym = token.text.length >= 2 ? (index.basenameAcronyms.get(token.text) || []).slice() : [];
  const combined = unionSorted(exact, acronym);

  if (combined.length > 0 || token.text.length < 4) {
    return combined;
  }

  return approximateCandidates(index, bytes, true);
}

function candidatesForQuery(index, parsed) {
  const tokenCandidates = [];
  for (const token of parsed.tokens) {
    const candidates = candidatesForToken(index, token);
    if (candidates.length === 0) {
      return [];
    }
    tokenCandidates.push(candidates);
  }

  tokenCandidates.sort((a, b) => a.length - b.length);
  let current = tokenCandidates[0].slice();
  for (let i = 1; i < tokenCandidates.length; i += 1) {
    current = intersectSorted(current, tokenCandidates[i]);
    if (current.length === 0) {
      break;
    }
  }
  return current;
}

function scoreToken(entry, token) {
  const basename = entry.lower.slice(entry.basenameOffset);
  const path = entry.lower;
  const basenameOriginal = entry.path.slice(entry.basenameOriginalOffset);
  const needle = token.text;

  if (token.field === "path") {
    const pos = path.indexOf(needle);
    return pos >= 0 ? 24000 - pos * 8 : null;
  }

  if (basename === needle) {
    return 64000 - basename.length;
  }
  if (basename.startsWith(needle)) {
    return 56000 - basename.length;
  }
  const basenamePos = basename.indexOf(needle);
  if (basenamePos >= 0) {
    return 48000 - basenamePos * 16;
  }
  const pathPos = path.indexOf(needle);
  if (pathPos >= 0) {
    return 24000 - pathPos * 8;
  }

  const acronymScore = scoreAcronymMatch(basenameOriginal, needle);
  if (acronymScore !== null) {
    return acronymScore;
  }

  return scoreTypoBasenameMatch(basenameOriginal, needle);
}

function scoreEntry(entry, parsed) {
  let total = 0;
  for (const token of parsed.tokens) {
    const score = scoreToken(entry, token);
    if (score === null) {
      return null;
    }
    total += score;
  }

  const depthBonus = Math.max(0, 2000 - entry.depth * 64);
  const basenameBonus = Math.max(0, 1024 - (entry.lower.length - entry.basenameOffset));
  return total + depthBonus + basenameBonus;
}

function searchWithCandidates(index, parsed, candidates, limit) {
  const candidateIds = candidates || index.entries.map((_, id) => id);
  const scored = [];

  for (const id of candidateIds) {
    const entry = index.entries[id];
    const score = scoreEntry(entry, parsed);
    if (score !== null) {
      scored.push({ id, score });
    }
  }

  scored.sort((left, right) => {
    if (right.score !== left.score) {
      return right.score - left.score;
    }
    return index.entries[left.id].path.localeCompare(index.entries[right.id].path);
  });

  return {
    hits: scored.slice(0, limit).map((item) => ({
      path: index.entries[item.id].path,
      score: item.score,
    })),
    stats: {
      totalEntries: index.entries.length,
      candidateCount: candidateIds.length,
      totalMatches: scored.length,
    },
  };
}

function shouldFallbackToScan(entryCount, candidateCount) {
  return entryCount > 0 && candidateCount * 100 >= entryCount * 80;
}

function search(index, rawQuery, limit = 12) {
  const parsed = parseQuery(rawQuery);
  if (parsed.tokens.length === 0) {
    return {
      hits: [],
      stats: {
        totalEntries: index.entries.length,
        candidateCount: 0,
        totalMatches: 0,
      },
      mode: "indexed",
    };
  }

  if (parsed.tokens.every((token) => token.text.length <= 1)) {
    const result = searchWithCandidates(index, parsed, null, limit);
    return { ...result, mode: "scan" };
  }

  const candidates = candidatesForQuery(index, parsed);
  if (shouldFallbackToScan(index.entries.length, candidates.length)) {
    const result = searchWithCandidates(index, parsed, null, limit);
    return { ...result, mode: "scan" };
  }

  const result = searchWithCandidates(index, parsed, candidates, limit);
  return { ...result, mode: "indexed" };
}

const index = buildIndex(SAMPLE_PATHS);
const input = document.querySelector("#query-input");
const clearButton = document.querySelector("#clear-button");
const resultsNode = document.querySelector("#results");
const entryCountNode = document.querySelector("#entry-count");
const candidateCountNode = document.querySelector("#candidate-count");
const matchCountNode = document.querySelector("#match-count");
const modeNode = document.querySelector("#mode-label");
const elapsedNode = document.querySelector("#elapsed-ms");

entryCountNode.textContent = String(index.entries.length);

function render(query) {
  const start = performance.now();
  const result = search(index, query);
  const elapsed = performance.now() - start;

  candidateCountNode.textContent = String(result.stats.candidateCount);
  matchCountNode.textContent = String(result.stats.totalMatches);
  modeNode.textContent = result.mode;
  elapsedNode.textContent = elapsed.toFixed(3);

  if (result.hits.length === 0) {
    resultsNode.innerHTML = '<li class="empty-state">No matches in the sample corpus.</li>';
    return;
  }

  resultsNode.innerHTML = result.hits
    .map(
      (hit) => `
        <li class="result-item">
          <p class="result-title">${hit.path}</p>
          <div class="result-meta">score ${hit.score}</div>
        </li>
      `,
    )
    .join("");
}

input.addEventListener("input", () => {
  render(input.value);
});

clearButton.addEventListener("click", () => {
  input.value = "";
  input.focus();
  render("");
});

for (const button of document.querySelectorAll("[data-query]")) {
  button.addEventListener("click", () => {
    input.value = button.dataset.query || "";
    input.focus();
    render(input.value);
  });
}

render(input.value);
