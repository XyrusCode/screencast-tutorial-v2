use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// A semantic locator handed to `agent-browser find`. `selector` remains
/// available alongside it for plain CSS; `target` wins when both are present.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Css(String),
    Role {
        role: String,
        #[serde(default)]
        name: Option<String>,
    },
    Text(String),
    TextExact(String),
    Label(String),
    Placeholder(String),
    Testid(String),
    Ref(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Navigate {
        url: String,
    },
    Click {
        selector: String,
        #[serde(default)]
        target: Option<Target>,
    },
    Fill {
        selector: String,
        value: String,
        #[serde(default)]
        target: Option<Target>,
    },
    Select {
        selector: String,
        value: String,
        #[serde(default)]
        target: Option<Target>,
    },
    Wait {
        ms: u64,
    },
    Scroll {
        #[serde(default)]
        selector: Option<String>,
        #[serde(default)]
        y: i64,
    },
    Hover {
        selector: String,
        #[serde(default)]
        target: Option<Target>,
    },
    Press {
        key: String,
    },
    WaitForSelector {
        selector: String,
        #[serde(default)]
        target: Option<Target>,
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    Note {
        text: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Segment {
    pub id: String,
    pub narration: String,
    pub pre_actions: Vec<Action>,
    pub actions: Vec<Action>,
    pub hold_ms: Option<u64>,
    pub ready_selector: Option<String>,
    pub ready_target: Option<Target>,
}

impl Default for Segment {
    fn default() -> Self {
        Self {
            id: String::new(),
            narration: String::new(),
            pre_actions: Vec::new(),
            actions: Vec::new(),
            hold_ms: None,
            ready_selector: None,
            ready_target: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Script {
    pub id: String,
    pub title: String,
    pub segments: Vec<Segment>,
}

impl Default for Script {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            segments: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct SegmentTiming {
    pub segment_id: String,
    pub start_ms: u64,
    pub narration_start_ms: u64,
    pub end_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct AudioClip {
    pub segment_id: String,
    /// Path to the cached MP3, relative to the project root.
    pub path: String,
    /// Measured by ffprobe, never guessed from the text length.
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct AudioManifest {
    pub script_id: String,
    pub voice: String,
    pub clips: Vec<AudioClip>,
}

pub fn load_audio_manifest(path: &Path) -> Result<AudioManifest> {
    let raw = fs::read_to_string(path)
        .map_err(|e| anyhow!("cannot read audio manifest {}: {e}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|e| anyhow!("invalid audio manifest {}: {e}", path.display()))
}

pub fn load_timings(path: &Path) -> Result<Vec<SegmentTiming>> {
    let raw = fs::read_to_string(path)
        .map_err(|e| anyhow!("cannot read timings {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| anyhow!("invalid timings {}: {e}", path.display()))
}

pub fn load_script(path: &Path) -> Result<Script> {
    let raw = fs::read_to_string(path)
        .map_err(|e| anyhow!("cannot read script {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| anyhow!("invalid script {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixture_script_with_hybrid_target() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/script.json");
        let script = load_script(&path).unwrap();
        assert_eq!(script.id, "demo-tour");
        assert_eq!(script.segments.len(), 2);

        match &script.segments[0].actions[0] {
            Action::Click {
                target: Some(Target::Role { name: Some(name), .. }),
                ..
            } => assert_eq!(name, "New"),
            other => panic!("unexpected first action: {other:?}"),
        }
        assert_eq!(script.segments[1].ready_target, Some(Target::Text("Project created".into())));
    }
}
