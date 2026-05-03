use crate::launcher::api::{
    fetch_indexed_resource, fetch_launch_response, fetch_metadata, resolve_assets_meta,
    resolve_launcher_version, resolve_subversion,
};
use crate::launcher::download::ensure_downloaded;
use crate::launcher::extract::{extract_archive, extract_zip};
use crate::launcher::models::{
    Artifact, AssetIndex, DiscoveredProfile, IndexedResource, JreInfo, LaunchContext, ProfileType,
    UiInfo,
};
use crate::launcher::paths::{build_launcher_paths, LauncherPaths};
use crate::launcher::profile_paths::resolve_profile_paths;
use crate::launcher::profiles::ensure_profile_launchable;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub async fn list_profiles() -> Result<Vec<DiscoveredProfile>> {
    let paths = build_launcher_paths();
    crate::launcher::profiles::discover_profiles(&paths)
}

pub async fn launch_profile(profile: DiscoveredProfile, branch: &str) -> Result<()> {
    ensure_profile_launchable(&profile)?;

    let paths = build_launcher_paths();
    tokio::fs::create_dir_all(&paths.offline_multiver).await?;
    tokio::fs::create_dir_all(&paths.natives).await?;
    tokio::fs::create_dir_all(&paths.jre).await?;
    tokio::fs::create_dir_all(&paths.launcher_cache).await?;
    tokio::fs::create_dir_all(paths.root.join("textures")).await?;
    tokio::fs::create_dir_all(paths.root.join("licenses")).await?;
    tokio::fs::create_dir_all(paths.root.join("ui")).await?;

    let client = reqwest::Client::builder().build()?;

    let metadata = fetch_metadata(&client, branch).await?;
    let subversion = resolve_subversion(&metadata, &profile.profile.game_version)?;
    let launcher_version = resolve_launcher_version(&client).await.unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
    let installation_id = uuid::Uuid::new_v4().to_string();
    let assets_meta = resolve_assets_meta(&metadata, &subversion)?;
    let launch_response = fetch_launch_response(&client, &profile, &subversion, branch).await?;

    if let Some(error) = &launch_response.error {
        anyhow::bail!("launch failed: {} {} {}", error.code, error.short, error.message);
    }

    let profile_paths = resolve_profile_paths(&profile);
    if let Some(version_dir) = &profile_paths.active_version_dir {
        anyhow::ensure!(
            version_dir.exists(),
            "active modpack version directory does not exist: {}",
            version_dir.display()
        );
    }

    let jre = launch_response
        .jre
        .clone()
        .context("launch response missing JRE info")?;
    let java_path = ensure_jre(&client, &paths, &jre).await?;

    let launch_type_data = launch_response
        .launch_type_data
        .clone()
        .context("launch response missing launchTypeData")?;

    download_licenses(&client, &paths, &launch_response.licenses).await?;
    if let Some(textures) = &launch_response.textures {
        download_indexed_assets(&client, &paths.root.join("textures"), textures, "texturesIndex.txt").await?;
    }
    if let Some(ui) = &launch_response.ui {
        download_ui(&client, &paths, ui).await?;
    }

    let artifact_result = download_artifacts(&client, &paths, &launch_type_data.artifacts).await?;
    extract_natives(&paths, &launch_type_data.artifacts)?;

    let game_dir = profile_paths.game_dir.clone();

    let context = LaunchContext {
        selected_profile: profile,
        profile_paths,
        metadata,
        assets_meta: assets_meta.clone(),
        launch_response,
        java_path,
        classpath_entries: artifact_result.classpath_entries,
        launcher_version,
        subversion,
        ichor_classpath: artifact_result.ichor_classpath,
        ichor_external_files: artifact_result.ichor_external_files,
        installation_id,
    };

    println!("[launcher] subversion={} launcher_version={} ichor_classpath={} ichor_external_files={} game_dir={}",
        context.subversion,
        context.launcher_version,
        context.ichor_classpath.len(),
        context.ichor_external_files.len(),
        context.profile_paths.game_dir.display(),
    );

    download_assets(&client, &game_dir, &assets_meta).await?;

    spawn_java(context)
}

async fn ensure_jre(client: &reqwest::Client, paths: &LauncherPaths, jre: &JreInfo) -> Result<PathBuf> {
    let archive_url = &jre.download.url;
    let archive_name = archive_url.split('/').next_back().unwrap_or("jre.zip");
    let archive_path = paths.jre.join(archive_name);
    let destination = paths.jre.join(&jre.folder_checksum);

    if !destination.exists() {
        if ensure_downloaded(client, archive_url, &archive_path, None)
            .await
            .is_err()
        {
            if let Some(fallback) = &jre.download.fallback_url {
                ensure_downloaded(client, fallback, &archive_path, None).await?;
            } else {
                anyhow::bail!("failed to download jre");
            }
        }
        extract_archive(&archive_path, &destination)?;
    }

    let java_path = jre
        .executable_path_in_archive
        .iter()
        .fold(destination, |acc, part| acc.join(part));
    Ok(java_path)
}

#[derive(Debug, Clone)]
struct ArtifactResult {
    classpath_entries: Vec<PathBuf>,
    ichor_classpath: Vec<String>,
    ichor_external_files: Vec<String>,
}

async fn download_artifacts(client: &reqwest::Client, paths: &LauncherPaths, artifacts: &[Artifact]) -> Result<ArtifactResult> {
    let mut classpath = Vec::new();
    let mut ichor_classpath = Vec::new();
    let mut ichor_external_files = Vec::new();
    for artifact in artifacts {
        let destination = paths.offline_multiver.join(&artifact.name);
        ensure_downloaded(client, &artifact.url, &destination, Some(&artifact.sha1)).await?;
        if artifact.artifact_type == "CLASS_PATH" {
            classpath.push(destination);
            ichor_classpath.push(artifact.name.clone());
        } else if artifact.artifact_type == "EXTERNAL_FILE" {
            ichor_external_files.push(artifact.name.clone());
        }
    }
    Ok(ArtifactResult { classpath_entries: classpath, ichor_classpath, ichor_external_files })
}

fn extract_natives(paths: &LauncherPaths, artifacts: &[Artifact]) -> Result<()> {
    for artifact in artifacts.iter().filter(|a| a.artifact_type == "NATIVES") {
        let archive = paths.offline_multiver.join(&artifact.name);
        if archive.exists() {
            extract_zip(&archive, &paths.natives)?;
        }
    }
    Ok(())
}

async fn download_assets(client: &reqwest::Client, game_dir: &Path, assets_meta: &crate::launcher::models::LunarAssetsMeta) -> Result<()> {
    let indexes_dir = game_dir.join("assets").join("indexes");
    let index_name = assets_meta
        .url
        .split('/')
        .next_back()
        .unwrap_or("assets.json");
    let index_path = indexes_dir.join(index_name);
    ensure_downloaded(client, &assets_meta.url, &index_path, Some(&assets_meta.sha1)).await?;

    let bytes = tokio::fs::read(&index_path).await?;
    let index: AssetIndex = serde_json::from_slice(&bytes)?;
    for object in index.objects.into_values() {
        let prefix = &object.hash[..2];
        let destination = game_dir.join("assets").join("objects").join(prefix).join(&object.hash);
        let url = format!("https://resources.download.minecraft.net/{prefix}/{}", object.hash);
        ensure_downloaded(client, &url, &destination, Some(&object.hash)).await?;
    }

    Ok(())
}

async fn download_licenses(
    client: &reqwest::Client,
    paths: &LauncherPaths,
    licenses: &[crate::launcher::models::LicenseInfo],
) -> Result<()> {
    for license in licenses {
        let destination = paths.root.join("licenses").join(&license.file);
        ensure_downloaded(client, &license.url, &destination, Some(&license.sha1)).await?;
    }
    Ok(())
}

async fn download_indexed_assets(
    client: &reqwest::Client,
    destination: &Path,
    indexed: &IndexedResource,
    index_name: &str,
) -> Result<()> {
    let index_cache = build_launcher_paths().launcher_cache.join(index_name);
    ensure_downloaded(client, &indexed.index_url, &index_cache, Some(&indexed.index_sha1)).await?;
    let index_body = fetch_indexed_resource(client, indexed).await.unwrap_or_else(|_| std::fs::read_to_string(&index_cache).unwrap_or_default());

    for line in index_body.lines() {
        let mut parts = line.split_whitespace();
        let Some(relative_path) = parts.next() else { continue; };
        let Some(hash) = parts.next() else { continue; };
        let url = format!("{}{}", indexed.base_url, hash);
        let target = destination.join(relative_path);
        ensure_downloaded(client, &url, &target, Some(hash)).await?;
    }
    Ok(())
}

async fn download_ui(client: &reqwest::Client, paths: &LauncherPaths, ui: &UiInfo) -> Result<()> {
    let zip_path = paths.launcher_cache.join(format!("ui-{}.zip", ui.source_sha1));
    let version_dir = paths.root.join("ui").join(&ui.source_sha1);
    ensure_downloaded(client, &ui.source_url, &zip_path, Some(&ui.source_sha1)).await?;
    if !version_dir.exists() {
        extract_zip(&zip_path, &version_dir)?;
    }
    download_indexed_assets(client, &version_dir.join("assets"), &ui.assets, &format!("uiIndex-{}.txt", ui.source_sha1)).await
}

fn spawn_java(context: LaunchContext) -> Result<()> {
    let launch_type_data = context
        .launch_response
        .launch_type_data
        .as_ref()
        .context("missing launchTypeData")?;
    let profile = &context.selected_profile.profile;
    let mut java_args = context
        .launch_response
        .jre
        .as_ref()
        .map(|j| j.extra_arguments.clone())
        .unwrap_or_default();

    let memory = profile
        .overrides
        .as_ref()
        .and_then(|o| o.allocated_memory)
        .unwrap_or(4096);
    java_args.push(format!("-Xmx{}m", memory));

    if let Some(mods_dir) = &context.profile_paths.mods_dir {
        if mods_dir.exists() {
            java_args.push(format!("-Dichor.fabric.localModPath={}", mods_dir.display()));
        }
    }

    java_args.push("-Djava.library.path=natives".to_string());

    std::fs::create_dir_all(&context.profile_paths.logs_dir).ok();
    java_args.push(format!("-Dlog4j.configurationFile={}", context.profile_paths.logs_dir.join("config.xml").display()));
    java_args.push(format!("-Dichor.logsFile={}", context.profile_paths.logs_dir.join("ichor-boot.log").display()));

    java_args.push("-XX:+DisableAttachMechanism".to_string());
    java_args.push("-XX:-CreateCoredumpOnCrash".to_string());
    java_args.push("-XX:-CreateMinidumpOnCrash".to_string());

    if let Some(extra) = profile.overrides.as_ref().and_then(|o| o.jvm_arguments.as_ref()) {
        java_args.extend(extra.split_whitespace().map(ToOwned::to_owned));
    }

    let separator = if cfg!(windows) { ";" } else { ":" };
    let classpath = context
        .classpath_entries
        .iter()
        .map(|p| p.file_name().and_then(|n| n.to_str()).unwrap_or_default())
        .collect::<Vec<_>>()
        .join(separator);
    java_args.push("-cp".to_string());
    java_args.push(classpath);
    java_args.push(launch_type_data.main_class.clone());

    java_args.extend(program_args(&context));

    let mut command = Command::new(&context.java_path);
    command.current_dir(&build_launcher_paths().offline_multiver);
    command.args(&java_args);
    println!("[launcher] spawning: {:?}", command);

    let status = command.status().context("failed to spawn java")?;
    anyhow::ensure!(status.success(), "java exited with status {status}");
    Ok(())
}

fn program_args(context: &LaunchContext) -> Vec<String> {
    let profile = &context.selected_profile.profile;
    let ui_dir = context
        .launch_response
        .ui
        .as_ref()
        .map(|u| build_launcher_paths().root.join("ui").join(&u.source_sha1))
        .unwrap_or_else(|| build_launcher_paths().root.join("ui"));

    let mut args = vec![
        "--version".to_string(),
        context.subversion.clone(),
        "--launcherVersion".to_string(),
        context.launcher_version.clone(),
        "--launcherFeatureFlags".to_string(),
        "{\"enabled\":[],\"disabled\":[]}".to_string(),
        "--installationId".to_string(),
        context.installation_id.clone(),
        "--username".to_string(),
        "Player".to_string(),
        "--uuid".to_string(),
        "00000000-0000-0000-0000-000000000000".to_string(),
        "--accessToken".to_string(),
        "0".to_string(),
        "--userProperties".to_string(),
        "{}".to_string(),
        "--assetIndex".to_string(),
        profile.major_game_version.clone(),
        "--gameDir".to_string(),
        context.profile_paths.game_dir.to_string_lossy().to_string(),
        "--texturesDir".to_string(),
        build_launcher_paths().root.join("textures").to_string_lossy().to_string(),
        "--uiDir".to_string(),
        ui_dir.to_string_lossy().to_string(),
        "--webosrDir".to_string(),
        build_launcher_paths().natives.to_string_lossy().to_string(),
        "--workingDirectory".to_string(),
        ".".to_string(),
        "--classpathDir".to_string(),
        ".".to_string(),
        "--width".to_string(),
        "854".to_string(),
        "--height".to_string(),
        "480".to_string(),
        "--ipcPort".to_string(),
        "28190".to_string(),
    ];

    if !context.ichor_classpath.is_empty() {
        args.push("--ichorClassPath".to_string());
        args.push(context.ichor_classpath.join(","));
    }

    if !context.ichor_external_files.is_empty() {
        args.push("--ichorExternalFiles".to_string());
        args.push(context.ichor_external_files.join(","));
    }

    if matches!(profile.profile_type, ProfileType::Modrinth) {
        if let Some(modrinth) = &profile.modrinth {
            args.push("--modrinthModpackProjectId".to_string());
            args.push(modrinth.project_id.clone());
            args.push("--modrinthModpackVersionId".to_string());
            args.push(modrinth.selected_version.version_id.clone());
        }
    }

    args
}
