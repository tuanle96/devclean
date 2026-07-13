use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Returns the exact allowlist for rebuildable build, package, and tool caches.
#[must_use]
pub fn global_cache_paths(home: &Path) -> Vec<PathBuf> {
    let mut paths = [
        ".npm/_cacache",
        ".npm/_npx",
        ".cargo/registry/cache",
        ".cargo/registry/src",
        ".cargo/registry/index",
        ".cargo/git/db",
        ".gradle/caches",
        ".gradle/wrapper/dists",
        "go/pkg/mod",
        ".cache/uv",
        ".cache/pip",
        ".cache/puppeteer",
        ".cache/gem",
        ".pub-cache/hosted",
        ".pub-cache/hosted-hashes",
        ".pub-cache/git",
    ]
    .into_iter()
    .map(|relative| home.join(relative))
    .collect::<Vec<_>>();
    if let Some(path) = configured_go_mod_cache() {
        if path.is_absolute() && !paths.contains(&path) {
            paths.push(path);
        }
    }
    if cfg!(target_os = "macos") {
        paths.extend(
            [
                "Library/pnpm",
                "Library/Caches/ms-playwright",
                "Library/Caches/node-gyp",
                "Library/Developer/Xcode/DerivedData",
            ]
            .into_iter()
            .map(|relative| home.join(relative)),
        );
    }
    if cfg!(target_os = "windows") {
        paths.extend(
            [
                "AppData/Local/npm-cache",
                "AppData/Local/pnpm/store",
                "AppData/Local/ms-playwright",
                "AppData/Local/node-gyp/Cache",
            ]
            .into_iter()
            .map(|relative| home.join(relative)),
        );
    }
    paths
}

fn configured_go_mod_cache() -> Option<PathBuf> {
    env::var_os("GOMODCACHE")
        .map(PathBuf::from)
        .or_else(go_env_mod_cache)
}

fn go_env_mod_cache() -> Option<PathBuf> {
    let output = Command::new("go")
        .args(["env", "GOMODCACHE"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| parse_go_mod_cache_output(&output.stdout))
        .flatten()
}

fn parse_go_mod_cache_output(output: &[u8]) -> Option<PathBuf> {
    let value = String::from_utf8_lossy(output);
    let path = PathBuf::from(value.trim());
    (!path.as_os_str().is_empty() && path.is_absolute()).then_some(path)
}

/// Returns the exact allowlist for caches that can be expensive to download again.
#[must_use]
pub fn expensive_global_cache_paths(home: &Path) -> Vec<PathBuf> {
    [
        ".cache/codex-runtimes",
        ".cache/huggingface",
        ".cache/whisper",
    ]
    .into_iter()
    .map(|relative| home.join(relative))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_env_output_should_accept_trimmed_absolute_path() {
        let expected = env::temp_dir().join("go-mod-cache");
        let output = format!("{}\n", expected.display());
        assert_eq!(parse_go_mod_cache_output(output.as_bytes()), Some(expected));
    }

    #[test]
    fn go_env_output_should_reject_empty_or_relative_path() {
        assert!(parse_go_mod_cache_output(b"\n").is_none());
        assert!(parse_go_mod_cache_output(b"relative/cache\n").is_none());
    }

    #[test]
    fn global_cache_allowlist_should_keep_managed_storage_out() {
        let home = env::temp_dir().join("devclean-cache-allowlist-home");
        let paths = global_cache_paths(&home);

        assert!(paths.contains(&home.join(".gradle/caches")));
        assert!(paths.contains(&home.join(".gradle/wrapper/dists")));
        assert!(!paths.contains(&home.join(".gradle")));
        assert!(!paths.contains(&home.join(".android/avd")));
        assert!(!paths.contains(&home.join("Library/Developer/CoreSimulator")));
        assert!(!paths.contains(&home.join("Library/Android/sdk")));
        assert!(!paths.contains(&home.join("Library/Caches/JetBrains")));
        assert!(!paths.contains(&home.join("Library/Containers/com.docker.docker")));
        if cfg!(target_os = "macos") {
            assert!(paths.contains(&home.join("Library/Developer/Xcode/DerivedData")));
        }
    }
}
