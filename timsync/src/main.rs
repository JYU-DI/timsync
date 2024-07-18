use std::process::ExitCode;

use anyhow::Result;
use clap::{command, Parser, Subcommand};
use shadow_rs::shadow;
use simplelog::*;
use simplelog::__private::paris::LogIcon;

use commands::InitOptions;

use crate::commands::SyncOpts;

mod commands;
mod processing;
mod project;
mod util;

shadow!(build);

#[derive(Debug, Parser)]
#[command(version = build::CLAP_LONG_VERSION)]
#[command(arg_required_else_help = true)]
#[command(next_line_help = true)]
#[command(propagate_version = true)]
#[command(long_about)]
/// A tool to preprocess and synchronize documents to TIM
///
/// TIMSync is a preprocessor and synchronizer for TIM documents.
/// It allows to upload documents and files to TIM.
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    #[command(name = "init")]
    /// Initialize a new TIMSync project
    Init(InitOptions),

    #[command(name = "sync")]
    /// Synchronize the project with TIM
    Sync(SyncOpts),
    // TODO: target command to modify upload targets
}

#[tokio::main]
async fn main() -> ExitCode {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])
    .unwrap();

    let cli = Cli::parse();
    let cmd_resul: Result<()> = match cli.command {
        Command::Init(opts) => commands::init_repo(opts).await,
        Command::Sync(opts) => commands::sync_target(opts).await,
    };

    match cmd_resul {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            error!("<red>{}</> {:#}", LogIcon::Warning, e);
            ExitCode::FAILURE
        }
    }
}
