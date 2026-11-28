const SMALL_KANA: &str = "ゃゅょぁぃぅぇぉゎ";
const VOWELS: [(&str, char); 5] = [
    ("あかさたなはまやらわがざだばぱぁゃゎ", 'a'),
    ("いきしちにひみりぎじぢびぴぃ", 'i'),
    ("うくすつぬふむゆるぐずづぶぷぅゅ", 'u'),
    ("えけせてねへめれげぜでべぺぇ", 'e'),
    ("おこそとのほもよろをごぞどぼぽぉょ", 'o'),
];

#[must_use]
pub fn moras(reading: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for ch in reading.chars().filter(|c| !c.is_whitespace()) {
        match out.last_mut() {
            Some(last) if SMALL_KANA.contains(ch) => last.push(ch),
            _ => out.push(ch.to_string()),
        }
    }
    out
}

#[must_use]
pub fn vowel(mora: &str) -> char {
    match mora {
        "っ" => 'Q',
        "ん" => 'N',
        "ー" => '-',
        _ => mora
            .chars()
            .last()
            .and_then(|last| {
                VOWELS
                    .iter()
                    .find(|(kana, _)| kana.contains(last))
                    .map(|(_, v)| *v)
            })
            .unwrap_or('?'),
    }
}

#[cfg(test)]
mod tests {
    use super::{moras, vowel};

    #[test]
    fn small_kana_join_the_previous_mora() {
        assert_eq!(
            moras("きょう いっかい"),
            ["きょ", "う", "い", "っ", "か", "い"]
        );
    }

    #[test]
    fn vowels_follow_the_last_kana() {
        let found: String = moras("しゃっきんー").iter().map(|m| vowel(m)).collect();
        assert_eq!(found, "aQiN-");
    }
}
