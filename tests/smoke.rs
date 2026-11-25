use screencast_v2::types::{load_script, SegmentTiming};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn version_flag_reports_binary_name() {
    let output = Command::new(env!("CARGO_BIN_EXE_screencast-v2"))
        .arg("--version")
        .output()
        .expect("run screencast-v2 --version");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("screencast-v2"), "unexpected output: {stdout}");
}

#[test]
fn help_lists_planned_stages() {
    let output = Command::new(env!("CARGO_BIN_EXE_screencast-v2"))
        .arg("--help")
        .output()
        .expect("run screencast-v2 --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for stage in ["inspect", "validate", "audio", "record", "merge", "all"] {
        assert!(stdout.contains(stage), "help is missing stage `{stage}`");
    }
}

#[test]
fn fixture_timings_are_monotonic() {
    let raw = fs::read_to_string(fixture("timings.json")).unwrap();
    let timings: Vec<SegmentTiming> = serde_json::from_str(&raw).unwrap();
    assert_eq!(timings.len(), 2);

    for pair in timings.windows(2) {
        let current = &pair[0];
        assert!(current.start_ms <= current.narration_start_ms);
        assert!(current.narration_start_ms <= current.end_ms);
        assert!(current.end_ms <= pair[1].start_ms);
    }
}

#[test]
fn init_writes_parseable_starter_files() {
    let dir = std::env::temp_dir().join(format!("screencast-v2-init-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);

    let output = Command::new(env!("CARGO_BIN_EXE_screencast-v2"))
        .args(["init", "--dir"])
        .arg(&dir)
        .output()
        .expect("run screencast-v2 init");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(dir.join("config/app.config.json").exists());
    let script_path = dir.join("config/script.example.json");
    assert!(script_path.exists());
    let script = load_script(&script_path).unwrap();
    assert!(!script.segments.is_empty());

    let _ = fs::remove_dir_all(&dir);
}
