use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn song() -> PathBuf {
    repo().join("songs/001-mou-ikkai")
}

fn run(bin: &str, args: &[&Path]) {
    let status = Command::new(bin).args(args).status().expect(bin);
    assert!(status.success(), "{bin} failed");
}

fn assert_same_tree(actual: &Path, expected: &Path) {
    for entry in fs::read_dir(expected).expect("expected dir") {
        let entry = entry.expect("entry");
        let path = entry.path();
        let other = actual.join(entry.file_name());
        if path.is_dir() {
            assert_same_tree(&other, &path);
        } else {
            assert!(
                fs::read(&other).expect("generated file") == fs::read(&path).expect("committed"),
                "{} differs from {}",
                other.display(),
                path.display()
            );
        }
    }
}

#[test]
fn score_midi_reproduces_the_committed_composition() {
    let out = tempfile::tempdir().expect("tempdir");
    let composition = song().join("02-composition");
    run(
        env!("CARGO_BIN_EXE_score-midi"),
        &[
            &composition.join("score.toml"),
            Path::new("--out"),
            out.path(),
        ],
    );
    assert_same_tree(out.path(), &composition.join("generated"));
}

#[test]
fn lyric_timing_reproduces_the_video_timing() {
    let out = tempfile::tempdir().expect("tempdir");
    let json = out.path().join("timing.json");
    let composition = song().join("02-composition");
    run(
        env!("CARGO_BIN_EXE_lyric-timing"),
        &[
            &composition.join("score.toml"),
            &composition.join("generated/vocal.tsv"),
            &json,
        ],
    );
    let committed = repo().join("apps/video/src/pages/mou-ikkai/generated/mou-ikkai.json");
    assert_eq!(
        fs::read(json).expect("json"),
        fs::read(committed).expect("committed")
    );
}

#[test]
fn midi_rpp_reproduces_the_reaper_project() {
    let out = tempfile::tempdir().expect("tempdir");
    let rpp = out.path().join("mou-ikkai.rpp");
    run(
        env!("CARGO_BIN_EXE_midi-rpp"),
        &[
            &song().join("02-composition/generated/full.mid"),
            Path::new("--out"),
            &rpp,
            Path::new("--audio"),
            Path::new("Vocal=../03-vocal/render/mou-ikkai_Vocal.wav"),
        ],
    );
    let committed = song().join("04-reaper/mou-ikkai.rpp");
    assert_eq!(
        fs::read(rpp).expect("rpp"),
        fs::read(committed).expect("committed")
    );
}

#[test]
fn ustx_from_vocal_reproduces_the_untuned_project() {
    let out = tempfile::tempdir().expect("tempdir");
    let ustx = out.path().join("mou-ikkai.ustx");
    run(
        env!("CARGO_BIN_EXE_ustx-from-vocal"),
        &[
            &song().join("02-composition/generated/vocal.tsv"),
            Path::new("--score"),
            &song().join("02-composition/score.toml"),
            Path::new("--out"),
            &ustx,
        ],
    );
    let parse = |path: &Path| -> serde_json::Value {
        serde_saphyr::from_str(&fs::read_to_string(path).expect("ustx")).expect("yaml")
    };
    assert_eq!(parse(&ustx), parse(&song().join("03-vocal/mou-ikkai.ustx")));
}
