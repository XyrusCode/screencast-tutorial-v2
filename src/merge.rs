//! Cut the continuous take per timing entry and mux measured narration.
//!
//! This mirrors the v1 math: audio-first, capped silent lead-in, frozen tail
//! padding, loudnorm, and a full re-encode. No time-stretching anywhere.

use anyhow::{anyhow, bail, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::audio::{load_manifest_path, safe_name};
use crate::config::AppConfig;
use crate::types::{load_script, load_audio_manifest, load_timings, SegmentTiming};

const MAX_LEAD_MS: u64 = 2500;
const TAIL_MS: u64 = 1200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePlan {
    pub start_ms: u64,
    pub end_ms: u64,
    pub lead_ms: u64,
    pub target_ms: u64,
    pub pad_ms: u64,
}

/// Derive cut points from a timing marker and the real audio duration.
pub fn plan(timing: &SegmentTiming, audio_ms: u64, max_lead_ms: u64, tail_ms: u64) -> MergePlan {
    let raw_lead = timing.narration_start_ms.saturating_sub(timing.start_ms);
    let lead_ms = raw_lead.min(max_lead_ms);
    let start_ms = timing.narration_start_ms - lead_ms;
    let end_ms = timing
        .end_ms
        .min(timing.narration_start_ms + audio_ms + tail_ms);
    let target_ms = lead_ms + audio_ms + tail_ms;
    let cut_ms = end_ms.saturating_sub(start_ms);
    let pad_ms = target_ms.saturating_sub(cut_ms);
    MergePlan {
        start_ms,
        end_ms,
        lead_ms,
        target_ms,
        pad_ms,
    }
}

pub fn sec(ms: u64) -> String {
    format!("{:.3}", ms as f64 / 1000.0)
}

pub fn ffmpeg_args(
    video: &Path,
    audio: &Path,
    output: &Path,
    cut: &MergePlan,
) -> Vec<String> {
    let filter = format!(
        "[0:v]tpad=stop_mode=clone:stop_duration={},fps=30,format=yuv420p[v];\
         [1:a]adelay={}:all=1,apad,loudnorm=I=-16:TP=-1.5:LRA=11[a]",
        sec(cut.pad_ms),
        cut.lead_ms
    );
    vec![
        "-y".into(),
        "-ss".into(),
        sec(cut.start_ms),
        "-to".into(),
        sec(cut.end_ms),
        "-i".into(),
        video.to_string_lossy().into_owned(),
        "-i".into(),
        audio.to_string_lossy().into_owned(),
        "-filter_complex".into(),
        filter,
        "-map".into(),
        "[v]".into(),
        "-map".into(),
        "[a]".into(),
        "-t".into(),
        sec(cut.target_ms),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "fast".into(),
        "-crf".into(),
        "23".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "128k".into(),
        "-ar".into(),
        "44100".into(),
        output.to_string_lossy().into_owned(),
    ]
}

pub fn concat_args(list: &Path, output: &Path) -> Vec<String> {
    vec![
        "-y".into(),
        "-f".into(),
        "concat".into(),
        "-safe".into(),
        "0".into(),
        "-i".into(),
        list.to_string_lossy().into_owned(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "fast".into(),
        "-crf".into(),
        "22".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "128k".into(),
        output.to_string_lossy().into_owned(),
    ]
}

pub fn concat_line(path: &Path) -> String {
    let escaped = path.to_string_lossy().replace('\'', "'\\''");
    format!("file '{escaped}'")
}

pub fn timings_path(output_dir: &Path, script_id: &str) -> PathBuf {
    output_dir
        .join("raw")
        .join(format!("{}-timings.json", safe_name(script_id)))
}

pub fn raw_video_path(output_dir: &Path, script_id: &str) -> PathBuf {
    output_dir.join("raw").join(format!("{}.webm", safe_name(script_id)))
}

pub fn run(config: &AppConfig, script_path: &Path) -> Result<()> {
    let script = load_script(script_path)?;
    let output_root = Path::new(&config.output_dir);
    let manifest = load_audio_manifest(&load_manifest_path(output_root, &script.id))?;
    let timings = load_timings(&timings_path(output_root, &script.id))?;
    let video_path = raw_video_path(output_root, &script.id);
    if !video_path.is_file() {
        bail!("no recorded take at {} — run `record` first", video_path.display());
    }

    let segments_dir = output_root.join("segments");
    let final_dir = output_root.join("final");
    fs::create_dir_all(&segments_dir)?;
    fs::create_dir_all(&final_dir)?;

    let mut merged = Vec::new();
    for timing in &timings {
        let clip = match manifest.clips.iter().find(|clip| clip.segment_id == timing.segment_id) {
            Some(clip) => clip,
            None => {
                println!("[merge] skip {}: no audio clip", timing.segment_id);
                continue;
            }
        };
        if clip.duration_ms == 0 {
            println!("[merge] skip {}: zero-length audio", timing.segment_id);
            continue;
        }

        let cut = plan(timing, clip.duration_ms, MAX_LEAD_MS, TAIL_MS);
        if cut.end_ms <= cut.start_ms {
            println!("[merge] skip {}: empty cut", timing.segment_id);
            continue;
        }

        let name = format!(
            "{}__{}",
            safe_name(&script.id),
            safe_name(&timing.segment_id)
        );
        let output = segments_dir.join(format!("{name}.mp4"));
        let args = ffmpeg_args(&video_path, Path::new(&clip.path), &output, &cut);
        run_ffmpeg(&args)?;
        println!(
            "[merge] {} lead={}ms pad={}ms",
            timing.segment_id, cut.lead_ms, cut.pad_ms
        );
        merged.push(output);
    }

    if merged.is_empty() {
        bail!("no segments merged — nothing to concatenate");
    }

    let list_path = segments_dir.join(format!("{}-concat.txt", safe_name(&script.id)));
    let lines: Vec<String> = merged.iter().map(|path| concat_line(path)).collect();
    fs::write(&list_path, format!("{}\n", lines.join("\n")))?;

    let final_path = final_dir.join(format!("{}.mp4", safe_name(&script.id)));
    run_ffmpeg(&concat_args(&list_path, &final_path))?;
    println!("[merge] wrote {}", final_path.display());
    Ok(())
}

fn run_ffmpeg(args: &[String]) -> Result<()> {
    let output = Command::new("ffmpeg")
        .args(args)
        .output()
        .map_err(|error| anyhow!("cannot run ffmpeg: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let tail: Vec<&str> = stderr.lines().rev().take(12).collect();
    bail!("ffmpeg failed:\n{}", tail.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timing() -> SegmentTiming {
        SegmentTiming {
            segment_id: "intro".into(),
            start_ms: 1000,
            narration_start_ms: 4500,
            end_ms: 9000,
        }
    }

    #[test]
    fn caps_the_silent_lead() {
        let cut = plan(&timing(), 2000, MAX_LEAD_MS, TAIL_MS);
        assert_eq!(cut.lead_ms, MAX_LEAD_MS);
        assert_eq!(cut.start_ms, 2000);
        assert_eq!(cut.target_ms, MAX_LEAD_MS + 2000 + TAIL_MS);
        assert_eq!(cut.pad_ms, cut.target_ms - (cut.end_ms - cut.start_ms));
    }

    #[test]
    fn never_time_stretches_and_formats_seconds() {
        assert_eq!(sec(2500), "2.500");
        assert_eq!(sec(0), "0.000");
    }

    #[test]
    fn ffmpeg_args_reencode_and_pad() {
        let cut = plan(&timing(), 2000, MAX_LEAD_MS, TAIL_MS);
        let args = ffmpeg_args(
            Path::new("raw.webm"),
            Path::new("voice.mp3"),
            Path::new("out.mp4"),
            &cut,
        );
        let filter = args.iter().find(|value| value.contains("tpad")).unwrap();
        assert!(filter.contains("stop_mode=clone"));
        assert!(filter.contains("adelay=2500:all=1"));
        assert!(args.windows(2).any(|pair| pair == ["-c:v", "libx264"]));
        assert!(!args.iter().any(|value| value == "copy"));
    }

    #[test]
    fn concat_lines_escape_quotes() {
        assert_eq!(
            concat_line(Path::new("it's.mp4")),
            "file 'it'\\''s.mp4'"
        );
    }
}
