use anyhow::{Context, Result};
use sha1::{Digest, Sha1};
use std::fs;
use std::path::Path;

pub async fn download_to_file(client: &reqwest::Client, url: &str, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    println!(
        "[launcher] download start url={} destination={}",
        url,
        destination.display()
    );
    let bytes = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("request failed: {url}"))?
        .error_for_status()
        .with_context(|| format!("bad status: {url}"))?
        .bytes()
        .await
        .with_context(|| format!("failed to read body: {url}"))?;

    tokio::fs::write(destination, &bytes).await?;
    println!(
        "[launcher] download done url={} bytes={} destination={}",
        url,
        bytes.len(),
        destination.display()
    );
    Ok(())
}

pub fn sha1_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

pub async fn ensure_downloaded(
    client: &reqwest::Client,
    url: &str,
    destination: &Path,
    expected_sha1: Option<&str>,
) -> Result<()> {
    if destination.exists() {
        if let Some(expected_sha1) = expected_sha1 {
            if sha1_file(destination).ok().as_deref() == Some(expected_sha1) {
                println!(
                    "[launcher] download cache-hit destination={} sha1={}",
                    destination.display(),
                    expected_sha1
                );
                return Ok(());
            }
            println!(
                "[launcher] download cache-miss destination={} reason=sha1-mismatch",
                destination.display()
            );
        } else {
            println!(
                "[launcher] download cache-hit destination={} reason=exists",
                destination.display()
            );
            return Ok(());
        }
    }

    download_to_file(client, url, destination).await?;
    Ok(())
}
