use std::time::Duration;

use anyhow::{Context, Result};
use dialoguer::{Input, Password};
use indicatif::ProgressBar;
use simplelog::__private::paris::LogIcon;
use simplelog::{error, info};

use crate::config::{SyncTarget, DEFAULT_SYNC_TARGET_HOST};
use crate::util::tim_client::TimClientBuilder;

/// Create a new sync target by asking the user for details.
pub async fn prompt_user_details_interactive() -> Result<Option<SyncTarget>> {
    loop {
        let host: String = Input::new()
            .with_prompt("TIM host to which to sync the files")
            .default(DEFAULT_SYNC_TARGET_HOST.to_string())
            .interact_text()
            .context("Invalid host given")?;

        let client_builder = TimClientBuilder::new().tim_host(&host);

        let bar = ProgressBar::new_spinner().with_message("Checking host...");
        bar.enable_steady_tick(Duration::from_millis(100));

        let result = client_builder.build().await;

        bar.finish_and_clear();

        if let Err(e) = result {
            error!(
                "Could not connect to TIM host. Please try again. Details: {}",
                e
            );
            continue;
        }

        let client = result.unwrap();

        info!("{} The host was successfully verified.", LogIcon::Tick);

        let username: String = Input::new()
            .with_prompt("Username")
            .interact_text()
            .context("No username given")?;

        let password: String = Password::new()
            .with_prompt("Password")
            .interact()
            .context("No password given")?;

        let bar = ProgressBar::new_spinner().with_message("Verifying account...");
        bar.enable_steady_tick(Duration::from_millis(100));

        let result = client.login_basic(&username, &password).await;

        bar.finish_and_clear();

        if let Err(e) = result {
            error!("Could log in to TIM. Please try again. Details: {}", e);
            continue;
        }

        info!("{} The account was successfully verified.", LogIcon::Tick);

        let folder_root: String = Input::new()
            .with_prompt("Path to TIM folder to which to sync the files")
            .interact_text()
            .context("Invalid folder path given")?;

        return Ok(Some(SyncTarget {
            host,
            folder_root,
            username,
            password,
        }));
    }
}
