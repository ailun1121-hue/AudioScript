pub mod catalog;
pub mod disk;
pub mod download;
pub mod extract;
pub mod install;
pub mod state;
pub mod status;

use serde::{Deserialize, Serialize};

pub const EVENT_MODEL_PROGRESS: &str = "models://progress";

pub(crate) const IO_CHUNK: usize = 64 * 1024;

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelKind {
    Asr,
    Mt,
}

impl ModelKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Asr => "asr",
            Self::Mt => "mt",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("未知模型：{0}")]
    UnknownModel(String),

    #[error("模型 {0} 暂不提供下载")]
    NotDownloadable(String),

    #[error("模型 {0} 已安装，如需修复请先删除")]
    AlreadyInstalled(String),

    #[error("内置模型随应用分发，不能删除")]
    Bundled,

    #[error("该模型当前被选中，请先切换到其他模型再删除")]
    Selected,

    #[error("正在转写，请先停止再删除模型")]
    InUse,

    #[error("已有模型下载任务进行中")]
    JobInProgress,

    #[error("下载失败：{0}")]
    Network(String),

    #[error("下载失败：HTTP {0}")]
    Http(u16),

    #[error("校验失败：期望 {expected}，实际 {actual}")]
    Checksum { expected: String, actual: String },

    #[error("归档内容不安全：{0}")]
    UnsafeArchive(String),

    #[error("解压失败：{0}")]
    Extract(String),

    #[error("磁盘空间不足：需要约 {} MB，可用约 {} MB", .need / 1048576, .available / 1048576)]
    DiskSpace { need: u64, available: u64 },

    #[error("模型文件不完整：{0}")]
    Incomplete(String),

    #[error("文件操作失败：{0}")]
    Io(String),

    #[error("下载已取消")]
    Cancelled,

    #[error("{0}")]
    Internal(String),
}

impl From<std::io::Error> for ModelError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}
