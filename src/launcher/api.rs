use crate::launcher::models::{
    DiscoveredProfile, IndexedResource, LaunchProfile, LaunchProfileModrinth, LaunchRequestBody,
    LaunchResponse, LunarAssetsMeta, LunarVersionsResponse, MetadataState, ProfileType,
};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn api_root() -> &'static str {
    "https://api.lunarclientprod.com"
}

#[derive(Debug, Clone)]
pub struct ParsedBranch {
    pub branch: String,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone)]
struct BaseApiRequestData {
    installation_id: String,
    overwolf_muid: Option<String>,
    os: String,
    os_release: String,
    arch: String,
    launcher_version: String,
}

#[derive(Debug, Deserialize)]
struct MiscMetadataResponse {
    updater: UpdaterMetadata,
}

#[derive(Debug, Deserialize)]
struct UpdaterMetadata {
    #[serde(rename = "feedUrl")]
    feed_url: String,
}

pub fn parse_branch(input: &str) -> ParsedBranch {
    let full_branch = if input.is_empty() { "master" } else { input };
    let mut parts = full_branch.replace(['/', '.', '+'], "_").split(':').map(str::to_string).collect::<Vec<_>>();
    let branch = parts.pop().unwrap_or_else(|| "master".to_string());
    let flags = parts
        .into_iter()
        .filter(|part| matches!(part.as_str(), "ds" | "lui" | "dui" | "cef" | "dau"))
        .collect();
    ParsedBranch { branch, flags }
}

pub async fn fetch_metadata(client: &reqwest::Client, branch: &str) -> Result<MetadataState> {
    let base = base_request_data(client).await?;
    let response = client
        .get(format!("{}/launcher/metadata/versions/lunar", api_root()))
        .headers(api_headers(&base))
        .query(&base_query_pairs(&base))
        .query(&[("branch", branch)])
        .send()
        .await?
        .error_for_status()?
        .json::<LunarVersionsResponse>()
        .await
        .context("failed to parse lunar metadata response")?;

    let mut subversion_assets = HashMap::new();
    for version in response.versions {
        for subversion in version.subversions {
            subversion_assets.insert(subversion.id, subversion.assets);
        }
    }

    Ok(MetadataState { subversion_assets })
}

pub async fn fetch_launch_response(
    client: &reqwest::Client,
    profile: &DiscoveredProfile,
    version: &str,
    raw_branch: &str,
) -> Result<LaunchResponse> {
    let parsed = parse_branch(raw_branch);
    let base = base_request_data(client).await?;
    let module = profile
        .profile
        .lunar
        .as_ref()
        .map(|x| x.module.clone())
        .unwrap_or_else(|| "lunar".to_string());

    let body = LaunchRequestBody {
        version: version.to_string(),
        branch: parsed.branch,
        args: parsed.flags,
        module,
        canary_preference: "NEUTRAL".to_string(),
        profile: LaunchProfile {
            id: profile.profile.id.clone(),
            name: profile.profile.name.clone(),
            modrinth: if matches!(profile.profile.profile_type, ProfileType::Modrinth) {
                profile.profile.modrinth.as_ref().map(|m| LaunchProfileModrinth {
                    id: m.project_id.clone(),
                    version_id: m.selected_version.version_id.clone(),
                })
            } else {
                None
            },
        },
        installation_id: base.installation_id.clone(),
        overwolf_muid: base.overwolf_muid.clone(),
        os: base.os.clone(),
        os_release: base.os_release.clone(),
        arch: base.arch.clone(),
        launcher_version: base.launcher_version.clone(),
    };

    let response = client
        .post(format!("{}/launcher/launch", api_root()))
        .headers(api_headers(&base))
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json::<LaunchResponse>()
        .await
        .context("failed to parse launch response")?;

    Ok(response)
}

pub fn resolve_subversion(metadata: &MetadataState, game_version: &str) -> Result<String> {
    metadata
        .subversion_assets
        .keys()
        .filter(|k| k.starts_with(game_version))
        .max_by_key(|k| k.len())
        .cloned()
        .with_context(|| format!("no subversion found for game version {game_version}"))
}

pub fn resolve_assets_meta(metadata: &MetadataState, subversion: &str) -> Result<LunarAssetsMeta> {
    metadata
        .subversion_assets
        .get(subversion)
        .cloned()
        .with_context(|| format!("missing assets metadata for subversion {subversion}"))
}

pub async fn fetch_indexed_resource(client: &reqwest::Client, indexed: &IndexedResource) -> Result<String> {
    client
        .get(&indexed.index_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await
        .context("failed to read indexed resource")
}

async fn base_request_data(client: &reqwest::Client) -> Result<BaseApiRequestData> {
    Ok(BaseApiRequestData {
        installation_id: Uuid::new_v4().to_string(),
        overwolf_muid: None,
        os: if cfg!(windows) { "win32" } else { env::consts::OS }.to_string(),
        os_release: os_info::get().version().to_string(),
        arch: match env::consts::ARCH {
            "x86_64" => "x64",
            other => other,
        }
        .to_string(),
        launcher_version: resolve_launcher_version(client)
            .await
            .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string()),
    })
}

pub async fn resolve_launcher_version(client: &reqwest::Client) -> Result<String> {
    let misc = client
        .get(format!("{}/launcher/metadata/misc", api_root()))
        .query(&[("launcher_update_stream", "latest")])
        .send()
        .await?
        .error_for_status()?
        .json::<MiscMetadataResponse>()
        .await
        .context("failed to parse misc metadata response")?;

    let manifest = client
        .get(format!("{}/latest.yml", misc.updater.feed_url.trim_end_matches('/')))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await
        .context("failed to read launcher update manifest")?;

    parse_launcher_version(&manifest).context("failed to parse launcher version from updater manifest")
}

fn parse_launcher_version(manifest: &str) -> Result<String> {
    manifest
        .lines()
        .find_map(|line| line.strip_prefix("version:").map(str::trim))
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .context("missing version field in updater manifest")
}

fn api_headers(base: &BaseApiRequestData) -> reqwest::header::HeaderMap {
    use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};

    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(&format!("Lunar Client Launcher v{}", base.launcher_version)).unwrap(),
    );
    headers.insert(
        "sentry-trace",
        HeaderValue::from_str(&fake_sentry_trace()).unwrap(),
    );
    headers.insert(
        "X-Installation-Id",
        HeaderValue::from_str(&base.installation_id).unwrap(),
    );
    if let Some(overwolf_muid) = &base.overwolf_muid {
        headers.insert(
            "X-Overwolf-Muid",
            HeaderValue::from_str(overwolf_muid).unwrap(),
        );
    }
    headers
}

fn base_query_pairs(base: &BaseApiRequestData) -> Vec<(&'static str, String)> {
    let mut pairs = vec![
        ("installation_id", base.installation_id.clone()),
        ("os", base.os.clone()),
        ("os_release", base.os_release.clone()),
        ("arch", base.arch.clone()),
        ("launcher_version", base.launcher_version.clone()),
    ];
    if let Some(overwolf_muid) = &base.overwolf_muid {
        pairs.push(("overwolf_muid", overwolf_muid.clone()));
    }
    pairs
}

fn fake_sentry_trace() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let trace = format!("{:032x}", now);
    let span = format!("{:016x}", now & 0xffff_ffff_ffff_ffff);
    format!("{trace}-{span}-1")
}
