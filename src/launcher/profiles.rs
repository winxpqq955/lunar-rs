use crate::launcher::models::{DiscoveredProfile, Profile, ProfileState, ProfileType};
use crate::launcher::paths::LauncherPaths;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn discover_profiles(paths: &LauncherPaths) -> Result<Vec<DiscoveredProfile>> {
    let mut profiles = Vec::new();
    let root = &paths.profiles;

    if !root.exists() {
        return Ok(profiles);
    }

    for type_dir in fs::read_dir(root).context("failed to read profiles directory")? {
        let type_dir = type_dir?;
        if !type_dir.file_type()?.is_dir() {
            continue;
        }

        for profile_dir in fs::read_dir(type_dir.path())? {
            let profile_dir = profile_dir?;
            if !profile_dir.file_type()?.is_dir() {
                continue;
            }

            let json_path = profile_dir.path().join("profile.json");
            if !json_path.exists() {
                continue;
            }

            let raw = fs::read_to_string(&json_path)
                .with_context(|| format!("failed to read {}", json_path.display()))?;
            let profile: Profile = match serde_json::from_str(&raw) {
                Ok(profile) => profile,
                Err(_) => continue,
            };

            if matches!(
                profile.profile_type,
                ProfileType::Lunar | ProfileType::Modrinth | ProfileType::CurseForge
            ) {
                profiles.push(DiscoveredProfile {
                    profile,
                    directory: profile_dir.path(),
                });
            }
        }
    }

    profiles.sort_by_key(|p| match p.profile.state {
        Some(ProfileState::Saved) => 0,
        _ => 1,
    });

    Ok(profiles)
}

pub fn find_profile<'a>(profiles: &'a [DiscoveredProfile], id: &str) -> Option<&'a DiscoveredProfile> {
    profiles.iter().find(|profile| profile.profile.id == id)
}

pub fn ensure_profile_launchable(profile: &DiscoveredProfile) -> Result<()> {
    match profile.profile.profile_type {
        ProfileType::Modrinth => {
            let props = profile.profile.modrinth.as_ref().context("missing modrinth profile data")?;
            let selected = &props.selected_version.version_id;
            let installed = props.installed_versions.iter().any(|v| &v.version_id == selected);
            anyhow::ensure!(installed, "selected Modrinth version is not installed");
        }
        ProfileType::CurseForge => {
            let props = profile.profile.curseforge.as_ref().context("missing curseforge profile data")?;
            let selected = props.selected_version.file_id;
            let installed = props.installed_versions.iter().any(|v| v.file_id == selected);
            anyhow::ensure!(installed, "selected CurseForge version is not installed");
        }
        _ => {}
    }
    Ok(())
}

pub fn profile_directory_name(profile: &Profile) -> String {
    match profile.profile_type {
        ProfileType::Modrinth | ProfileType::CurseForge | ProfileType::UserModpack => profile
            .name
            .trim()
            .replace(|c: char| !c.is_ascii_alphanumeric() && c != '.', "-")
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-")
            .to_lowercase(),
        _ => profile.major_game_version.clone(),
    }
}

pub fn expected_profile_directory(base: &Path, profile: &Profile) -> std::path::PathBuf {
    let type_name = match profile.profile_type {
        ProfileType::Lunar => "lunar",
        ProfileType::Modrinth => "modrinth",
        ProfileType::CurseForge => "curseforge",
        ProfileType::UserModpack => "user-modpack",
        ProfileType::Vanilla => "vanilla",
        ProfileType::Badlion => "badlion",
    };

    base.join(type_name).join(profile_directory_name(profile))
}
