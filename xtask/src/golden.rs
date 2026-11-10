//! The goldens' comparison job.
//!
//! - `cargo xtask golden-download`: the recordings of both commits, from the runs' artifacts,
//!   through gh (with `GH_TOKEN`): the head's from this run (`RUN`) into golden/head, and the
//!   base's from this run or from an earlier one whose recordings are reused (`BASE_RUN`) into
//!   golden/base.

use std::process::Command;

use crate::ci;

/// `cargo xtask golden-download`.
pub fn download(arguments: &[String]) -> Result<u8, String> {
    if !arguments.is_empty() {
        return Err("usage: cargo xtask golden-download".to_owned());
    }
    let run = ci::setting("RUN");
    let base_run = ci::setting("BASE_RUN");
    let base_run = if base_run.is_empty() { &run } else { &base_run };
    let legs = [
        (&run, "golden-head-*", "golden/head"),
        (base_run, "golden-base-*", "golden/base"),
    ];
    for (from, pattern, folder) in legs {
        let mut command = Command::new("gh");
        command
            .args(["run", "download", from])
            .args(["--pattern", pattern, "--dir", folder]);
        ci::must(&mut command)?;
    }
    Ok(0)
}
