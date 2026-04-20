use std::env;
use std::path::PathBuf;
use std::process;
use std::time::{Duration, Instant};

use applefind::ContentIndex;
use applefind::dataset::{collect_paths, normalize_root};
use fff_search::grep::{GrepMode, GrepSearchOptions, parse_grep_query};
use fff_search::{FFFMode, FilePicker, FilePickerOptions};

#[derive(Debug)]
struct Config {
    root: PathBuf,
    queries: Vec<String>,
    iters: usize,
    limit: usize,
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

    let build_start = Instant::now();
    let index = ContentIndex::build(&config.root, paths).map_err(|err| err.to_string())?;
    let apple_build = build_start.elapsed();

    let picker_start = Instant::now();
    let mut picker = FilePicker::new(FilePickerOptions {
        base_path: config.root.to_string_lossy().into_owned(),
        enable_mmap_cache: false,
        enable_content_indexing: true,
        mode: FFFMode::Neovim,
        watch: false,
        ..FilePickerOptions::default()
    })
    .map_err(|err| err.to_string())?;
    picker.collect_files().map_err(|err| err.to_string())?;
    let fff_build = picker_start.elapsed();

    println!("repo          : {}", config.root.display());
    println!("iterations    : {}", config.iters);
    println!("limit         : {}", config.limit);
    println!("apple build   : {:?}", apple_build);
    println!("fff build     : {:?}", fff_build);
    println!(
        "{:<20} {:>12} {:>12} {:>9} {:>12} {:>10} {:>10}",
        "query", "applefind", "fff", "speedup", "candidates", "apple m", "fff m"
    );

    for query in &config.queries {
        let apple_elapsed = time_many(config.iters, || index.search_exact(query, config.limit));
        let apple = index.search_exact(query, config.limit);

        let parsed = parse_grep_query(query);
        let options = GrepSearchOptions {
            max_matches_per_file: config.limit.max(1),
            page_limit: config.limit.max(1),
            mode: GrepMode::PlainText,
            ..GrepSearchOptions::default()
        };
        let fff_elapsed = time_many(config.iters, || picker.grep(&parsed, &options));
        let fff = picker.grep(&parsed, &options);

        let speedup = if apple_elapsed.is_zero() {
            0.0
        } else {
            fff_elapsed.as_secs_f64() / apple_elapsed.as_secs_f64()
        };

        println!(
            "{:<20} {:>12} {:>12} {:>8.2}x {:>12} {:>10} {:>10}",
            query,
            fmt_duration(apple_elapsed),
            fmt_duration(fff_elapsed),
            speedup,
            apple.stats.candidate_files,
            apple.stats.returned_matches,
            fff.matches.len()
        );
    }

    Ok(())
}

fn parse_args(args: Vec<String>) -> Result<Config, String> {
    let mut root = None;
    let mut queries = Vec::new();
    let mut iters = 5usize;
    let mut limit = 200usize;

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
            "--query" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--query requires a value".to_string())?;
                queries.push(value.clone());
                i += 2;
            }
            "--iters" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--iters requires a value".to_string())?;
                iters = value
                    .parse::<usize>()
                    .map_err(|_| "invalid iteration count".to_string())?;
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
            "-h" | "--help" | "help" => {
                print_usage();
                process::exit(0);
            }
            other => return Err(format!("unexpected argument: {other}")),
        }
    }

    let root = root.ok_or_else(|| "--root is required".to_string())?;
    if queries.is_empty() {
        queries = vec![
            "QueryParser".to_string(),
            "parse_grep_query".to_string(),
            "fuzzy_search".to_string(),
            "GrepMode".to_string(),
            "TODO".to_string(),
        ];
    }

    Ok(Config {
        root,
        queries,
        iters,
        limit,
    })
}

fn print_usage() {
    println!("Usage:");
    println!(
        "  cargo run --release --features compare-fff --bin compare-fff-grep -- --root PATH [--query TEXT]... [--iters N] [--limit N]"
    );
}

fn time_many<T, F>(iters: usize, mut f: F) -> Duration
where
    F: FnMut() -> T,
{
    let start = Instant::now();
    for _ in 0..iters {
        let _ = f();
    }
    start.elapsed() / (iters as u32)
}

fn fmt_duration(duration: Duration) -> String {
    let micros = duration.as_micros();
    if micros > 1_000 {
        format!("{:.2}ms", micros as f64 / 1_000.0)
    } else {
        format!("{micros}us")
    }
}
