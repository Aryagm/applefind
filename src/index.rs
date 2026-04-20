use std::collections::HashMap;

use neo_frizbee::{Config as FuzzyConfig, Match as FuzzyMatch, match_list_parallel};
use rayon::prelude::*;

use crate::query::{ParsedQuery, QueryToken, SearchField, parse_query};

#[derive(Debug, Clone)]
struct PathEntry {
    path: String,
    lower: String,
    basename_offset: usize,
    basename_original_offset: usize,
    depth: u16,
}

impl PathEntry {
    fn new(path: String) -> Self {
        let lower = path.to_lowercase();
        let basename_offset = lower.rfind(['/', '\\']).map(|idx| idx + 1).unwrap_or(0);
        let basename_original_offset = path.rfind(['/', '\\']).map(|idx| idx + 1).unwrap_or(0);
        let depth = lower.bytes().filter(|&b| b == b'/' || b == b'\\').count() as u16;

        Self {
            path,
            lower,
            basename_offset,
            basename_original_offset,
            depth,
        }
    }

    fn basename(&self) -> &str {
        &self.lower[self.basename_offset..]
    }

    fn basename_original(&self) -> &str {
        &self.path[self.basename_original_offset..]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub path: String,
    pub score: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchStats {
    pub total_entries: usize,
    pub candidate_count: usize,
    pub scanned_entries: usize,
    pub total_matches: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    pub stats: SearchStats,
}

#[derive(Debug, Default)]
pub struct PathIndex {
    entries: Vec<PathEntry>,
    basename_chars: Vec<Vec<u32>>,
    basename_bigrams: HashMap<u16, Vec<u32>>,
    basename_trigrams: HashMap<u32, Vec<u32>>,
    basename_acronyms: HashMap<Box<str>, Vec<u32>>,
    path_chars: Vec<Vec<u32>>,
    path_bigrams: HashMap<u16, Vec<u32>>,
    path_trigrams: HashMap<u32, Vec<u32>>,
    path_components: HashMap<Box<str>, Vec<u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RankingMode {
    Exact,
    Fuzzy,
}

impl PathIndex {
    pub fn build(paths: Vec<String>) -> Self {
        let entries: Vec<PathEntry> = paths.into_iter().map(PathEntry::new).collect();
        let mut index = Self {
            entries,
            basename_chars: vec![Vec::new(); 256],
            path_chars: vec![Vec::new(); 256],
            ..Self::default()
        };

        for id in 0..index.entries.len() {
            let id = id as u32;
            let basename = index.entries[id as usize].basename().as_bytes().to_vec();
            let lower = index.entries[id as usize].lower.as_bytes().to_vec();
            let path_components = extract_components(index.entries[id as usize].lower.as_str());
            let basename_acronym = compute_acronym(strip_extension(
                index.entries[id as usize].basename_original(),
            ));
            index.push_char_postings(&basename, id, true);
            index.push_char_postings(&lower, id, false);
            index.push_ngram_postings(&basename, id, true);
            index.push_ngram_postings(&lower, id, false);
            index.push_component_postings(&path_components, id);
            index.push_acronym_posting(&basename_acronym, id);
        }

        index
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn search_scan(&self, query: &str, limit: usize) -> SearchResult {
        let parsed = parse_query(query);
        self.search_with_candidates(parsed, None, limit)
    }

    pub fn search_indexed(&self, query: &str, limit: usize) -> SearchResult {
        let parsed = parse_query(query);
        if should_bypass_index(&parsed) {
            return self.search_with_candidates(parsed, None, limit);
        }
        let mut candidates = self.candidates_for_query(&parsed);
        if should_use_fuzzy_ranking(&parsed)
            && should_expand_fuzzy_candidates(&parsed, candidates.len(), limit)
        {
            let fuzzy_candidates = self.fuzzy_candidates_for_query(&parsed);
            if should_accept_fuzzy_expansion(candidates.len(), fuzzy_candidates.len()) {
                candidates = union_sorted(&candidates, &fuzzy_candidates);
            }
        }
        if self.should_fallback_to_scan(&parsed, candidates.len()) {
            return self.search_with_candidates(parsed, None, limit);
        }
        self.search_with_candidates(parsed, Some(candidates), limit)
    }

    fn search_with_candidates(
        &self,
        parsed: ParsedQuery,
        candidates: Option<Vec<u32>>,
        limit: usize,
    ) -> SearchResult {
        if parsed.tokens.is_empty() {
            return SearchResult {
                hits: Vec::new(),
                stats: SearchStats {
                    total_entries: self.entries.len(),
                    ..SearchStats::default()
                },
            };
        }

        let candidate_ids = candidates.unwrap_or_else(|| (0..self.entries.len() as u32).collect());
        let candidate_count = candidate_ids.len();
        let mut scored = if choose_ranking_mode(&parsed, candidate_count) == RankingMode::Fuzzy {
            self.rank_fuzzy_candidates(&parsed, &candidate_ids)
        } else {
            self.rank_exact_candidates(&parsed, &candidate_ids)
        };

        let compare = |left: &(u32, u32), right: &(u32, u32)| {
            right.1.cmp(&left.1).then_with(|| {
                self.entries[left.0 as usize]
                    .path
                    .cmp(&self.entries[right.0 as usize].path)
            })
        };

        let total_matches = scored.len();
        if limit > 0 && scored.len() > limit {
            scored.select_nth_unstable_by(limit - 1, compare);
            scored.truncate(limit);
        }
        scored.sort_unstable_by(compare);

        let hits = if limit == 0 {
            Vec::new()
        } else {
            scored
                .into_iter()
                .map(|(id, score)| SearchHit {
                    path: self.entries[id as usize].path.clone(),
                    score,
                })
                .collect()
        };

        SearchResult {
            hits,
            stats: SearchStats {
                total_entries: self.entries.len(),
                candidate_count,
                scanned_entries: candidate_count,
                total_matches,
            },
        }
    }

    fn rank_exact_candidates(
        &self,
        parsed: &ParsedQuery,
        candidate_ids: &[u32],
    ) -> Vec<(u32, u32)> {
        if candidate_ids.len() >= 32_768 {
            candidate_ids
                .par_iter()
                .filter_map(|&id| {
                    let entry = &self.entries[id as usize];
                    score_entry(entry, parsed).map(|score| (id, score))
                })
                .collect()
        } else {
            let mut scored = Vec::with_capacity(candidate_ids.len().min(8_192));
            for &id in candidate_ids {
                let entry = &self.entries[id as usize];
                if let Some(score) = score_entry(entry, parsed) {
                    scored.push((id, score));
                }
            }
            scored
        }
    }

    fn rank_fuzzy_candidates(
        &self,
        parsed: &ParsedQuery,
        candidate_ids: &[u32],
    ) -> Vec<(u32, u32)> {
        let mut current: Vec<(u32, u32)> =
            candidate_ids.iter().copied().map(|id| (id, 0)).collect();
        let threads = fuzzy_threads(candidate_ids.len());

        for token in &parsed.tokens {
            let basename_haystacks: Vec<&str> = current
                .iter()
                .map(|(id, _)| self.entries[*id as usize].basename_original())
                .collect();
            let config = fuzzy_config_for_token(token.text.as_str());
            let should_match_path = should_match_path_fuzzy(token, current.len());
            let basename_matches = match_list_parallel(
                token.text.as_str(),
                basename_haystacks.as_slice(),
                &config,
                if current.len() >= 4_096 { threads } else { 1 },
            );

            let path_scores = if should_match_path {
                let path_haystacks: Vec<&str> = current
                    .iter()
                    .map(|(id, _)| self.entries[*id as usize].path.as_str())
                    .collect();
                let path_matches = match_list_parallel(
                    token.text.as_str(),
                    path_haystacks.as_slice(),
                    &config,
                    threads,
                );
                let mut scores = vec![None; current.len()];
                for matched in path_matches {
                    scores[matched.index as usize] = Some(matched);
                }
                Some(scores)
            } else {
                None
            };

            let mut basename_scores = vec![None; current.len()];
            for matched in basename_matches {
                basename_scores[matched.index as usize] = Some(matched);
            }

            let mut next = Vec::with_capacity(current.len().min(8_192));
            for (idx, (id, accumulated)) in current.into_iter().enumerate() {
                let entry = &self.entries[id as usize];
                if let Some(token_score) = score_fuzzy_token(
                    entry,
                    token,
                    path_scores.as_ref().and_then(|scores| scores[idx]),
                    basename_scores[idx],
                ) {
                    next.push((id, accumulated.saturating_add(token_score)));
                }
            }

            current = next;
            if current.is_empty() {
                break;
            }
        }

        current
            .into_iter()
            .map(|(id, score)| {
                let entry = &self.entries[id as usize];
                (id, score.saturating_add(entry_rank_bonus(entry)))
            })
            .collect()
    }

    fn push_char_postings(&mut self, bytes: &[u8], id: u32, basename: bool) {
        let mut seen = [false; 256];
        let target = if basename {
            &mut self.basename_chars
        } else {
            &mut self.path_chars
        };

        for &byte in bytes {
            let idx = byte as usize;
            if seen[idx] {
                continue;
            }
            seen[idx] = true;
            target[idx].push(id);
        }
    }

    fn push_ngram_postings(&mut self, bytes: &[u8], id: u32, basename: bool) {
        let target_bigrams = if basename {
            &mut self.basename_bigrams
        } else {
            &mut self.path_bigrams
        };
        let target_trigrams = if basename {
            &mut self.basename_trigrams
        } else {
            &mut self.path_trigrams
        };

        let mut bigrams = extract_bigrams(bytes);
        bigrams.sort_unstable();
        bigrams.dedup();
        for key in bigrams {
            target_bigrams.entry(key).or_default().push(id);
        }

        let mut trigrams = extract_trigrams(bytes);
        trigrams.sort_unstable();
        trigrams.dedup();
        for key in trigrams {
            target_trigrams.entry(key).or_default().push(id);
        }
    }

    fn push_component_postings(&mut self, components: &[Box<str>], id: u32) {
        for component in components {
            self.path_components
                .entry(component.clone())
                .or_default()
                .push(id);
        }
    }

    fn push_acronym_posting(&mut self, acronym: &str, id: u32) {
        if acronym.len() < 2 {
            return;
        }
        self.basename_acronyms
            .entry(acronym.into())
            .or_default()
            .push(id);
    }

    fn candidates_for_query(&self, parsed: &ParsedQuery) -> Vec<u32> {
        let mut token_candidates = Vec::with_capacity(parsed.tokens.len());

        for token in &parsed.tokens {
            let candidates = self.candidates_for_token(token);
            if candidates.is_empty() {
                return Vec::new();
            }
            token_candidates.push(candidates);
        }

        token_candidates.sort_by_key(Vec::len);

        let mut current = token_candidates.remove(0);
        for next in token_candidates {
            current = intersect_sorted(&current, &next);
            if current.is_empty() {
                break;
            }
        }

        current
    }

    fn fuzzy_candidates_for_query(&self, parsed: &ParsedQuery) -> Vec<u32> {
        let mut token_candidates = Vec::with_capacity(parsed.tokens.len());

        for token in &parsed.tokens {
            let candidates = self.fuzzy_candidates_for_token(token);
            if candidates.is_empty() {
                return Vec::new();
            }
            token_candidates.push(candidates);
        }

        token_candidates.sort_by_key(Vec::len);

        let mut current = token_candidates.remove(0);
        for next in token_candidates {
            current = intersect_sorted(&current, &next);
            if current.is_empty() {
                break;
            }
        }

        current
    }

    fn candidates_for_token(&self, token: &QueryToken) -> Vec<u32> {
        match token.field {
            SearchField::Path => self.candidates_for_field(token.text.as_bytes(), false),
            SearchField::BasenameOrPath => {
                let basename = self.candidates_for_field(token.text.as_bytes(), true);
                let path = self.candidates_for_field(token.text.as_bytes(), false);
                let exact = union_sorted(&basename, &path);
                let acronym = if token.text.len() >= 2 {
                    self.basename_acronyms
                        .get(token.text.as_str())
                        .cloned()
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                let combined = union_sorted(&exact, &acronym);
                if !combined.is_empty() || token.text.len() < 4 {
                    combined
                } else {
                    self.approximate_candidates(token.text.as_bytes(), true)
                }
            }
        }
    }

    fn fuzzy_candidates_for_token(&self, token: &QueryToken) -> Vec<u32> {
        match token.field {
            SearchField::Path => self.candidates_for_field(token.text.as_bytes(), false),
            SearchField::BasenameOrPath => {
                let bytes = token.text.as_bytes();
                let exact = union_sorted(
                    &self.candidates_for_field(bytes, true),
                    &self.candidates_for_field(bytes, false),
                );
                let acronym = if token.text.len() >= 2 {
                    self.basename_acronyms
                        .get(token.text.as_str())
                        .cloned()
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                let basename_chars = approximate_char_candidates(bytes, &self.basename_chars);
                let path_chars = approximate_char_candidates(bytes, &self.path_chars);
                let basename_bigrams = if token.text.len() >= 3 {
                    self.approximate_candidates(bytes, true)
                } else {
                    Vec::new()
                };
                let path_bigrams = if token.text.len() >= 3 {
                    self.approximate_candidates(bytes, false)
                } else {
                    Vec::new()
                };

                let combined = union_sorted(
                    &union_sorted(&exact, &acronym),
                    &union_sorted(
                        &union_sorted(&basename_chars, &path_chars),
                        &union_sorted(&basename_bigrams, &path_bigrams),
                    ),
                );

                if combined.is_empty() {
                    self.approximate_candidates(bytes, false)
                } else {
                    combined
                }
            }
        }
    }

    fn candidates_for_field(&self, bytes: &[u8], basename: bool) -> Vec<u32> {
        if bytes.is_empty() {
            return Vec::new();
        }

        let components = if !basename && bytes.iter().any(|&b| b == b'/' || b == b'\\') {
            extract_components_from_bytes(bytes)
        } else {
            Vec::new()
        };

        let chars = if basename {
            &self.basename_chars
        } else {
            &self.path_chars
        };
        let bigrams = if basename {
            &self.basename_bigrams
        } else {
            &self.path_bigrams
        };
        let trigrams = if basename {
            &self.basename_trigrams
        } else {
            &self.path_trigrams
        };

        match bytes.len() {
            0 => Vec::new(),
            1 => chars[bytes[0] as usize].clone(),
            2 => bigrams
                .get(&pack_bigram(bytes[0], bytes[1]))
                .cloned()
                .unwrap_or_default(),
            _ => {
                let mut candidate_sets: Vec<Vec<u32>> = Vec::new();

                if !basename && !components.is_empty() {
                    for component in &components {
                        let posting = match self.path_components.get(component.as_ref()) {
                            Some(posting) => posting.clone(),
                            None => return Vec::new(),
                        };
                        candidate_sets.push(posting);
                    }
                }

                let mut keys = extract_trigrams(bytes);
                keys.sort_unstable();
                keys.dedup();
                if keys.is_empty() && candidate_sets.is_empty() {
                    return Vec::new();
                }

                for key in keys {
                    let posting = match trigrams.get(&key) {
                        Some(posting) => posting.clone(),
                        None => return Vec::new(),
                    };
                    candidate_sets.push(posting);
                }

                candidate_sets.sort_by_key(Vec::len);
                let mut acc = candidate_sets.remove(0);
                for posting in &candidate_sets {
                    acc = intersect_sorted(&acc, posting);
                    if acc.is_empty() {
                        break;
                    }
                }
                acc
            }
        }
    }

    fn approximate_candidates(&self, bytes: &[u8], basename: bool) -> Vec<u32> {
        let mut keys = extract_bigrams(bytes);
        keys.sort_unstable();
        keys.dedup();
        if keys.is_empty() {
            return Vec::new();
        }

        let postings = if basename {
            &self.basename_bigrams
        } else {
            &self.path_bigrams
        };

        let required = required_bigram_overlap(keys.len());
        let mut counts: HashMap<u32, u8> = HashMap::new();
        for key in keys {
            if let Some(posting) = postings.get(&key) {
                for &id in posting {
                    let count = counts.entry(id).or_insert(0);
                    *count = count.saturating_add(1);
                }
            }
        }

        let mut out: Vec<u32> = counts
            .into_iter()
            .filter_map(|(id, count)| (count as usize >= required).then_some(id))
            .collect();
        out.sort_unstable();
        out
    }

    fn should_fallback_to_scan(&self, parsed: &ParsedQuery, candidate_count: usize) -> bool {
        if self.entries.is_empty() {
            return false;
        }

        let fallback_percent = fallback_scan_percent(parsed);
        candidate_count.saturating_mul(100) >= self.entries.len().saturating_mul(fallback_percent)
    }
}

fn should_use_fuzzy_ranking(parsed: &ParsedQuery) -> bool {
    !parsed.tokens.is_empty()
        && parsed
            .tokens
            .iter()
            .all(|token| token.field == SearchField::BasenameOrPath && token.text.len() >= 2)
}

fn should_bypass_index(parsed: &ParsedQuery) -> bool {
    parsed.tokens.iter().all(|token| token.text.len() <= 1)
        || (parsed.tokens.len() == 1
            && parsed.tokens[0].field == SearchField::BasenameOrPath
            && parsed.tokens[0].text.len() <= 2)
}

fn choose_ranking_mode(parsed: &ParsedQuery, candidate_count: usize) -> RankingMode {
    if should_use_short_query_exact_ranking(parsed, candidate_count) {
        RankingMode::Exact
    } else if should_use_fuzzy_ranking(parsed) {
        RankingMode::Fuzzy
    } else {
        RankingMode::Exact
    }
}

fn should_use_short_query_exact_ranking(parsed: &ParsedQuery, candidate_count: usize) -> bool {
    candidate_count > 12_288
        && parsed.tokens.len() == 1
        && parsed.tokens[0].field == SearchField::BasenameOrPath
        && parsed.tokens[0].text.len() <= 4
}

fn fallback_scan_percent(parsed: &ParsedQuery) -> usize {
    if parsed.tokens.len() == 1
        && parsed.tokens[0].field == SearchField::BasenameOrPath
        && parsed.tokens[0].text.len() <= 2
    {
        60
    } else {
        80
    }
}

fn should_expand_fuzzy_candidates(
    parsed: &ParsedQuery,
    candidate_count: usize,
    _limit: usize,
) -> bool {
    candidate_count < 32 && parsed.tokens.iter().all(|token| token.text.len() <= 12)
}

fn should_accept_fuzzy_expansion(_strict_count: usize, fuzzy_count: usize) -> bool {
    fuzzy_count <= 4_096
}

fn fuzzy_config_for_token(needle: &str) -> FuzzyConfig {
    let max_typos = (needle.len() as u16 / 4)
        .clamp(2, 6)
        .min(needle.len() as u16);
    FuzzyConfig {
        max_typos: Some(max_typos),
        sort: false,
        ..FuzzyConfig::default()
    }
}

fn fuzzy_threads(candidate_count: usize) -> usize {
    let available = std::thread::available_parallelism()
        .map(|parallelism| parallelism.get())
        .unwrap_or(4);
    if candidate_count < 4_096 {
        1
    } else if candidate_count < 65_536 {
        available.min(4)
    } else {
        available.min(8)
    }
}

fn should_match_path_fuzzy(token: &QueryToken, candidate_count: usize) -> bool {
    token.text.len() >= 4 && candidate_count <= 4_096
}

fn score_entry(entry: &PathEntry, parsed: &ParsedQuery) -> Option<u32> {
    let mut total = 0u32;
    for token in &parsed.tokens {
        total = total.saturating_add(score_token(entry, token)?);
    }

    Some(total.saturating_add(entry_rank_bonus(entry)))
}

fn entry_rank_bonus(entry: &PathEntry) -> u32 {
    let depth_bonus = 2_000u32.saturating_sub((entry.depth as u32) * 64);
    let basename_len_bonus = 1_024u32.saturating_sub(entry.basename().len() as u32);
    depth_bonus.saturating_add(basename_len_bonus)
}

fn score_token(entry: &PathEntry, token: &QueryToken) -> Option<u32> {
    let basename = entry.basename();
    let path = entry.lower.as_str();
    let basename_original = entry.basename_original();
    let needle = token.text.as_str();

    match token.field {
        SearchField::Path => path
            .find(needle)
            .map(|pos| 24_000u32.saturating_sub((pos as u32) * 8)),
        SearchField::BasenameOrPath => {
            if basename == needle {
                Some(64_000u32.saturating_sub(basename.len() as u32))
            } else if basename.starts_with(needle) {
                Some(56_000u32.saturating_sub(basename.len() as u32))
            } else if let Some(pos) = basename.find(needle) {
                Some(48_000u32.saturating_sub((pos as u32) * 16))
            } else if let Some(pos) = path.find(needle) {
                Some(24_000u32.saturating_sub((pos as u32) * 8))
            } else if let Some(score) = score_acronym_match(basename_original, needle) {
                Some(score)
            } else {
                score_typo_basename_match(basename_original, needle)
            }
        }
    }
}

fn score_fuzzy_token(
    entry: &PathEntry,
    token: &QueryToken,
    path_match: Option<FuzzyMatch>,
    basename_match: Option<FuzzyMatch>,
) -> Option<u32> {
    let needle = token.text.as_str();
    let basename = entry.basename();
    let path = entry.lower.as_str();

    let mut matched = false;
    let mut score = 0u32;

    if let Some(matched_path) = path_match {
        matched = true;
        score = score.max((matched_path.score as u32).saturating_mul(128));
        if matched_path.exact {
            score = score.saturating_add(2_048);
        }
    }

    if let Some(matched_basename) = basename_match {
        matched = true;
        let basename_score = (matched_basename.score as u32)
            .saturating_mul(144)
            .saturating_add(1_536);
        score = score.max(basename_score);
        if matched_basename.exact {
            score = score.saturating_add(3_072);
        }
    }

    if basename == needle {
        matched = true;
        score = score
            .saturating_add(16_384)
            .saturating_sub(basename.len() as u32);
    } else if basename.starts_with(needle) {
        matched = true;
        score = score
            .saturating_add(12_288)
            .saturating_sub(basename.len() as u32);
    } else if let Some(pos) = basename.find(needle) {
        matched = true;
        score = score.saturating_add(8_192u32.saturating_sub((pos as u32) * 32));
    } else if let Some(pos) = path.find(needle) {
        matched = true;
        score = score.saturating_add(2_048u32.saturating_sub((pos as u32) * 8));
    }

    if let Some(acronym_score) = score_acronym_match(entry.basename_original(), needle) {
        matched = true;
        score = score.max(acronym_score.saturating_mul(2));
    }

    if let Some(typo_score) = score_typo_basename_match(entry.basename_original(), needle) {
        matched = true;
        score = score.max(typo_score);
    }

    matched.then_some(score)
}

fn pack_bigram(a: u8, b: u8) -> u16 {
    ((a as u16) << 8) | (b as u16)
}

fn pack_trigram(a: u8, b: u8, c: u8) -> u32 {
    ((a as u32) << 16) | ((b as u32) << 8) | (c as u32)
}

fn extract_bigrams(bytes: &[u8]) -> Vec<u16> {
    bytes
        .windows(2)
        .map(|window| pack_bigram(window[0], window[1]))
        .collect()
}

fn extract_trigrams(bytes: &[u8]) -> Vec<u32> {
    bytes
        .windows(3)
        .map(|window| pack_trigram(window[0], window[1], window[2]))
        .collect()
}

fn required_bigram_overlap(total: usize) -> usize {
    total.saturating_sub(2).max(2).min(total)
}

fn required_char_overlap(total: usize) -> usize {
    total.saturating_sub(2).max(2).min(total)
}

fn approximate_char_candidates(bytes: &[u8], postings: &[Vec<u32>]) -> Vec<u32> {
    let mut unique = bytes.to_vec();
    unique.sort_unstable();
    unique.dedup();
    if unique.is_empty() {
        return Vec::new();
    }

    let required = required_char_overlap(unique.len());
    let mut counts: HashMap<u32, u8> = HashMap::new();
    for byte in unique {
        for &id in &postings[byte as usize] {
            let count = counts.entry(id).or_insert(0);
            *count = count.saturating_add(1);
        }
    }

    let mut out: Vec<u32> = counts
        .into_iter()
        .filter_map(|(id, count)| (count as usize >= required).then_some(id))
        .collect();
    out.sort_unstable();
    out
}

fn extract_components(path: &str) -> Vec<Box<str>> {
    let mut components: Vec<Box<str>> = path
        .split(['/', '\\'])
        .filter(|component| !component.is_empty())
        .map(|component| component.into())
        .collect();
    components.sort_unstable();
    components.dedup();
    components
}

fn extract_components_from_bytes(bytes: &[u8]) -> Vec<Box<str>> {
    let text = String::from_utf8_lossy(bytes);
    extract_components(&text)
}

fn strip_extension(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => name,
    }
}

fn compute_acronym(text: &str) -> String {
    let mut out = String::new();
    for term in extract_terms(text) {
        if let Some(ch) = term.chars().next() {
            out.push(ch);
        }
    }
    out
}

fn extract_terms(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut prev_is_lower = false;

    for ch in text.chars() {
        if !ch.is_ascii_alphanumeric() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            prev_is_lower = false;
            continue;
        }

        let is_upper = ch.is_ascii_uppercase();
        if is_upper && prev_is_lower && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }

        current.push(ch.to_ascii_lowercase());
        prev_is_lower = ch.is_ascii_lowercase();
    }

    if !current.is_empty() {
        out.push(current);
    }

    out
}

fn score_acronym_match(basename_original: &str, needle: &str) -> Option<u32> {
    if needle.len() < 2 {
        return None;
    }

    let acronym = compute_acronym(strip_extension(basename_original));
    if acronym == needle {
        Some(40_000u32.saturating_sub(acronym.len() as u32 * 8))
    } else if acronym.starts_with(needle) {
        Some(32_000u32.saturating_sub(acronym.len() as u32 * 8))
    } else {
        None
    }
}

fn score_typo_basename_match(basename_original: &str, needle: &str) -> Option<u32> {
    if needle.len() < 4 {
        return None;
    }

    let stem = strip_extension(basename_original);
    let mut best = None;
    for term in extract_terms(stem) {
        if term == needle {
            return Some(48_000u32.saturating_sub(term.len() as u32 * 16));
        }

        let len_diff = term.len().abs_diff(needle.len());
        if len_diff > 2 {
            if let Some(score) = fuzzy_subsequence_score(&term, needle) {
                best = Some(best.map_or(score, |current: u32| current.max(score)));
            }
            continue;
        }

        if let Some(distance) = bounded_edit_distance(term.as_bytes(), needle.as_bytes(), 2) {
            let score = match distance {
                0 => 48_000u32.saturating_sub(term.len() as u32 * 16),
                1 => 20_000u32.saturating_sub(term.len() as u32 * 8),
                2 => 14_000u32.saturating_sub(term.len() as u32 * 8),
                _ => continue,
            };
            best = Some(best.map_or(score, |current: u32| current.max(score)));
        } else if let Some(score) = fuzzy_subsequence_score(&term, needle) {
            best = Some(best.map_or(score, |current: u32| current.max(score)));
        }
    }
    best
}

fn bounded_edit_distance(left: &[u8], right: &[u8], max_distance: usize) -> Option<usize> {
    if left.len().abs_diff(right.len()) > max_distance {
        return None;
    }

    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0usize; right.len() + 1];

    for (i, &lb) in left.iter().enumerate() {
        current[0] = i + 1;
        let mut row_min = current[0];

        for (j, &rb) in right.iter().enumerate() {
            let cost = usize::from(lb != rb);
            let delete = previous[j + 1] + 1;
            let insert = current[j] + 1;
            let replace = previous[j] + cost;
            let value = delete.min(insert).min(replace);
            current[j + 1] = value;
            row_min = row_min.min(value);
        }

        if row_min > max_distance {
            return None;
        }

        std::mem::swap(&mut previous, &mut current);
    }

    let distance = previous[right.len()];
    (distance <= max_distance).then_some(distance)
}

fn fuzzy_subsequence_score(haystack: &str, needle: &str) -> Option<u32> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }

    let haystack = haystack.as_bytes();
    let needle = needle.as_bytes();
    let mut needle_idx = 0usize;
    let mut last_match = None;
    let mut gaps = 0usize;
    let mut current_streak = 0usize;
    let mut best_streak = 0usize;

    for (idx, &hb) in haystack.iter().enumerate() {
        if needle_idx == needle.len() {
            break;
        }

        if hb.eq_ignore_ascii_case(&needle[needle_idx]) {
            if let Some(prev) = last_match {
                if idx == prev + 1 {
                    current_streak += 1;
                } else {
                    gaps += idx - prev - 1;
                    current_streak = 1;
                }
            } else {
                current_streak = 1;
            }

            best_streak = best_streak.max(current_streak);
            last_match = Some(idx);
            needle_idx += 1;
        }
    }

    if needle_idx != needle.len() {
        return None;
    }

    let score = 12_000u32
        .saturating_add((needle.len() as u32) * 700)
        .saturating_add((best_streak as u32) * 350)
        .saturating_sub((gaps as u32) * 40)
        .saturating_sub((haystack.len().saturating_sub(needle.len()) as u32) * 30);

    Some(score)
}

fn intersect_sorted(left: &[u32], right: &[u32]) -> Vec<u32> {
    let mut i = 0usize;
    let mut j = 0usize;
    let mut out = Vec::with_capacity(left.len().min(right.len()));

    while i < left.len() && j < right.len() {
        match left[i].cmp(&right[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                out.push(left[i]);
                i += 1;
                j += 1;
            }
        }
    }

    out
}

fn union_sorted(left: &[u32], right: &[u32]) -> Vec<u32> {
    let mut i = 0usize;
    let mut j = 0usize;
    let mut out = Vec::with_capacity(left.len() + right.len());

    while i < left.len() && j < right.len() {
        match left[i].cmp(&right[j]) {
            std::cmp::Ordering::Less => {
                out.push(left[i]);
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                out.push(right[j]);
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                out.push(left[i]);
                i += 1;
                j += 1;
            }
        }
    }

    out.extend_from_slice(&left[i..]);
    out.extend_from_slice(&right[j..]);
    out
}

#[cfg(test)]
mod tests {
    use super::{PathIndex, should_bypass_index, should_expand_fuzzy_candidates};
    use crate::query::parse_query;

    fn sample_index() -> PathIndex {
        PathIndex::build(vec![
            "src/auth/user_controller.rs".to_string(),
            "src/auth/session_manager.rs".to_string(),
            "src/chat/controller_view.swift".to_string(),
            "docs/controller-notes.md".to_string(),
            "pkg/gateway/client.go".to_string(),
            "src/lib.rs".to_string(),
            "content/browser/RenderFrameHostImpl.cc".to_string(),
        ])
    }

    #[test]
    fn indexed_and_scan_match_for_basename_query() {
        let index = sample_index();
        let scan = index.search_scan("controller", 10);
        let indexed = index.search_indexed("controller", 10);
        assert_eq!(scan.hits, indexed.hits);
        assert_eq!(scan.stats.total_matches, indexed.stats.total_matches);
    }

    #[test]
    fn indexed_and_scan_match_for_multi_token_query() {
        let index = sample_index();
        let scan = index.search_scan("auth manager", 10);
        let indexed = index.search_indexed("auth manager", 10);
        assert_eq!(scan.hits, indexed.hits);
        assert_eq!(scan.stats.total_matches, indexed.stats.total_matches);
    }

    #[test]
    fn indexed_and_scan_match_for_path_query() {
        let index = sample_index();
        let scan = index.search_scan("src/auth", 10);
        let indexed = index.search_indexed("src/auth", 10);
        assert_eq!(scan.hits, indexed.hits);
        assert_eq!(scan.stats.total_matches, indexed.stats.total_matches);
    }

    #[test]
    fn indexed_and_scan_match_for_typo_query() {
        let index = sample_index();
        let scan = index.search_scan("contrlr", 10);
        let indexed = index.search_indexed("contrlr", 10);
        assert_eq!(scan.hits, indexed.hits);
        assert_eq!(scan.stats.total_matches, indexed.stats.total_matches);
        assert!(!indexed.hits.is_empty());
    }

    #[test]
    fn indexed_and_scan_match_for_acronym_query() {
        let index = sample_index();
        let scan = index.search_scan("rfhi", 10);
        let indexed = index.search_indexed("rfhi", 10);
        assert_eq!(scan.hits, indexed.hits);
        assert_eq!(scan.stats.total_matches, indexed.stats.total_matches);
        assert!(!indexed.hits.is_empty());
    }

    #[test]
    fn bypasses_index_for_single_two_character_query() {
        let parsed = parse_query("st");
        assert!(should_bypass_index(&parsed));
    }

    #[test]
    fn skips_fuzzy_expansion_for_long_selective_query() {
        let parsed = parse_query("user_authentication");
        assert!(!should_expand_fuzzy_candidates(&parsed, 2, 20));
    }

    #[test]
    fn keeps_fuzzy_expansion_for_normal_basename_query() {
        let parsed = parse_query("controller");
        assert!(should_expand_fuzzy_candidates(&parsed, 2, 20));
    }
}
