use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileType {
    Lunar,
    Modrinth,
    #[serde(rename = "user-modpack")]
    UserModpack,
    Vanilla,
    CurseForge,
    Badlion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileState {
    Saved,
    Virtual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileLoader {
    Ichor,
    Fabric,
    Forge,
    Optifine,
    Neoforge,
    Quilt,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileOverrides {
    pub allocated_memory: Option<u32>,
    pub game_directory: Option<String>,
    pub jvm_arguments: Option<String>,
    pub loader_version: Option<String>,
    pub pre_launch_command: Option<String>,
    pub wrapper_command: Option<String>,
    pub post_exit_command: Option<String>,
    pub environment_variables: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthProfileVersion {
    #[serde(rename = "versionId")]
    pub version_id: String,
    #[serde(rename = "versionNumber")]
    pub version_number: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthProfileProps {
    #[serde(rename = "projectId")]
    pub project_id: String,
    #[serde(rename = "selectedVersion")]
    pub selected_version: ModrinthProfileVersion,
    #[serde(rename = "installedVersions", default)]
    pub installed_versions: Vec<ModrinthProfileVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurseForgeVersion {
    #[serde(rename = "fileId")]
    pub file_id: u64,
    #[serde(rename = "displayName")]
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurseForgeProfileProps {
    #[serde(rename = "modId")]
    pub mod_id: u64,
    #[serde(rename = "selectedVersion")]
    pub selected_version: CurseForgeVersion,
    #[serde(rename = "installedVersions", default)]
    pub installed_versions: Vec<CurseForgeVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LunarProfileProps {
    pub module: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "majorGameVersion")]
    pub major_game_version: String,
    #[serde(rename = "gameVersion")]
    pub game_version: String,
    pub loaders: Vec<ProfileLoader>,
    #[serde(rename = "type")]
    pub profile_type: ProfileType,
    pub state: Option<ProfileState>,
    pub modrinth: Option<ModrinthProfileProps>,
    pub curseforge: Option<CurseForgeProfileProps>,
    pub lunar: Option<LunarProfileProps>,
    #[serde(rename = "configVersion")]
    pub config_version: u32,
    pub overrides: Option<ProfileOverrides>,
    #[serde(rename = "useLunarFeatures")]
    pub use_lunar_features: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct DiscoveredProfile {
    pub profile: Profile,
    pub directory: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ResolvedProfilePaths {
    pub profile_dir: PathBuf,
    pub active_version_dir: Option<PathBuf>,
    pub logs_dir: PathBuf,
    pub mods_dir: Option<PathBuf>,
    pub resourcepacks_dir: Option<PathBuf>,
    pub shaders_dir: Option<PathBuf>,
    pub game_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LunarVersionsResponse {
    pub versions: Vec<LunarVersion>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LunarVersion {
    #[serde(default)]
    pub subversions: Vec<LunarSubversion>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LunarSubversion {
    pub id: String,
    pub assets: LunarAssetsMeta,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LunarAssetsMeta {
    pub url: String,
    pub sha1: String,
}

#[derive(Debug, Clone)]
pub struct MetadataState {
    pub subversion_assets: HashMap<String, LunarAssetsMeta>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LaunchRequestBody {
    pub version: String,
    pub branch: String,
    pub args: Vec<String>,
    pub module: String,
    pub canary_preference: String,
    pub profile: LaunchProfile,
    pub installation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overwolf_muid: Option<String>,
    pub os: String,
    pub os_release: String,
    pub arch: String,
    pub launcher_version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LaunchProfile {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modrinth: Option<LaunchProfileModrinth>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LaunchProfileModrinth {
    pub id: String,
    pub version_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LaunchResponse {
    pub success: Option<bool>,
    pub error: Option<LaunchError>,
    #[serde(rename = "launchTypeData")]
    pub launch_type_data: Option<LaunchTypeData>,
    pub jre: Option<JreInfo>,
    #[serde(default)]
    pub licenses: Vec<LicenseInfo>,
    pub textures: Option<IndexedResource>,
    pub ui: Option<UiInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LaunchError {
    pub code: String,
    pub short: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LaunchTypeData {
    #[serde(rename = "mainClass")]
    pub main_class: String,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub url: String,
    pub sha1: String,
    #[serde(rename = "type")]
    pub artifact_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JreInfo {
    #[serde(rename = "folderChecksum")]
    pub folder_checksum: String,
    #[serde(rename = "executablePathInArchive")]
    pub executable_path_in_archive: Vec<String>,
    #[serde(rename = "extraArguments", default)]
    pub extra_arguments: Vec<String>,
    pub download: JreDownload,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JreDownload {
    pub url: String,
    #[serde(rename = "fallbackUrl")]
    pub fallback_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LicenseInfo {
    pub file: String,
    pub url: String,
    pub sha1: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IndexedResource {
    #[serde(rename = "indexUrl")]
    pub index_url: String,
    #[serde(rename = "indexSha1")]
    pub index_sha1: String,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UiInfo {
    #[serde(rename = "sourceUrl")]
    pub source_url: String,
    #[serde(rename = "sourceSha1")]
    pub source_sha1: String,
    pub assets: IndexedResource,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetObject {
    pub hash: String,
}

#[derive(Debug, Clone)]
pub struct LaunchContext {
    pub selected_profile: DiscoveredProfile,
    pub profile_paths: ResolvedProfilePaths,
    pub metadata: MetadataState,
    pub assets_meta: LunarAssetsMeta,
    pub launch_response: LaunchResponse,
    pub java_path: PathBuf,
    pub classpath_entries: Vec<PathBuf>,
    pub launcher_version: String,
    pub subversion: String,
    pub ichor_classpath: Vec<String>,
    pub ichor_external_files: Vec<String>,
    pub installation_id: String,
}
