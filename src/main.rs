mod launcher;

use anyhow::Result;
use launcher::launch::{launch_profile, list_profiles};
use launcher::profiles::find_profile;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("list") | None => {
            let profiles = list_profiles().await?;
            for profile in profiles {
                println!(
                    "{}\t{:?}\t{}\t{}",
                    profile.profile.id,
                    profile.profile.profile_type,
                    profile.profile.game_version,
                    profile.directory.display()
                );
            }
        }
        Some("launch") => {
            let profile_id = args.next().ok_or_else(|| anyhow::anyhow!("missing profile id"))?;
            let branch = args.next().unwrap_or_else(|| "master".to_string());
            let profiles = list_profiles().await?;
            let profile = find_profile(&profiles, &profile_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("profile not found: {profile_id}"))?;
            launch_profile(profile, &branch).await?;
        }
        Some(other) => {
            anyhow::bail!("unknown command: {other}. use `list` or `launch <profile-id> [branch]`")
        }
    }

    Ok(())
}
