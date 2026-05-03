use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LauncherPaths {
    pub root: PathBuf,
    pub profiles: PathBuf,
    pub offline_multiver: PathBuf,
    pub natives: PathBuf,
    pub jre: PathBuf,
    pub launcher_cache: PathBuf,
}

fn is_windows_absolute_path(path: &Path) -> bool {
    let value = path.to_string_lossy();
    value.len() > 2 && value.as_bytes()[1] == b':' && matches!(value.as_bytes()[2], b'\\' | b'/')
}

fn custom_launcher_dir() -> Option<PathBuf> {
    env::var_os("LUNAR_CLIENT_DIRECTORY").map(PathBuf::from).map(|path| {
        if path.is_absolute() || is_windows_absolute_path(&path) {
            path
        } else {
            env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
        }
    })
}

fn env_path(key: &str) -> Option<PathBuf> {
    env::var_os(key).filter(|value| !value.is_empty()).map(PathBuf::from)
}

fn windows_user_home_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(user_profile) = env_path("USERPROFILE") {
        candidates.push(user_profile);
    }

    match (env::var_os("HOMEDRIVE"), env::var_os("HOMEPATH")) {
        (Some(drive), Some(path)) if !drive.is_empty() && !path.is_empty() => {
            candidates.push(PathBuf::from(drive).join(path));
        }
        _ => {}
    }

    if cfg!(not(target_os = "windows")) {
        if let Some(users_dir) = Some(PathBuf::from("/mnt/c/Users")).filter(|p| p.exists()) {
            if let Ok(entries) = std::fs::read_dir(users_dir) {
                for entry in entries.filter_map(Result::ok) {
                    candidates.push(entry.path());
                }
            }
        }
    }

    dedupe_existing(candidates)
}

fn dedupe_existing(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut unique = Vec::new();
    for path in paths {
        if unique.iter().any(|existing| existing == &path) {
            continue;
        }
        unique.push(path);
    }
    unique
}

fn existing_or_first(paths: Vec<PathBuf>) -> Option<PathBuf> {
    // Prefer paths where the profiles subdirectory exists (has actual data)
    let with_profiles = paths
        .iter()
        .find(|path| path.join("profiles").exists())
        .cloned();
    if with_profiles.is_some() {
        return with_profiles;
    }
    paths.iter()
        .find(|path| path.exists())
        .cloned()
        .or_else(|| paths.into_iter().next())
}

fn launcher_root_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(custom) = custom_launcher_dir() {
        candidates.push(custom);
    }

    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".lunarclient"));
    }

    candidates.extend(
        windows_user_home_candidates()
            .into_iter()
            .map(|home| home.join(".lunarclient")),
    );

    dedupe_existing(candidates)
}

fn minecraft_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    for home in windows_user_home_candidates() {
        candidates.push(home.join("AppData").join("Roaming").join(".minecraft"));
        candidates.push(home.join(".minecraft"));
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            candidates.push(home.join("Library/Application Support/minecraft"));
        }
    }

    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".minecraft"));
    }

    // Prefer paths that actually exist; put existing candidates first
    let (mut exist, nonexist): (Vec<_>, Vec<_>) =
        dedupe_existing(candidates).into_iter().partition(|p| p.exists());
    exist.extend(nonexist);
    exist
}

pub fn launcher_root() -> PathBuf {
    existing_or_first(launcher_root_candidates()).unwrap_or_else(|| PathBuf::from(".lunarclient"))
}

pub fn default_minecraft_dir() -> PathBuf {
    // Derive from already-resolved launcher root when possible
    let launcher = launcher_root();
    if let Some(parent) = launcher.parent() {
        let appdata = parent.join("AppData").join("Roaming").join(".minecraft");
        if appdata.exists() {
            return appdata;
        }
        let simple = parent.join(".minecraft");
        if simple.exists() {
            return simple;
        }
    }

    existing_or_first(minecraft_dir_candidates()).unwrap_or_else(|| PathBuf::from(".minecraft"))
}

pub fn build_launcher_paths() -> LauncherPaths {
    let root = launcher_root();
    let offline_multiver = root.join("offline").join("multiver");

    LauncherPaths {
        profiles: root.join("profiles"),
        natives: offline_multiver.join("natives"),
        jre: root.join("jre"),
        launcher_cache: root.join("launcher-cache"),
        root,
        offline_multiver,
    }
}
