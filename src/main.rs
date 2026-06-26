// SPDX-License-Identifier: AGPL-3.0-only

use clap::Parser;

use ragdoll::cli::{run, Cli};
use ragdoll::telemetry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    telemetry::init();
    run(Cli::parse()).await
}
