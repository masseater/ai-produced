use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use crate::shared::note_matching::{Selection, score_files};

#[derive(Parser)]
#[command(about = "Score a transcription against a reference MIDI with mir_eval's note matching")]
struct Args {
    reference: PathBuf,
    estimate: PathBuf,
    #[arg(long)]
    ref_instrument: Option<usize>,
    #[arg(long, value_delimiter = ',')]
    est_programs: Option<Vec<u8>>,
    #[arg(long, default_value_t = 0.1)]
    max_shift: f64,
}

pub fn run() -> Result<()> {
    println!("{}", report(&Args::parse())?);
    Ok(())
}

fn report(args: &Args) -> Result<String> {
    let selection = Selection {
        ref_instrument: args.ref_instrument,
        est_programs: args.est_programs.as_deref(),
        max_shift: args.max_shift,
    };
    Ok(score_files(&args.reference, &args.estimate, &selection)?.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::shared::test_melody::write_reference;

    #[test]
    fn is_perfect_against_itself_and_finds_a_shift() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reference.mid");
        write_reference(&path, 0.0);
        let shifted = dir.path().join("shifted.mid");
        write_reference(&shifted, 0.08);
        let args = Args::parse_from([
            "transcription-score",
            path.to_str().unwrap(),
            shifted.to_str().unwrap(),
        ]);
        let out: Value = serde_json::from_str(&report(&args).unwrap()).unwrap();
        assert_eq!(out["f1"], 1.0, "{out}");
        assert_eq!(out["f1_with_offset"], 1.0, "{out}");
        assert_eq!(out["shift_ms"], -100, "{out}");
    }
}
