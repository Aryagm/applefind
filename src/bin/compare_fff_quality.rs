use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::process;

use applefind::PathIndex;
use applefind::dataset::{collect_paths, default_bench_queries, normalize_root};
use fff_search::{
    FFFMode, FilePicker, FilePickerOptions, FuzzySearchOptions, PaginationArgs, QueryParser,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Plain,
    Markdown,
    Tsv,
}

#[derive(Debug)]
struct Config {
    root: PathBuf,
    limit: usize,
    max_threads: usize,
    example_count: usize,
    format: OutputFormat,
    queries: Vec<String>,
}

#[derive(Debug)]
struct QueryMetrics {
    query: String,
    apple_hits: usize,
    fff_hits: usize,
    apple_candidates: usize,
    apple_candidate_pct: f64,
    top1_same: bool,
    overlap_at_5: usize,
    overlap_at_10: usize,
    overlap_returned: usize,
    apple_only_examples: Vec<String>,
    fff_only_examples: Vec<String>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = parse_args(env::args().skip(1).collect())?;

    let paths = collect_paths(&config.root).map_err(|err| err.to_string())?;
    let index = PathIndex::build(paths);

    let mut picker = FilePicker::new(FilePickerOptions {
        base_path: config.root.to_string_lossy().into_owned(),
        enable_mmap_cache: false,
        enable_content_indexing: false,
        mode: FFFMode::Neovim,
        watch: false,
        ..FilePickerOptions::default()
    })
    .map_err(|err| err.to_string())?;
    picker.collect_files().map_err(|err| err.to_string())?;

    let queries = if config.queries.is_empty() {
        default_bench_queries()
            .into_iter()
            .filter(|query| *query != "a")
            .map(str::to_string)
            .collect()
    } else {
        config.queries.clone()
    };

    let metrics: Vec<QueryMetrics> = queries
        .iter()
        .map(|query| compare_query(query, &config, &index, &picker))
        .collect();

    match config.format {
        OutputFormat::Plain => {
            print_plain(&config, index.len(), picker.get_files().len(), &metrics)
        }
        OutputFormat::Markdown => {
            print_markdown(&config, index.len(), picker.get_files().len(), &metrics)
        }
        OutputFormat::Tsv => print_tsv(&metrics),
    }

    Ok(())
}

fn parse_args(args: Vec<String>) -> Result<Config, String> {
    let mut root = None;
    let mut limit = 100usize;
    let mut max_threads = 4usize;
    let mut example_count = 3usize;
    let mut format = OutputFormat::Plain;
    let mut queries = Vec::new();

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--root requires a value".to_string())?;
                root = Some(normalize_root(value).map_err(|err| err.to_string())?);
                i += 2;
            }
            "--limit" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--limit requires a value".to_string())?;
                limit = value
                    .parse::<usize>()
                    .map_err(|_| "invalid limit".to_string())?;
                i += 2;
            }
            "--max-threads" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--max-threads requires a value".to_string())?;
                max_threads = value
                    .parse::<usize>()
                    .map_err(|_| "invalid max thread count".to_string())?;
                i += 2;
            }
            "--example-count" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--example-count requires a value".to_string())?;
                example_count = value
                    .parse::<usize>()
                    .map_err(|_| "invalid example count".to_string())?;
                i += 2;
            }
            "--format" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--format requires a value".to_string())?;
                format = parse_format(value)?;
                i += 2;
            }
            "--query" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--query requires a value".to_string())?;
                queries.push(value.clone());
                i += 2;
            }
            "-h" | "--help" | "help" => {
                print_usage();
                process::exit(0);
            }
            other => {
                return Err(format!("unexpected argument: {other}"));
            }
        }
    }

    let root = root.ok_or_else(|| "--root is required".to_string())?;
    Ok(Config {
        root,
        limit,
        max_threads,
        example_count,
        format,
        queries,
    })
}

fn parse_format(value: &str) -> Result<OutputFormat, String> {
    match value {
        "plain" => Ok(OutputFormat::Plain),
        "markdown" => Ok(OutputFormat::Markdown),
        "tsv" => Ok(OutputFormat::Tsv),
        _ => Err("invalid format; expected plain, markdown, or tsv".to_string()),
    }
}

fn print_usage() {
    println!("Usage:");
    println!(
        "  cargo run --release --features compare-fff --bin compare-fff-quality -- --root PATH [options]"
    );
    println!();
    println!("Options:");
    println!("  --root PATH           Corpus root to compare");
    println!("  --limit N             Number of top results to compare (default: 100)");
    println!("  --max-threads N       Thread count for fff search (default: 4)");
    println!("  --example-count N     Mismatch examples to print per side (default: 3)");
    println!("  --format plain|markdown|tsv");
    println!("  --query TEXT          Repeat to override default benchmark queries");
}

fn compare_query(
    query: &str,
    config: &Config,
    index: &PathIndex,
    picker: &FilePicker,
) -> QueryMetrics {
    let apple = index.search_indexed(query, config.limit);
    let parsed = QueryParser::default().parse(query);
    let fff = picker.fuzzy_search(
        &parsed,
        None,
        FuzzySearchOptions {
            max_threads: config.max_threads,
            pagination: PaginationArgs {
                offset: 0,
                limit: config.limit,
            },
            ..FuzzySearchOptions::default()
        },
    );

    let apple_paths: Vec<String> = apple.hits.iter().map(|hit| hit.path.clone()).collect();
    let fff_paths: Vec<String> = fff
        .items
        .iter()
        .map(|item| item.relative_path(picker))
        .collect();

    QueryMetrics {
        query: query.to_string(),
        apple_hits: apple.stats.total_matches,
        fff_hits: fff.total_matched,
        apple_candidates: apple.stats.candidate_count,
        apple_candidate_pct: percentage(apple.stats.candidate_count, apple.stats.total_entries),
        top1_same: apple_paths.first() == fff_paths.first(),
        overlap_at_5: overlap_count(&apple_paths, &fff_paths, 5),
        overlap_at_10: overlap_count(&apple_paths, &fff_paths, 10),
        overlap_returned: overlap_count(&apple_paths, &fff_paths, config.limit),
        apple_only_examples: unique_examples(&apple_paths, &fff_paths, config.example_count),
        fff_only_examples: unique_examples(&fff_paths, &apple_paths, config.example_count),
    }
}

fn percentage(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        (part as f64 / total as f64) * 100.0
    }
}

fn overlap_count(left: &[String], right: &[String], limit: usize) -> usize {
    let left_set: HashSet<&str> = left.iter().take(limit).map(String::as_str).collect();
    let right_set: HashSet<&str> = right.iter().take(limit).map(String::as_str).collect();
    left_set.intersection(&right_set).count()
}

fn unique_examples(primary: &[String], other: &[String], limit: usize) -> Vec<String> {
    let other_set: HashSet<&str> = other.iter().map(String::as_str).collect();
    primary
        .iter()
        .filter(|path| !other_set.contains(path.as_str()))
        .take(limit)
        .cloned()
        .collect()
}

fn print_plain(
    config: &Config,
    apple_entries: usize,
    fff_entries: usize,
    metrics: &[QueryMetrics],
) {
    println!("applefind entries : {apple_entries}");
    println!("fff entries       : {fff_entries}");
    println!("result limit      : {}", config.limit);
    println!(
        "{:<18} {:>12} {:>10} {:>12} {:>8} {:>6} {:>7} {:>8} {:>6}",
        "query", "apple hits", "fff hits", "candidates", "cand%", "ov@5", "ov@10", "ov@ret", "top1"
    );

    for metric in metrics {
        println!(
            "{:<18} {:>12} {:>10} {:>12} {:>7.2}% {:>6} {:>7} {:>8} {:>6}",
            metric.query,
            metric.apple_hits,
            metric.fff_hits,
            metric.apple_candidates,
            metric.apple_candidate_pct,
            metric.overlap_at_5,
            metric.overlap_at_10,
            metric.overlap_returned,
            if metric.top1_same { "yes" } else { "no" }
        );
    }

    for metric in metrics {
        if metric.apple_hits == metric.fff_hits
            && metric.top1_same
            && metric.apple_only_examples.is_empty()
            && metric.fff_only_examples.is_empty()
        {
            continue;
        }

        println!();
        println!("query: {}", metric.query);
        println!(
            "  apple-only top results: {}",
            join_examples(&metric.apple_only_examples)
        );
        println!(
            "  fff-only top results  : {}",
            join_examples(&metric.fff_only_examples)
        );
    }
}

fn print_markdown(
    config: &Config,
    apple_entries: usize,
    fff_entries: usize,
    metrics: &[QueryMetrics],
) {
    println!(
        "Compared `applefind` and `fff` on `{}` with result limit `{}`.",
        config.root.display(),
        config.limit
    );
    println!();
    println!("- `applefind` entries: `{apple_entries}`");
    println!("- `fff` entries: `{fff_entries}`");
    println!();
    println!(
        "| query | apple hits | fff hits | apple candidates | candidate pct | overlap@5 | overlap@10 | overlap@returned | top1 same |"
    );
    println!("|---|---:|---:|---:|---:|---:|---:|---:|---:|");
    for metric in metrics {
        println!(
            "| `{}` | {} | {} | {} | {:.2}% | {} | {} | {} | {} |",
            metric.query,
            metric.apple_hits,
            metric.fff_hits,
            metric.apple_candidates,
            metric.apple_candidate_pct,
            metric.overlap_at_5,
            metric.overlap_at_10,
            metric.overlap_returned,
            if metric.top1_same { "yes" } else { "no" }
        );
    }

    let mut printed_details = false;
    for metric in metrics {
        if metric.apple_only_examples.is_empty() && metric.fff_only_examples.is_empty() {
            continue;
        }

        if !printed_details {
            println!();
            println!("## Mismatch Examples");
            printed_details = true;
        }

        println!();
        println!("### `{}`", metric.query);
        println!(
            "- apple-only: {}",
            join_examples(&metric.apple_only_examples)
        );
        println!("- fff-only: {}", join_examples(&metric.fff_only_examples));
    }
}

fn print_tsv(metrics: &[QueryMetrics]) {
    println!(
        "query\tapple_hits\tfff_hits\tapple_candidates\tapple_candidate_pct\ttop1_same\toverlap_at_5\toverlap_at_10\toverlap_returned\tapple_only_examples\tfff_only_examples"
    );
    for metric in metrics {
        println!(
            "{}\t{}\t{}\t{}\t{:.2}\t{}\t{}\t{}\t{}\t{}\t{}",
            metric.query,
            metric.apple_hits,
            metric.fff_hits,
            metric.apple_candidates,
            metric.apple_candidate_pct,
            metric.top1_same,
            metric.overlap_at_5,
            metric.overlap_at_10,
            metric.overlap_returned,
            join_examples(&metric.apple_only_examples),
            join_examples(&metric.fff_only_examples)
        );
    }
}

fn join_examples(paths: &[String]) -> String {
    if paths.is_empty() {
        "<none>".to_string()
    } else {
        paths.join(" | ")
    }
}
