use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use memchr::memmem::Finder;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentMatch {
    pub path: String,
    pub line_number: u64,
    pub column: usize,
    pub line_content: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentSearchStats {
    pub total_files: usize,
    pub indexed_files: usize,
    pub candidate_files: usize,
    pub searched_files: usize,
    pub files_with_matches: usize,
    pub returned_matches: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentSearchResult {
    pub matches: Vec<ContentMatch>,
    pub stats: ContentSearchStats,
}

#[derive(Debug, Clone)]
struct ContentFile {
    path: String,
    contents: Box<[u8]>,
    lower_contents: Box<[u8]>,
}

#[derive(Debug)]
pub struct ContentIndex {
    files: Vec<ContentFile>,
    total_files: usize,
    char_postings: Vec<Vec<u32>>,
    bigram_postings: HashMap<u16, Vec<u32>>,
    trigram_postings: HashMap<u32, Vec<u32>>,
}

impl ContentIndex {
    pub fn build(root: &Path, paths: Vec<String>) -> io::Result<Self> {
        let mut index = Self {
            files: Vec::new(),
            total_files: paths.len(),
            char_postings: vec![Vec::new(); 256],
            bigram_postings: HashMap::new(),
            trigram_postings: HashMap::new(),
        };

        for rel_path in paths {
            let abs_path = root.join(&rel_path);
            let metadata = match fs::metadata(&abs_path) {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };

            if metadata.len() > 10 * 1024 * 1024 {
                continue;
            }

            let bytes = match fs::read(&abs_path) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };

            if looks_binary(&bytes) {
                continue;
            }

            let id = index.files.len() as u32;
            let lowered = ascii_lower_bytes(&bytes);
            index.push_char_postings(&lowered, id);
            index.push_ngram_postings(&lowered, id);
            index.files.push(ContentFile {
                path: rel_path,
                contents: bytes.into_boxed_slice(),
                lower_contents: lowered.into_boxed_slice(),
            });
        }

        Ok(index)
    }

    pub fn indexed_files(&self) -> usize {
        self.files.len()
    }

    pub fn search_exact(&self, query: &str, limit: usize) -> ContentSearchResult {
        if query.is_empty() {
            return ContentSearchResult {
                matches: Vec::new(),
                stats: ContentSearchStats {
                    total_files: self.total_files,
                    indexed_files: self.files.len(),
                    ..ContentSearchStats::default()
                },
            };
        }

        let case_sensitive = query.bytes().any(|byte| byte.is_ascii_uppercase());
        let needle = query.as_bytes().to_vec();
        let lowered = ascii_lower_bytes(query.as_bytes());

        let mut candidate_ids = self.candidates_for_query(&lowered);
        if should_scan_all_candidates(lowered.len(), candidate_ids.len(), self.files.len()) {
            candidate_ids = (0..self.files.len() as u32).collect();
        }

        let candidate_files = candidate_ids.len();
        let mut searched_files = 0usize;
        let mut files_with_matches = 0usize;
        let mut matches = Vec::new();

        for file_id in candidate_ids {
            if limit > 0 && matches.len() >= limit {
                break;
            }

            let file = &self.files[file_id as usize];
            searched_files += 1;

            let positions = if case_sensitive {
                find_positions(&file.contents, &needle)
            } else {
                find_positions(&file.lower_contents, &lowered)
            };

            if positions.is_empty() {
                continue;
            }

            files_with_matches += 1;
            let line_starts = collect_line_starts(&file.contents);
            for position in positions {
                if limit > 0 && matches.len() >= limit {
                    break;
                }
                matches.push(build_match(
                    &file.path,
                    &file.contents,
                    &line_starts,
                    position,
                ));
            }
        }

        ContentSearchResult {
            stats: ContentSearchStats {
                total_files: self.total_files,
                indexed_files: self.files.len(),
                candidate_files,
                searched_files,
                files_with_matches,
                returned_matches: matches.len(),
            },
            matches,
        }
    }

    fn candidates_for_query(&self, lowered_query: &[u8]) -> Vec<u32> {
        match lowered_query.len() {
            0 => Vec::new(),
            1 => self.char_postings[lowered_query[0] as usize].clone(),
            2 => self
                .bigram_postings
                .get(&pack_bigram(lowered_query[0], lowered_query[1]))
                .cloned()
                .unwrap_or_default(),
            _ => {
                let mut keys = extract_trigrams(lowered_query);
                keys.sort_unstable();
                keys.dedup();
                if keys.is_empty() {
                    return Vec::new();
                }

                let mut candidate_sets = Vec::with_capacity(keys.len());
                for key in keys {
                    let Some(posting) = self.trigram_postings.get(&key) else {
                        return Vec::new();
                    };
                    candidate_sets.push(posting.clone());
                }

                candidate_sets.sort_by_key(Vec::len);
                let mut acc = candidate_sets.remove(0);
                for posting in candidate_sets {
                    acc = intersect_sorted(&acc, &posting);
                    if acc.is_empty() {
                        break;
                    }
                }
                acc
            }
        }
    }

    fn push_char_postings(&mut self, bytes: &[u8], id: u32) {
        let mut seen = [false; 256];
        for &byte in bytes {
            let idx = byte as usize;
            if seen[idx] {
                continue;
            }
            seen[idx] = true;
            self.char_postings[idx].push(id);
        }
    }

    fn push_ngram_postings(&mut self, bytes: &[u8], id: u32) {
        let mut bigrams = extract_bigrams(bytes);
        bigrams.sort_unstable();
        bigrams.dedup();
        for key in bigrams {
            self.bigram_postings.entry(key).or_default().push(id);
        }

        let mut trigrams = extract_trigrams(bytes);
        trigrams.sort_unstable();
        trigrams.dedup();
        for key in trigrams {
            self.trigram_postings.entry(key).or_default().push(id);
        }
    }
}

fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8_192).any(|&byte| byte == 0)
}

fn ascii_lower_bytes(bytes: &[u8]) -> Vec<u8> {
    bytes.iter().map(u8::to_ascii_lowercase).collect()
}

fn find_positions(bytes: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() {
        return Vec::new();
    }

    let finder = Finder::new(needle);
    finder.find_iter(bytes).collect()
}

fn collect_line_starts(bytes: &[u8]) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (idx, &byte) in bytes.iter().enumerate() {
        if byte == b'\n' && idx + 1 < bytes.len() {
            starts.push(idx + 1);
        }
    }
    starts
}

fn build_match(path: &str, bytes: &[u8], line_starts: &[usize], position: usize) -> ContentMatch {
    let line_index = match line_starts.binary_search(&position) {
        Ok(index) => index,
        Err(index) => index.saturating_sub(1),
    };
    let line_start = line_starts[line_index];
    let line_end = if line_index + 1 < line_starts.len() {
        line_starts[line_index + 1].saturating_sub(1)
    } else {
        bytes.len()
    };
    let line_bytes = &bytes[line_start..line_end];

    ContentMatch {
        path: path.to_string(),
        line_number: (line_index + 1) as u64,
        column: position.saturating_sub(line_start),
        line_content: String::from_utf8_lossy(line_bytes).into_owned(),
    }
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

fn should_scan_all_candidates(
    query_len: usize,
    candidate_count: usize,
    indexed_files: usize,
) -> bool {
    if indexed_files == 0 {
        return false;
    }

    if query_len <= 2 {
        return true;
    }

    candidate_count.saturating_mul(100) >= indexed_files.saturating_mul(85)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::ContentIndex;

    fn test_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("applefind-content-{unique}"))
    }

    #[test]
    fn finds_exact_matches_with_line_info() {
        let root = test_dir();
        fs::create_dir_all(root.join("src")).expect("create temp dir");
        fs::write(
            root.join("src/lib.rs"),
            "fn controller() {}\nlet user_authentication = true;\n",
        )
        .expect("write fixture");
        fs::write(root.join("README.md"), "controller\n").expect("write fixture");

        let index = ContentIndex::build(
            &root,
            vec!["src/lib.rs".to_string(), "README.md".to_string()],
        )
        .expect("build content index");

        let result = index.search_exact("user_authentication", 10);
        assert_eq!(result.stats.indexed_files, 2);
        assert_eq!(result.stats.files_with_matches, 1);
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].path, "src/lib.rs");
        assert_eq!(result.matches[0].line_number, 2);
        assert!(
            result.matches[0]
                .line_content
                .contains("user_authentication")
        );

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn lowercase_query_matches_uppercase_content() {
        let root = test_dir();
        fs::create_dir_all(root.join("src")).expect("create temp dir");
        fs::write(root.join("src/lib.rs"), "const QueryParser = 1;\n").expect("write fixture");

        let index = ContentIndex::build(&root, vec!["src/lib.rs".to_string()])
            .expect("build content index");
        let result = index.search_exact("queryparser", 10);
        assert_eq!(result.matches.len(), 1);

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }
}
