//! One-command pipeline: audio -> record -> merge.
//!
//! Each stage is a plain function call in the same process; there is no
//! background queue. A failure stops the pipeline at that stage.

use anyhow::Result;
use std::path::Path;

use crate::audio;
use crate::config::AppConfig;
use crate::merge;
use crate::record;

/// Stages run by `all`, in order. Exposed for tests and docs.
pub fn stages() -> [&'static str; 3] {
    ["audio", "record", "merge"]
}

pub fn run(config: &AppConfig, script_path: &Path) -> Result<()> {
    println!("[all] 1/3 audio");
    audio::run(config, script_path)?;
    println!("[all] 2/3 record");
    record::run(config, script_path)?;
    println!("[all] 3/3 merge");
    merge::run(config, script_path)?;
    println!("[all] complete — final videos in {}/final", config.output_dir);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_precedes_record_and_merge() {
        assert_eq!(stages(), ["audio", "record", "merge"]);
    }
}
