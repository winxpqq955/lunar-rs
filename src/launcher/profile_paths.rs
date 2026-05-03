use crate::launcher::models::{DiscoveredProfile, ProfileLoader, ProfileType, ResolvedProfilePaths};
use crate::launcher::paths::default_minecraft_dir;
use std::path::PathBuf;

fn profile_loader_name(profile: &DiscoveredProfile) -> Option<&'static str> {
    match profile.profile.lunar.as_ref().map(|x| x.module.as_str()) {
        Some("fabric") | Some("vanilla-fabric") => Some("fabric"),
        Some("forge") | Some("vanilla-forge") => Some("forge"),
        Some("neoforge") | Some("vanilla-neoforge") => Some("neoforge"),
        Some("quilt") | Some("vanilla-quilt") => Some("quilt"),
        _ => match profile.profile.loaders.first() {
            Some(ProfileLoader::Fabric) => Some("fabric"),
            Some(ProfileLoader::Forge) => Some("forge"),
            Some(ProfileLoader::Neoforge) => Some("neoforge"),
            Some(ProfileLoader::Quilt) => Some("quilt"),
            _ => None,
        },
    }
}

fn mods_dir_name(profile: &DiscoveredProfile) -> String {
    match profile_loader_name(profile) {
        Some(loader) => format!("{loader}-{}", profile.profile.game_version),
        None => profile.profile.game_version.clone(),
    }
}

pub fn resolve_profile_paths(profile: &DiscoveredProfile) -> ResolvedProfilePaths {
    let profile_dir = profile.directory.clone();
    let active_version_dir = match profile.profile.profile_type {
        ProfileType::Modrinth => profile
            .profile
            .modrinth
            .as_ref()
            .map(|m| profile_dir.join("versions").join(&m.selected_version.version_number)),
        ProfileType::CurseForge => profile
            .profile
            .curseforge
            .as_ref()
            .map(|c| profile_dir.join("versions").join(c.selected_version.file_id.to_string())),
        _ => None,
    };

    let content_root = active_version_dir.clone().unwrap_or_else(|| profile_dir.clone());
    let logs_dir = content_root.join("logs");
    let mods_dir = Some(content_root.join("mods"));
    let resourcepacks_dir = Some(content_root.join("resourcepacks"));
    let shaders_dir = Some(content_root.join("shaderpacks"));

    let game_dir = profile
        .profile
        .overrides
        .as_ref()
        .and_then(|o| o.game_directory.as_ref())
        .map(PathBuf::from)
        .unwrap_or_else(default_minecraft_dir);

    ResolvedProfilePaths {
        profile_dir,
        active_version_dir,
        logs_dir,
        mods_dir,
        resourcepacks_dir,
        shaders_dir,
        game_dir,
    }
}
