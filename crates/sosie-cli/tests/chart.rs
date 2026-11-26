use sosie_cli::output::{OutputError, Targets};
use sosie_core::{chart, encode_png, rasterize, Chart, Fonts, Metric, Theme};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const METRICS: &str = r#"{
  "title": "Performance",
  "metrics": [
    { "name": "First Contentful Paint", "unit": "ms", "before": 1136, "after": 588, "lowerIsBetter": true },
    { "name": "JavaScript bundle", "unit": "kB", "before": 292.5, "after": 297.2, "lowerIsBetter": true }
  ]
}"#;

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("cli-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("metrics.json"), METRICS).expect("metrics.json");
    dir
}

fn sosie(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sosie"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run sosie")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn names(dir: &Path) -> BTreeSet<String> {
    fs::read_dir(dir)
        .expect("read dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| n.to_string()).collect()
}

fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
}

#[test]
fn chart_replaces_both_files_with_webp_and_prints_their_paths() {
    let dir = scratch("webp");
    fs::write(dir.join("bars-light.webp"), "old light").unwrap();
    fs::write(dir.join("bars-dark.webp"), "old dark").unwrap();
    let out = sosie(&dir, &["chart", "metrics.json", "bars"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "bars-light.webp\nbars-dark.webp\n"
    );
    assert!(is_webp(&fs::read(dir.join("bars-light.webp")).unwrap()));
    assert!(is_webp(&fs::read(dir.join("bars-dark.webp")).unwrap()));
    assert_eq!(
        names(&dir),
        set(&["metrics.json", "bars-light.webp", "bars-dark.webp"])
    );
}

#[test]
fn a_prefix_with_spaces_names_the_files() {
    let dir = scratch("spaces");
    fs::create_dir(dir.join("out dir")).unwrap();
    let out = sosie(&dir, &["chart", "metrics.json", "out dir/my bars"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        names(&dir.join("out dir")),
        set(&["my bars-light.webp", "my bars-dark.webp"])
    );
}

#[test]
fn png_output_equals_core_encode_png_at_978_px() {
    let dir = scratch("png");
    let out = sosie(&dir, &["chart", "--format", "png", "metrics.json", "bars"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let fonts = Fonts::new();
    let metric = |name: &str, unit: &str, before, after| Metric {
        name: name.to_string(),
        unit: unit.to_string(),
        before,
        after,
        lower_is_better: true,
    };
    let metrics = vec![
        metric("First Contentful Paint", "ms", 1136.0, 588.0),
        metric("JavaScript bundle", "kB", 292.5, 297.2),
    ];
    let chart = Chart::new(Some("Performance".to_string()), metrics).unwrap();
    for (theme, file) in [
        (Theme::Light, "bars-light.png"),
        (Theme::Dark, "bars-dark.png"),
    ] {
        let list = chart::render(&fonts, &chart, theme, 978).unwrap();
        let pixmap = rasterize(&fonts, &list, 2.0, &Default::default()).unwrap();
        let bytes = fs::read(dir.join(file)).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "{file}");
        assert!(bytes == encode_png(&pixmap), "{file}: bytes differ");
    }
}

#[test]
fn a_malformed_metric_exits_2_naming_its_index() {
    let dir = scratch("malformed");
    fs::write(
        dir.join("bad.json"),
        r#"{ "metrics": [
            { "name": "a", "unit": "", "before": 1, "after": 2, "lowerIsBetter": true },
            { "name": "b", "unit": "", "before": 1, "after": 2 }
        ] }"#,
    )
    .unwrap();
    let out = sosie(&dir, &["chart", "bad.json", "bars"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).starts_with("metrics.json: metrics[1] needs a name"),
        "{}",
        stderr(&out)
    );
    assert_eq!(names(&dir), set(&["metrics.json", "bad.json"]));
}

#[test]
fn usage_and_chart_errors_exit_2() {
    let dir = scratch("usage");
    for args in [
        &[][..],
        &["chart", "metrics.json"],
        &["chart", "metrics.json", "bars", "--format", "gif"],
        &["chart", "metrics.json", "bars", "--width", "wide"],
        &["chart", "metrics.json", "bars", "--width", "309"],
    ] {
        let out = sosie(&dir, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", stderr(&out));
    }
    assert_eq!(names(&dir), set(&["metrics.json"]));
}

#[test]
fn help_names_the_receivers_that_need_png() {
    let out = sosie(&scratch("help"), &["--help"]);
    assert_eq!(out.status.code(), Some(0));
    let help = String::from_utf8_lossy(&out.stdout);
    for receiver in [
        "Slack image blocks",
        "Outlook for Windows",
        "LinkedIn and Facebook og:image",
        "Gmail (converts WebP to JPG)",
        "desk albums",
    ] {
        assert!(help.contains(receiver), "{receiver}");
    }
}

#[test]
fn a_directory_at_the_dark_path_fails_before_any_change() {
    let dir = scratch("directory");
    fs::write(dir.join("bars-light.webp"), "old light").unwrap();
    fs::create_dir(dir.join("bars-dark.webp")).unwrap();
    fs::write(dir.join("bars-dark.webp/keep"), "kept").unwrap();
    let out = sosie(&dir, &["chart", "metrics.json", "bars"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        stderr(&out),
        "bars-dark.webp: exists and is not a regular file\n"
    );
    assert_eq!(
        fs::read_to_string(dir.join("bars-light.webp")).unwrap(),
        "old light"
    );
    assert_eq!(
        fs::read_to_string(dir.join("bars-dark.webp/keep")).unwrap(),
        "kept"
    );
    assert_eq!(
        names(&dir),
        set(&["metrics.json", "bars-light.webp", "bars-dark.webp"])
    );
}

#[cfg(unix)]
#[test]
fn a_symlink_at_the_light_path_fails_before_any_change() {
    let dir = scratch("symlink");
    fs::write(dir.join("target"), "target").unwrap();
    std::os::unix::fs::symlink("target", dir.join("bars-light.webp")).unwrap();
    let out = sosie(&dir, &["chart", "metrics.json", "bars"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        stderr(&out),
        "bars-light.webp: exists and is not a regular file\n"
    );
    assert!(fs::symlink_metadata(dir.join("bars-light.webp"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_to_string(dir.join("target")).unwrap(), "target");
    assert_eq!(
        names(&dir),
        set(&["metrics.json", "target", "bars-light.webp"])
    );
}

fn failing_once(at: PathBuf) -> impl FnMut(&Path, &Path) -> io::Result<()> {
    let mut failed = false;
    move |from, to| {
        if !failed && to == at {
            failed = true;
            return Err(io::Error::other("injected"));
        }
        fs::rename(from, to)
    }
}

fn pair(dir: &Path) -> [PathBuf; 2] {
    [dir.join("a-light.webp"), dir.join("a-dark.webp")]
}

#[test]
fn a_failure_after_the_first_rename_restores_both_old_files() {
    let dir = scratch("rollback");
    let [light, dark] = pair(&dir);
    fs::write(&light, "old light").unwrap();
    fs::write(&dark, "old dark").unwrap();
    let targets = Targets::check(pair(&dir)).unwrap();
    let result = targets.write([b"new light", b"new dark"], failing_once(dark.clone()));
    assert!(
        matches!(&result, Err(OutputError::Io { path, .. }) if *path == dark),
        "{result:?}"
    );
    assert_eq!(fs::read_to_string(&light).unwrap(), "old light");
    assert_eq!(fs::read_to_string(&dark).unwrap(), "old dark");
    assert_eq!(
        names(&dir),
        set(&["metrics.json", "a-light.webp", "a-dark.webp"])
    );
}

#[test]
fn a_failure_after_the_first_rename_removes_the_new_file() {
    let dir = scratch("rollback-new");
    let [_, dark] = pair(&dir);
    let targets = Targets::check(pair(&dir)).unwrap();
    let result = targets.write([b"new light", b"new dark"], failing_once(dark));
    assert!(matches!(result, Err(OutputError::Io { .. })), "{result:?}");
    assert_eq!(names(&dir), set(&["metrics.json"]));
}

#[test]
fn a_failed_restore_is_reported_as_a_rollback_failure() {
    let dir = scratch("rollback-failed");
    let [light, dark] = pair(&dir);
    fs::write(&light, "old light").unwrap();
    fs::write(&dark, "old dark").unwrap();
    let targets = Targets::check(pair(&dir)).unwrap();
    let to_dark = dark.clone();
    let result = targets.write(
        [b"new light", b"new dark"],
        move |from: &Path, to: &Path| {
            if to == to_dark {
                return Err(io::Error::other("injected"));
            }
            fs::rename(from, to)
        },
    );
    let Err(OutputError::Rollback { path, failed, .. }) = result else {
        panic!("{result:?}");
    };
    assert_eq!(path, dark);
    assert_eq!(failed.len(), 1);
    assert_eq!(fs::read_to_string(&light).unwrap(), "old light");
}

fn rename(from: &Path, to: &Path) -> io::Result<()> {
    fs::rename(from, to)
}

#[test]
fn files_at_the_temp_and_backup_names_survive_a_write() {
    let dir = scratch("reserved");
    let [light, dark] = pair(&dir);
    fs::write(&light, "old light").unwrap();
    fs::write(&dark, "old dark").unwrap();
    let pid = std::process::id();
    let planted = [
        format!(".a-light.webp.sosie-{pid}-0.bak"),
        format!(".a-light.webp.sosie-{pid}-0.tmp"),
    ];
    for name in &planted {
        fs::write(dir.join(name), name).unwrap();
    }
    let targets = Targets::check(pair(&dir)).unwrap();
    let stale = targets.write([b"new light", b"new dark"], rename).unwrap();
    assert!(stale.is_empty(), "{stale:?}");
    assert_eq!(fs::read_to_string(&light).unwrap(), "new light");
    assert_eq!(fs::read_to_string(&dark).unwrap(), "new dark");
    for name in &planted {
        assert_eq!(&fs::read_to_string(dir.join(name)).unwrap(), name);
    }
    let mut expected = set(&["metrics.json", "a-light.webp", "a-dark.webp"]);
    expected.extend(planted);
    assert_eq!(names(&dir), expected);
}

#[cfg(unix)]
#[test]
fn a_backup_that_cannot_be_removed_keeps_the_new_files_and_is_reported() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("stale").join("out");
    fs::create_dir(&dir).unwrap();
    let [light, dark] = pair(&dir);
    fs::write(&light, "old light").unwrap();
    fs::write(&dark, "old dark").unwrap();
    let targets = Targets::check(pair(&dir)).unwrap();
    let (last, locked) = (dark.clone(), dir.clone());
    let result = targets.write(
        [b"new light", b"new dark"],
        move |from: &Path, to: &Path| {
            fs::rename(from, to)?;
            if to == last {
                fs::set_permissions(&locked, fs::Permissions::from_mode(0o500))?;
            }
            Ok(())
        },
    );
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    let stale = result.unwrap();
    assert_eq!(fs::read_to_string(&light).unwrap(), "new light");
    assert_eq!(fs::read_to_string(&dark).unwrap(), "new dark");
    let mut kept: Vec<String> = stale
        .iter()
        .map(|s| fs::read_to_string(&s.path).unwrap())
        .collect();
    kept.sort();
    assert_eq!(kept, ["old dark", "old light"]);
}
