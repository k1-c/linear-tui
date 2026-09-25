//! `linear-tui paths`: where linear-tui keeps its files, for scripts such
//! as the herdr plugin that read them.

use anyhow::Result;
use serde_json::json;

use super::args::Args;
use crate::config::Config;
use crate::infra::disk::snapshot;

pub fn run(args: &[String]) -> Result<()> {
    let args = Args::parse(args, &["json"], &[])?;
    args.positionals::<0>("linear-tui paths [--json]")?;
    let config = Config::config_dir()?;
    let state = snapshot::state_dir()?;
    if args.flag("json") {
        let value = json!({ "config": config, "state": state });
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("config  {}", config.display());
        println!("state   {}", state.display());
    }
    Ok(())
}
