//! Narration first: synthesize every segment with edge-tts and measure it.
//!
//! Audio is cached next to the MP3s. A cache entry is reused only when the
//! voice and the exact narration text still match; otherwise the clip is
//! regenerated. Durations always come from ffprobe, never from text length.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::AppConfig;
use crate::types::{load_script, AudioClip, AudioManifest};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
struct AudioCache {
    voice: String,
    text: String,
    duration_ms: u64,
}

pub fn run(config: &AppConfig, script_path: &Path) -> Result<()> {
    let script = load_script(script_path)?;
    if script.segments.is_empty() {
        bail!("script {} has no segments", script_path.display());
    }

    let audio_dir = Path::new(&config.output_dir).join("audio");
    fs::create_dir_all(&audio_dir)?;

    let bin = std::env::var("EDGE_TTS_BIN").unwrap_or_else(|_| "uv".into());
    let mut clips = Vec::with_capacity(script.segments.len());

    for segment in &script.segments {
        let name = safe_name(&segment.id);
        let mp3 = audio_dir.join(format!("{name}.mp3"));
        let cache_path = audio_dir.join(format!("{name}.json"));
        let expected = AudioCache {
            voice: config.voice.clone(),
            text: segment.narration.clone(),
            duration_ms: 0,
        };

        let duration_ms = if mp3.is_file() && cache_is_valid(&cache_path, &expected) {
            load_cache(&cache_path)?.duration_ms
        } else {
            println!("[audio] {}", segment.id);
            synthesize(&bin, &config.voice, &segment.narration, &mp3)?;
            let duration_ms = ffprobe_ms(&mp3)?;
            let cache = AudioCache {
                duration_ms,
                ..expected
            };
            fs::write(&cache_path, serde_json::to_string_pretty(&cache)?)?;
            duration_ms
        };

        clips.push(AudioClip {
            segment_id: segment.id.clone(),
            path: mp3.to_string_lossy().into_owned(),
            duration_ms,
        });
    }

    let manifest = AudioManifest {
        script_id: script.id.clone(),
        voice: config.voice.clone(),
        clips,
    };
    let manifest_path = load_manifest_path(Path::new(&config.output_dir), &script.id);
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    println!("[audio] wrote {}", manifest_path.display());
    Ok(())
}

fn cache_is_valid(path: &Path, expected: &AudioCache) -> bool {
    match load_cache(path) {
        Ok(cache) => cache.voice == expected.voice && cache.text == expected.text,
        Err(_) => false,
    }
}

fn load_cache(path: &Path) -> Result<AudioCache> {
    let raw = fs::read_to_string(path)
        .map_err(|error| anyhow!("cannot read audio cache {}: {error}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|error| anyhow!("invalid audio cache {}: {error}", path.display()))
}

fn synthesize(bin: &str, voice: &str, text: &str, output: &Path) -> Result<()> {
    let (program, args) = tts_args(bin, voice, text, output);
    let status = Command::new(&program)
        .args(&args)
        .status()
        .map_err(|error| anyhow!("cannot run {program}: {error}"))?;
    if !status.success() {
        bail!("edge-tts failed for {}", output.display());
    }
    Ok(())
}

/// Build the subprocess argv without a shell. `uv` launches the tool; a direct
/// `EDGE_TTS_BIN` override is used as-is.
pub fn tts_args(bin: &str, voice: &str, text: &str, output: &Path) -> (String, Vec<String>) {
    let mut args: Vec<String> = if bin == "uv" {
        vec!["tool".into(), "run".into(), "edge-tts".into()]
    } else {
        Vec::new()
    };
    args.extend([
        "--voice".into(),
        voice.to_string(),
        "--text".into(),
        text.to_string(),
        "--write-media".into(),
        output.to_string_lossy().into_owned(),
    ]);
    (bin.to_string(), args)
}

fn ffprobe_ms(path: &Path) -> Result<u64> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()
        .map_err(|error| anyhow!("cannot run ffprobe: {error}"))?;
    if !output.status.success() {
        bail!("ffprobe failed for {}", path.display());
    }
    parse_ffprobe_duration(&String::from_utf8_lossy(&output.stdout))
}

pub fn parse_ffprobe_duration(raw: &str) -> Result<u64> {
    let value = raw.trim();
    let seconds: f64 = value
        .parse()
        .map_err(|_| anyhow!("ffprobe returned no duration for audio ({value:?})"))?;
    if !seconds.is_finite() || seconds < 0.0 {
        bail!("ffprobe returned an invalid duration ({value:?})");
    }
    Ok((seconds * 1000.0).round() as u64)
}

pub fn load_manifest_path(output_dir: &Path, script_id: &str) -> PathBuf {
    output_dir
        .join("audio")
        .join(format!("{}-manifest.json", safe_name(script_id)))
}

/// Segment ids become filenames, so keep them inside the audio directory.
pub fn safe_name(raw: &str) -> String {
    let name: String = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() {
        "segment".into()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uv_argv_uses_tool_run() {
        let (bin, args) = tts_args("uv", "en-US-AriaNeural", "Hello", Path::new("a.mp3"));
        assert_eq!(bin, "uv");
        assert_eq!(
            args,
            vec![
                "tool",
                "run",
                "edge-tts",
                "--voice",
                "en-US-AriaNeural",
                "--text",
                "Hello",
                "--write-media",
                "a.mp3"
            ]
        );
    }

    #[test]
    fn direct_tts_binary_has_no_uv_prefix() {
        let (_, args) = tts_args("edge-tts", "v", "t", Path::new("x.mp3"));
        assert_eq!(args[0], "--voice");
    }

    #[test]
    fn parses_ffprobe_seconds_to_milliseconds() {
        assert_eq!(parse_ffprobe_duration("1.2345\n").unwrap(), 1235);
        assert!(parse_ffprobe_duration("N/A").is_err());
    }

    #[test]
    fn safe_name_never_escapes_the_audio_dir() {
        assert_eq!(safe_name("intro"), "intro");
        assert_eq!(safe_name("../etc/passwd"), ".._etc_passwd");
    }

    #[test]
    fn cache_is_reused_only_for_same_voice_and_text() {
        let path = std::env::temp_dir().join(format!("screencast-audio-cache-{}.json", std::process::id()));
        let cache = AudioCache {
            voice: "v1".into(),
            text: "hello".into(),
            duration_ms: 100,
        };
        fs::write(&path, serde_json::to_string(&cache).unwrap()).unwrap();

        assert!(cache_is_valid(&path, &cache));
        let changed = AudioCache { voice: "v2".into(), ..cache.clone() };
        assert!(!cache_is_valid(&path, &changed));
        let changed = AudioCache { text: "goodbye".into(), ..cache };
        assert!(!cache_is_valid(&path, &changed));
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn cache_path_helper_is_testable() {
        let audio_dir = PathBuf::from("output/audio");
        assert_eq!(audio_dir.join(format!("{}.mp3", safe_name("a/b"))), audio_dir.join("a_b.mp3"));
    }
}
