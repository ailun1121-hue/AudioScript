pub mod segment;
pub mod streaming;

use crate::config::AsrConfig;
use crate::transcript::TranscriptSegment;

pub const TARGET_SAMPLE_RATE: u32 = 16_000;

pub mod latin_case;

pub const DEFAULT_MODEL_DIR_NAME: &str =
    "sherpa-onnx-streaming-zipformer-small-bilingual-zh-en-2023-02-16";

#[derive(Debug, thiserror::Error)]
pub enum AsrError {
    #[error("引擎未初始化")]
    NotInitialized,

    #[error("配置无效：{0}")]
    InvalidConfig(String),

    #[error("模型加载失败：{0}")]
    ModelLoad(String),

    #[error("模型目录缺少文件：{0}")]
    MissingModelFile(String),

    #[error("引擎内部错误：{0}")]
    Internal(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineCaps {
    pub streaming: bool,
    pub produces_partial: bool,
    pub requires_full_utterance: bool,
    pub supports_endpoint: bool,
}

impl EngineCaps {
    pub const STREAMING_TRANSDUCER: Self = Self {
        streaming: true,
        produces_partial: true,
        requires_full_utterance: false,
        supports_endpoint: true,
    };

    pub const BATCH: Self = Self {
        streaming: false,
        produces_partial: false,
        requires_full_utterance: true,
        supports_endpoint: false,
    };
}

pub trait AsrEngine: Send {
    fn init(&mut self, cfg: &AsrConfig) -> Result<(), AsrError>;

    fn capabilities(&self) -> EngineCaps;

    fn feed_pcm(&mut self, pcm: &[f32]) -> Result<Vec<TranscriptSegment>, AsrError>;

    fn flush(&mut self) -> Result<Vec<TranscriptSegment>, AsrError>;

    fn shutdown(&mut self);
}
