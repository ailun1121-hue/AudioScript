use zhconv::{zhconv, Variant};

pub fn to_simplified(text: &str) -> String {
    zhconv(text, Variant::ZhHans)
}

pub fn to_traditional(text: &str) -> String {
    zhconv(text, Variant::ZhHant)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_between_scripts() {
        assert_eq!(to_traditional("汉字"), "漢字");
        assert_eq!(to_simplified("漢字"), "汉字");
    }

    #[test]
    fn converts_phrases_not_just_characters() {
        assert_eq!(to_traditional("机器学习"), "機器學習");
        assert_eq!(to_simplified("機器學習"), "机器学习");
        assert_eq!(to_traditional("软件"), "軟件");
    }

    #[test]
    fn leaves_english_and_short_text_alone() {
        for s in ["hello, world!", "OK", "123", ""] {
            assert_eq!(to_traditional(s), s);
            assert_eq!(to_simplified(s), s);
        }
    }
}
