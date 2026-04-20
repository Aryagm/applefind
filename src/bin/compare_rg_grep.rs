use std::collections::HashSet;
use std::env;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{self, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use applefind::ContentIndex;
use applefind::dataset::{collect_paths, normalize_root};

#[derive(Debug)]
struct Config {
    root: PathBuf,
    queries: Vec<String>,
    iters: usize,
    limit: usize,
}

#[derive(Debug, Clone, Copy)]
struct RgQueryResult {
    returned_lines: usize,
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

    println!("repo          : {}", config.root.display());
    println!("iterations    : {}", config.iters);
    println!("limit         : {}", config.limit);
    println!("apple build   : {:?}", apple_build);
    println!(
        "{:<20} {:>12} {:>12} {:>9} {:>12} {:>10} {:>10} {:>10}",
        "query", "applefind", "ripgrep", "speedup", "candidates", "apple l", "rg l", "break-even"
    );

    for query in &config.queries {
        let apple_elapsed =
            time_many(config.iters, || Ok(index.search_exact(query, config.limit)))?;
        let apple = index.search_exact(query, config.limit);
        let apple_lines = unique_match_lines(&apple);

        let rg_elapsed = time_many(config.iters, || {
            run_rg_query(&config.root, query, config.limit)
        })?;
        let rg = run_rg_query(&config.root, query, config.limit)?;

        let speedup = if apple_elapsed.is_zero() {
            0.0
        } else {
            rg_elapsed.as_secs_f64() / apple_elapsed.as_secs_f64()
        };

        println!(
            "{:<20} {:>12} {:>12} {:>8.2}x {:>12} {:>10} {:>10} {:>10}",
            query,
            fmt_duration(apple_elapsed),
            fmt_duration(rg_elapsed),
            speedup,
            apple.stats.candidate_files,
            apple_lines,
            rg.returned_lines,
            fmt_break_even(apple_build, apple_elapsed, rg_elapsed),
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
            "module_init".to_string(),
            "copy_from_user".to_string(),
            "spin_lock_irqsave".to_string(),
            "EXPORT_SYMBOL_GPL".to_string(),
            "of_match_ptr".to_string(),
            "dma_alloc_coherent".to_string(),
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
        "  cargo run --release --bin compare_rg_grep -- --root PATH [--query TEXT]... [--iters N] [--limit N]"
    );
}

fn run_rg_query(root: &PathBuf, query: &str, limit: usize) -> Result<RgQueryResult, String> {
    let mut child = Command::new("rg")
        .current_dir(root)
        .args([
            "--no-config",
            "--hidden",
            "--smart-case",
            "--fixed-strings",
            "--line-number",
            "--color",
            "never",
            "--max-filesize",
            "10M",
        ])
        .arg(query)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to spawn rg: {err}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture rg stdout".to_string())?;
    let mut reader = BufReader::new(stdout);
    let mut lines = 0usize;
    let mut buf = String::new();
    let mut hit_limit = false;

    loop {
        buf.clear();
        let read = reader
            .read_line(&mut buf)
            .map_err(|err| format!("failed reading rg output: {err}"))?;
        if read == 0 {
            break;
        }
        lines += 1;
        if limit > 0 && lines >= limit {
            hit_limit = true;
            break;
        }
    }

    let status = if hit_limit {
        child.kill().ok();
        child
            .wait()
            .map_err(|err| format!("failed waiting for rg after kill: {err}"))?
    } else {
        child
            .wait()
            .map_err(|err| format!("failed waiting for rg: {err}"))?
    };

    let stderr = child
        .stderr
        .take()
        .map(BufReader::new)
        .map(read_to_string)
        .transpose()
        .map_err(|err| format!("failed reading rg stderr: {err}"))?
        .unwrap_or_default();

    if !status_ok(status, hit_limit) {
        return Err(format!("rg exited with {status}: {stderr}"));
    }

    Ok(RgQueryResult {
        returned_lines: lines,
    })
}

fn read_to_string<R: BufRead>(mut reader: R) -> Result<String, std::io::Error> {
    let mut output = String::new();
    reader.read_to_string(&mut output)?;
    Ok(output)
}

fn status_ok(status: ExitStatus, hit_limit: bool) -> bool {
    if hit_limit {
        return true;
    }
    matches!(status.code(), Some(0 | 1))
}

fn unique_match_lines(result: &applefind::ContentSearchResult) -> usize {
    let mut seen = HashSet::with_capacity(result.matches.len());
    for item in &result.matches {
        seen.insert((item.path.as_str(), item.line_number));
    }
    seen.len()
}

fn time_many<T, F>(iters: usize, mut f: F) -> Result<Duration, String>
where
    F: FnMut() -> Result<T, String>,
{
    let start = Instant::now();
    for _ in 0..iters {
        let _ = f()?;
    }
    Ok(start.elapsed() / (iters as u32))
}

fn fmt_duration(duration: Duration) -> String {
    let micros = duration.as_micros();
    if micros > 1_000 {
        format!("{:.2}ms", micros as f64 / 1_000.0)
    } else {
        format!("{micros}us")
    }
}

fn fmt_break_even(build: Duration, apple_query: Duration, rg_query: Duration) -> String {
    if apple_query >= rg_query {
        return "-".to_string();
    }

    let savings = rg_query.as_secs_f64() - apple_query.as_secs_f64();
    if savings <= 0.0 {
        return "-".to_string();
    }

    let queries = (build.as_secs_f64() / savings).ceil();
    format!("{queries:.0}q")
}
