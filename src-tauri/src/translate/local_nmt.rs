use std::path::{Path, PathBuf};

use ct2rs::tokenizers::hf::Tokenizer as HfTokenizer;

use super::{TranslateCaps, TranslateError, Translator};
use crate::config::TranslateConfig;

pub const MODEL_EN_ZH: &str = "opus-mt-en-zh-ct2";
pub const MODEL_ZH_EN: &str = "opus-mt-zh-en-ct2";

const SOURCE_TOKENIZER: &str = "source-tokenizer.json";
const TARGET_TOKENIZER: &str = "target-tokenizer.json";

pub struct MarianTokenizer {
    source: HfTokenizer,
    target: HfTokenizer,
}

impl MarianTokenizer {
    fn load(dir: &Path) -> Result<Self, TranslateError> {
        let load = |f: &str| {
            HfTokenizer::from_file(dir.join(f))
                .map_err(|e| TranslateError::ModelLoad(format!("{}/{f}：{e}", dir.display())))
        };
        Ok(Self {
            source: load(SOURCE_TOKENIZER)?,
            target: load(TARGET_TOKENIZER)?,
        })
    }
}

impl ct2rs::Tokenizer for MarianTokenizer {
    fn encode(&self, input: &str) -> anyhow::Result<Vec<String>> {
        let mut tokens = ct2rs::Tokenizer::encode(&self.source, input)?;

        tokens.push("</s>".to_string());
        Ok(tokens)
    }

    fn decode(&self, tokens: Vec<String>) -> anyhow::Result<String> {
        ct2rs::Tokenizer::decode(&self.target, tokens)
    }
}

pub struct LocalNmt {
    engine: Option<ct2rs::Translator<MarianTokenizer>>,
    direction: Direction,
    model_root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    EnZh,
    ZhEn,
}

impl Direction {
    fn dir_name(self) -> &'static str {
        match self {
            Self::EnZh => MODEL_EN_ZH,
            Self::ZhEn => MODEL_ZH_EN,
        }
    }

    fn resolve(from: &str, to: &str) -> Result<Self, TranslateError> {
        let from = from.to_ascii_lowercase();
        let to = to.to_ascii_lowercase();

        if from == "auto" {
            return Err(TranslateError::UnsupportedPair {
                from: "auto（尚未实现，请显式指定源语言）".into(),
                to: to.to_string(),
            });
        }
        let from_is_en = from.starts_with("en");
        let from_is_zh = from.starts_with("zh");
        let to_is_zh = to.starts_with("zh");
        let to_is_en = to.starts_with("en");

        match (from_is_en || from_is_zh, to_is_zh, to_is_en) {
            (true, true, _) if !from_is_zh => Ok(Self::EnZh),
            (true, _, true) if !from_is_en => Ok(Self::ZhEn),
            _ => Err(TranslateError::UnsupportedPair {
                from: from.to_string(),
                to: to.to_string(),
            }),
        }
    }
}

impl LocalNmt {
    pub fn new(model_root: PathBuf) -> Self {
        Self {
            engine: None,
            direction: Direction::EnZh,
            model_root,
        }
    }

    fn model_dir(&self) -> PathBuf {
        self.model_root.join(self.direction.dir_name())
    }

    fn check_model_dir(dir: &Path) -> Result<(), TranslateError> {
        if !dir.is_dir() {
            return Err(TranslateError::MissingModelFile(format!(
                "{} 不存在。\n→ 运行 `pnpm fetch:model` 下载并转换翻译模型",
                dir.display()
            )));
        }

        for f in ["model.bin", SOURCE_TOKENIZER, TARGET_TOKENIZER] {
            if !dir.join(f).is_file() {
                return Err(TranslateError::MissingModelFile(format!(
                    "{}/{f}\n→ 模型目录不完整，重新运行 `pnpm fetch:model`",
                    dir.display()
                )));
            }
        }
        Ok(())
    }
}

impl Translator for LocalNmt {
    fn init(&mut self, cfg: &TranslateConfig) -> Result<(), TranslateError> {
        self.direction = Direction::resolve(&cfg.source_lang, &cfg.target_lang)?;

        if let Some(dir) = &cfg.model_dir {
            self.model_root = PathBuf::from(dir);
        }

        let dir = self.model_dir();
        Self::check_model_dir(&dir)?;

        let config = ct2rs::Config {
            num_threads_per_replica: 2,
            ..Default::default()
        };

        let tok = MarianTokenizer::load(&dir)?;
        let engine = ct2rs::Translator::with_tokenizer(&dir, tok, &config)
            .map_err(|e| TranslateError::ModelLoad(format!("{}：{e}", dir.display())))?;

        self.engine = Some(engine);
        Ok(())
    }

    fn capabilities(&self) -> TranslateCaps {
        TranslateCaps::LOCAL_NMT
    }

    fn translate(&mut self, text: &str) -> Result<String, TranslateError> {
        let engine = self.engine.as_ref().ok_or(TranslateError::NotInitialized)?;

        let sources = vec![text.to_string()];
        let options = ct2rs::TranslationOptions {
            beam_size: 1,
            ..Default::default()
        };

        let results = engine
            .translate_batch(&sources, &options, None)
            .map_err(|e| TranslateError::Internal(e.to_string()))?;

        let (translated, _) = results
            .into_iter()
            .next()
            .ok_or_else(|| TranslateError::Internal("引擎返回了空结果".into()))?;

        if let Some(reason) = crate::translate::degenerate_translation(&translated) {
            return Err(TranslateError::Degenerate { reason });
        }

        Ok(translated)
    }

    fn shutdown(&mut self) {
        self.engine = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_resolves_both_ways() {
        assert_eq!(Direction::resolve("en", "zh").unwrap(), Direction::EnZh);
        assert_eq!(Direction::resolve("zh", "en").unwrap(), Direction::ZhEn);
        assert!(matches!(
            Direction::resolve("auto", "zh"),
            Err(TranslateError::UnsupportedPair { .. })
        ));
        assert_eq!(
            Direction::resolve("en-US", "zh-CN").unwrap(),
            Direction::EnZh
        );
    }

    #[test]
    fn unsupported_pairs_are_rejected_not_silently_defaulted() {
        assert!(matches!(
            Direction::resolve("ja", "zh"),
            Err(TranslateError::UnsupportedPair { .. })
        ));
        assert!(matches!(
            Direction::resolve("en", "fr"),
            Err(TranslateError::UnsupportedPair { .. })
        ));
        assert!(Direction::resolve("zh", "zh").is_err());
    }

    #[test]
    fn missing_model_dir_gives_actionable_error() {
        let e = LocalNmt::check_model_dir(Path::new("/nonexistent/mt")).unwrap_err();
        let msg = e.to_string();
        assert!(msg.contains("fetch:model"), "错误里要给出修复动作：{msg}");
    }

    #[test]
    fn translate_before_init_is_rejected() {
        let mut t = LocalNmt::new(PathBuf::from("/tmp"));
        assert!(matches!(
            t.translate("hello"),
            Err(TranslateError::NotInitialized)
        ));
    }
}
