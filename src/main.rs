use std::env;
use std::path::PathBuf;
use std::process;
use std::time::{Duration, Instant};

use applefind::dataset::{
    collect_paths, default_bench_queries, generate_synthetic_paths, normalize_root,
};
use applefind::{PathIndex, SearchResult};

enum DatasetSource {
    Root(PathBuf),
    Synthetic(usize),
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return Ok(());
    };

    match command.as_str() {
        "search" => run_search(args.collect()),
        "bench" => run_bench(args.collect()),
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown command: {other}")),
    }
}

fn run_search(args: Vec<String>) -> Result<(), String> {
    let mut source = None;
    let mut limit = 20usize;
    let mut query_parts = Vec::new();

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--root requires a value".to_string())?;
                source = Some(DatasetSource::Root(
                    normalize_root(value).map_err(|err| err.to_string())?,
                ));
                i += 2;
            }
            "--synthetic" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--synthetic requires a value".to_string())?;
                let count = value
                    .parse::<usize>()
                    .map_err(|_| "invalid synthetic count".to_string())?;
                source = Some(DatasetSource::Synthetic(count));
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
            value => {
                query_parts.push(value.to_string());
                i += 1;
            }
        }
    }

    let query = query_parts.join(" ");
    if query.is_empty() {
        return Err("search requires a query".to_string());
    }

    let source = source.unwrap_or(DatasetSource::Root(
        normalize_root(".").map_err(|err| err.to_string())?,
    ));

    let (paths, load_time) = load_paths(source)?;
    let build_start = Instant::now();
    let index = PathIndex::build(paths);
    let build_time = build_start.elapsed();

    let scan = index.search_scan(&query, limit);
    let indexed = index.search_indexed(&query, limit);

    println!(
        "Loaded {} paths in {:?}, built index in {:?}",
        index.len(),
        load_time,
        build_time
    );
    print_result("scan", &scan);
    print_result("indexed", &indexed);

    let same = scan.hits == indexed.hits;
    println!("same hits: {same}");
    Ok(())
}

fn run_bench(args: Vec<String>) -> Result<(), String> {
    let mut source = None;
    let mut iters = 100usize;
    let mut limit = 100usize;
    let mut queries = Vec::new();

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--root requires a value".to_string())?;
                source = Some(DatasetSource::Root(
                    normalize_root(value).map_err(|err| err.to_string())?,
                ));
                i += 2;
            }
            "--synthetic" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--synthetic requires a value".to_string())?;
                let count = value
                    .parse::<usize>()
                    .map_err(|_| "invalid synthetic count".to_string())?;
                source = Some(DatasetSource::Synthetic(count));
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
            "--query" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--query requires a value".to_string())?;
                queries.push(value.clone());
                i += 2;
            }
            other => {
                return Err(format!("unexpected argument: {other}"));
            }
        }
    }

    let source = source.unwrap_or(DatasetSource::Synthetic(250_000));
    let (paths, load_time) = load_paths(source)?;
    let build_start = Instant::now();
    let index = PathIndex::build(paths);
    let build_time = build_start.elapsed();

    println!("entries      : {}", index.len());
    println!("load time    : {:?}", load_time);
    println!("build time   : {:?}", build_time);
    println!("iterations   : {iters}");
    println!(
        "{:<18} {:>12} {:>12} {:>10} {:>12} {:>8}",
        "query", "scan", "indexed", "speedup", "candidates", "hits"
    );

    let bench_queries: Vec<String> = if queries.is_empty() {
        default_bench_queries()
            .into_iter()
            .map(str::to_string)
            .collect()
    } else {
        queries
    };

    for query in &bench_queries {
        let scan_elapsed = time_many(iters, || index.search_scan(query, limit));
        let indexed = index.search_indexed(query, limit);
        let indexed_elapsed = time_many(iters, || index.search_indexed(query, limit));
        let speedup = if indexed_elapsed.is_zero() {
            0.0
        } else {
            scan_elapsed.as_secs_f64() / indexed_elapsed.as_secs_f64()
        };

        println!(
            "{:<18} {:>12} {:>12} {:>9.2}x {:>12} {:>8}",
            query,
            fmt_duration(scan_elapsed),
            fmt_duration(indexed_elapsed),
            speedup,
            indexed.stats.candidate_count,
            indexed.stats.total_matches
        );
    }

    Ok(())
}

fn load_paths(source: DatasetSource) -> Result<(Vec<String>, Duration), String> {
    let start = Instant::now();
    let paths = match source {
        DatasetSource::Root(root) => collect_paths(&root).map_err(|err| err.to_string())?,
        DatasetSource::Synthetic(count) => generate_synthetic_paths(count),
    };
    Ok((paths, start.elapsed()))
}

fn time_many<F>(iters: usize, mut f: F) -> Duration
where
    F: FnMut() -> SearchResult,
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

fn print_result(label: &str, result: &SearchResult) {
    println!(
        "{label}: hits={} candidates={} scanned={}",
        result.stats.total_matches, result.stats.candidate_count, result.stats.scanned_entries
    );
    for hit in &result.hits {
        println!("  {:>6} {}", hit.score, hit.path);
    }
}

fn print_usage() {
    println!("applefind");
    println!();
    println!("Commands:");
    println!("  search [--root PATH | --synthetic N] [--limit N] QUERY...");
    println!("  bench  [--root PATH | --synthetic N] [--iters N] [--limit N] [--query TEXT]...");
}
