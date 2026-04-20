use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub fn collect_paths(root: &Path) -> io::Result<Vec<String>> {
    use ignore::WalkBuilder;

    let is_git_repo = has_git_ancestor(root);
    let threads = std::thread::available_parallelism()
        .map(|parallelism| parallelism.get())
        .unwrap_or(4);
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(!is_git_repo)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .ignore(true)
        .follow_links(false)
        .threads(threads);

    if !is_git_repo && let Some(overrides) = non_git_repo_overrides(root) {
        builder.overrides(overrides);
    }

    let out = Arc::new(Mutex::new(Vec::new()));
    let walker = builder.build_parallel();
    walker.run(|| {
        let out = Arc::clone(&out);
        let root = root.to_path_buf();
        Box::new(move |entry| {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => return ignore::WalkState::Continue,
            };
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                return ignore::WalkState::Continue;
            }

            let path = entry.path();
            if is_git_file(path) {
                return ignore::WalkState::Continue;
            }
            if !is_git_repo && is_known_binary_extension(path) {
                return ignore::WalkState::Continue;
            }

            let rel = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned();
            if let Ok(mut guard) = out.lock() {
                guard.push(rel);
            }
            ignore::WalkState::Continue
        })
    });

    let mut out = Arc::into_inner(out)
        .expect("parallel walker references should be dropped")
        .into_inner()
        .expect("collector mutex poisoned");
    out.sort_unstable();
    Ok(out)
}

pub fn generate_synthetic_paths(count: usize) -> Vec<String> {
    const TOP_LEVEL: &[&str] = &[
        "src",
        "app",
        "services",
        "packages",
        "modules",
        "tools",
        "experiments",
        "vendor",
    ];
    const DOMAINS: &[&str] = &[
        "auth",
        "billing",
        "chat",
        "compute",
        "config",
        "controller",
        "daemon",
        "gateway",
        "kernel",
        "metrics",
        "pipeline",
        "storage",
        "worker",
    ];
    const KINDS: &[&str] = &[
        "client",
        "config",
        "controller",
        "handler",
        "index",
        "manager",
        "model",
        "routes",
        "service",
        "session",
        "state",
        "view",
    ];
    const LANGS: &[&str] = &["rs", "swift", "ts", "tsx", "py", "go", "cpp", "h"];

    let mut paths = Vec::with_capacity(count);
    for i in 0..count {
        let top = TOP_LEVEL[i % TOP_LEVEL.len()];
        let domain = DOMAINS[(i / TOP_LEVEL.len()) % DOMAINS.len()];
        let nested = DOMAINS[(i / (TOP_LEVEL.len() * DOMAINS.len())) % DOMAINS.len()];
        let kind = KINDS[(i / 7) % KINDS.len()];
        let ext = LANGS[(i / 11) % LANGS.len()];
        let name = format!("{domain}_{kind}_{:05}", i % 100_000);
        let variant = if i % 9 == 0 { "generated" } else { "main" };
        let rel = format!("{top}/{domain}/{nested}/{variant}/{name}.{ext}");
        paths.push(rel);
    }

    paths
}

pub fn default_bench_queries() -> Vec<&'static str> {
    vec![
        "mod",
        "controller",
        "user_authentication",
        "contrlr",
        "src/lib",
        "a",
        "st",
        "test",
        "drivers/net",
        ".rs",
    ]
}

pub fn normalize_root(root: &str) -> io::Result<PathBuf> {
    std::fs::canonicalize(root)
}

fn has_git_ancestor(root: &Path) -> bool {
    let mut current = Some(root);
    while let Some(path) = current {
        if path.join(".git").exists() {
            return true;
        }
        current = path.parent();
    }
    false
}

fn is_git_file(path: &Path) -> bool {
    path.to_str().is_some_and(|path| {
        if cfg!(target_family = "windows") {
            path.contains("\\.git\\")
        } else {
            path.contains("/.git/")
        }
    })
}

fn is_known_binary_extension(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext,
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "bmp"
            | "ico"
            | "webp"
            | "tiff"
            | "tif"
            | "avif"
            | "heic"
            | "psd"
            | "icns"
            | "cur"
            | "raw"
            | "cr2"
            | "nef"
            | "dng"
            | "mp4"
            | "avi"
            | "mov"
            | "wmv"
            | "mkv"
            | "mp3"
            | "wav"
            | "flac"
            | "ogg"
            | "m4a"
            | "aac"
            | "webm"
            | "flv"
            | "mpg"
            | "mpeg"
            | "wma"
            | "opus"
            | "zip"
            | "tar"
            | "gz"
            | "bz2"
            | "xz"
            | "7z"
            | "rar"
            | "zst"
            | "lz4"
            | "lzma"
            | "cab"
            | "cpio"
            | "deb"
            | "rpm"
            | "apk"
            | "dmg"
            | "msi"
            | "iso"
            | "nupkg"
            | "whl"
            | "egg"
            | "snap"
            | "appimage"
            | "flatpak"
            | "exe"
            | "dll"
            | "so"
            | "dylib"
            | "o"
            | "a"
            | "lib"
            | "bin"
            | "elf"
            | "pdf"
            | "doc"
            | "docx"
            | "xls"
            | "xlsx"
            | "ppt"
            | "pptx"
            | "db"
            | "sqlite"
            | "sqlite3"
            | "mdb"
            | "ttf"
            | "otf"
            | "woff"
            | "woff2"
            | "eot"
            | "class"
            | "pyc"
            | "pyo"
            | "wasm"
            | "dex"
            | "jar"
            | "war"
            | "npy"
            | "npz"
            | "pkl"
            | "pickle"
            | "h5"
            | "hdf5"
            | "pt"
            | "pth"
            | "onnx"
            | "safetensors"
            | "tfrecord"
            | "glb"
            | "fbx"
            | "blend"
            | "parquet"
            | "arrow"
            | "pb"
            | "DS_Store"
            | "suo"
    )
}

fn non_git_repo_overrides(base_path: &Path) -> Option<ignore::overrides::Override> {
    use ignore::overrides::OverrideBuilder;

    const NON_GIT_IGNORED_DIRS: &[&str] = &[
        "node_modules",
        "__pycache__",
        "venv",
        ".venv",
        "target/debug",
        "target/release",
        "target/rust-analyzer",
        "target/criterion",
    ];

    #[cfg(target_os = "macos")]
    const PLATFORM_IGNORED_DIRS: &[&str] = &["Library/Application Support", "Library/Caches"];
    #[cfg(not(target_os = "macos"))]
    const PLATFORM_IGNORED_DIRS: &[&str] = &[];

    let mut builder = OverrideBuilder::new(base_path);
    for dir in NON_GIT_IGNORED_DIRS.iter().chain(PLATFORM_IGNORED_DIRS) {
        let pattern = format!("!**/{dir}/");
        if builder.add(&pattern).is_err() {
            continue;
        }
    }

    builder.build().ok()
}

#[cfg(test)]
mod tests {
    use super::generate_synthetic_paths;

    #[test]
    fn synthetic_paths_have_requested_size() {
        let paths = generate_synthetic_paths(128);
        assert_eq!(paths.len(), 128);
        assert!(paths[0].contains('/'));
    }
}
