use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use song::{moras, vowel};

const VOICELESS: &str = "かきくけこさしすせそたちつてとはひふへほぱぴぷぺぽ";

#[derive(Parser)]
#[command(
    about = "Print a Markdown table of morae, vowels and devoicing-prone morae per lyric line"
)]
struct Args {
    lyrics: PathBuf,
}

fn starts_voiceless(mora: &str) -> bool {
    mora.chars().next().is_some_and(|c| VOICELESS.contains(c))
}

fn row(kanji: &str, kana: &str) -> String {
    let ms = moras(kana);
    let vowels: Vec<char> = ms.iter().map(|m| vowel(m)).collect();
    let devoiced: Vec<&str> = ms
        .iter()
        .enumerate()
        .filter(|(i, m)| {
            matches!(vowels[*i], 'i' | 'u')
                && starts_voiceless(m)
                && ms
                    .get(i + 1)
                    .is_some_and(|next| next == "っ" || starts_voiceless(next))
        })
        .map(|(_, m)| m.as_str())
        .collect();
    let tail = vowels.last().map_or_default(char::to_string);
    let devoiced = if devoiced.is_empty() {
        "—".to_string()
    } else {
        devoiced.join(",")
    };
    format!(
        "| {kanji} | {kana} | {} | {} | {tail} | {devoiced} |",
        ms.len(),
        vowels.iter().collect::<String>()
    )
}

fn main() -> Result<()> {
    let args = Args::parse();
    let text =
        fs::read_to_string(&args.lyrics).with_context(|| args.lyrics.display().to_string())?;
    for line in text.lines() {
        if let Some(header) = line.strip_prefix('#') {
            let (section, bars) = header
                .split_once('|')
                .ok_or_else(|| anyhow!("section line without bars: {line}"))?;
            println!(
                "\n### {section}（{bars}小節）\n\n| 歌詞 | 読み | 音数 | 母音列 | 末尾 | 無声化しやすい音 |\n|---|---|---|---|---|---|"
            );
            continue;
        }
        let (kanji, kana) = line
            .split_once('|')
            .ok_or_else(|| anyhow!("lyric line without reading: {line}"))?;
        println!("{}", row(kanji, kana));
    }
    Ok(())
}
