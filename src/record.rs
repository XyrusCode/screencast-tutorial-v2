//! Record one continuous browser take and emit audio-sized timing markers.
//!
//! The app is authenticated and settled before `record start`. Markers are
//! wall-clock offsets from that instant, so `merge` can cut the take without
//! ever time-stretching narration or video.

use anyhow::{bail, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::audio::{load_manifest_path, safe_name};
use crate::auth;
use crate::browser::Browser;
use crate::config::AppConfig;
use crate::types::{load_audio_manifest, load_script, SegmentTiming};

const TAIL_MS: u64 = 1200;
const MIN_HOLD_MS: u64 = 3500;

pub fn run(config: &AppConfig, script_path: &Path) -> Result<()> {
    let script = load_script(script_path)?;
    if script.segments.is_empty() {
        bail!("script {} has no segments", script_path.display());
    }

    let manifest = load_audio_manifest(&load_manifest_path(
        Path::new(&config.output_dir),
        &script.id,
    ))?;
    let durations: HashMap<String, u64> = manifest
        .clips
        .iter()
        .map(|clip| (clip.segment_id.clone(), clip.duration_ms))
        .collect();
    for segment in &script.segments {
        match durations.get(&segment.id) {
            Some(ms) if *ms > 0 => {}
            _ => bail!(
                "no narration audio for {} — run `audio` first",
                segment.id
            ),
        }
    }

    let raw_dir = Path::new(&config.output_dir).join("raw");
    fs::create_dir_all(&raw_dir)?;
    let video_path = raw_dir.join(format!("{}.webm", safe_name(&script.id)));

    let browser = Browser::new("record");
    browser.set_viewport(config.viewport.width, config.viewport.height)?;
    let init_script = auth::prepare(&browser, config, Path::new(&config.output_dir))?;
    browser.open(Some(&config.base_url), init_script.as_deref())?;

    // Warm-up before recording: hydration must not replay on camera.
    crate::validate::settle(&browser, config, None)?;
    browser.wait_ms(1500)?;

    let mut timings = Vec::with_capacity(script.segments.len());
    let mut failures = Vec::new();

    let walk = (|| -> Result<()> {
        browser.record_start(&video_path, true)?;
        let record_started = Instant::now();
        let mut floor = 0;

        for segment in &script.segments {
            for action in &segment.pre_actions {
                if let Err(error) = browser.run_action(action, &config.base_url) {
                    failures.push(format!("{} pre_action {action:?}: {error}", segment.id));
                }
            }
            if let Err(error) = crate::validate::settle(&browser, config, Some(segment)) {
                failures.push(format!("{} gate: {error}", segment.id));
            }

            let start_ms = elapsed_ms(record_started, floor);
            floor = start_ms;

            for action in &segment.actions {
                if let Err(error) = browser.run_action(action, &config.base_url) {
                    failures.push(format!("{} action {action:?}: {error}", segment.id));
                }
            }
            if let Err(error) = crate::validate::settle(&browser, config, Some(segment)) {
                failures.push(format!("{} ready gate: {error}", segment.id));
            }

            let narration_start_ms = elapsed_ms(record_started, floor);
            floor = narration_start_ms;

            let audio_ms = durations[&segment.id];
            let hold = hold_ms(audio_ms, segment.hold_ms.unwrap_or(0));
            browser.wait_ms(hold)?;

            let end_ms = elapsed_ms(record_started, floor);
            floor = end_ms;
            timings.push(SegmentTiming {
                segment_id: segment.id.clone(),
                start_ms,
                narration_start_ms,
                end_ms,
            });
        }
        Ok(())
    })();

    // Always stop the take, even if a gate or action failed.
    let stopped = browser.record_stop();
    walk?;
    stopped?;

    if !failures.is_empty() {
        bail!(
            "{} recording gate(s) failed:\n- {}",
            failures.len(),
            failures.join("\n- ")
        );
    }

    let timings_path = timings_path(&raw_dir, &script.id);
    if let Some(parent) = timings_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&timings_path, serde_json::to_string_pretty(&timings)?)?;
    println!("[record] video {}", video_path.display());
    println!("[record] timings {}", timings_path.display());
    Ok(())
}

/// Derive the on-screen hold from audio, never the reverse.
pub fn hold_ms(audio_ms: u64, extra_ms: u64) -> u64 {
    (audio_ms + TAIL_MS + extra_ms).max(MIN_HOLD_MS)
}

pub fn timings_path(raw_dir: &Path, script_id: &str) -> PathBuf {
    raw_dir.join(format!("{}-timings.json", safe_name(script_id)))
}

fn elapsed_ms(started: Instant, floor: u64) -> u64 {
    started.elapsed().as_millis().max(floor as u128) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_follows_audio_duration() {
        assert_eq!(hold_ms(6000, 0), 7200);
        assert_eq!(hold_ms(1000, 0), MIN_HOLD_MS);
        assert_eq!(hold_ms(1000, 900), MIN_HOLD_MS);
    }

    #[test]
    fn timings_are_named_after_the_script() {
        let path = timings_path(Path::new("output/raw"), "demo tour");
        assert_eq!(path, PathBuf::from("output/raw/demo_tour-timings.json"));
    }
}
