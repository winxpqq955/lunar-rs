use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use std::fs::File;
use std::path::Path;
use tar::Archive;
use zip::ZipArchive;

pub fn extract_zip(archive_path: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    let file = File::open(archive_path)
        .with_context(|| format!("failed to open {}", archive_path.display()))?;
    let mut archive = ZipArchive::new(file)?;
    archive.extract(destination)?;
    Ok(())
}

pub fn extract_tar_gz(archive_path: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    let file = File::open(archive_path)
        .with_context(|| format!("failed to open {}", archive_path.display()))?;
    let tar = GzDecoder::new(file);
    let mut archive = Archive::new(tar);
    archive.unpack(destination)?;
    Ok(())
}

pub fn extract_archive(archive_path: &Path, destination: &Path) -> Result<()> {
    let file_name = archive_path.file_name().and_then(|x| x.to_str()).unwrap_or_default();
    if file_name.ends_with(".zip") {
        extract_zip(archive_path, destination)
    } else if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
        extract_tar_gz(archive_path, destination)
    } else {
        anyhow::bail!("unsupported archive format: {}", archive_path.display())
    }
}
