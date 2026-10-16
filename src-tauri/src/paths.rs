use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::asr::DEFAULT_MODEL_DIR_NAME;

pub const ENV_MODEL_DIR: &str = "AUDIOSCRIPT_MODEL_DIR";

#[derive(Debug, Clone)]
pub struct ResolvedModelDir {
    pub path: PathBuf,
    pub via: ResolutionVia,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionVia {
    EnvOverride,
    BundledResource,
    DevSourceTree,
}

impl ResolutionVia {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EnvOverride => "env-override",
            Self::BundledResource => "bundled-resource",
            Self::DevSourceTree => "dev-source-tree",
        }
    }
}

pub fn resolve_models_root(app: &AppHandle) -> Result<ResolvedModelDir, String> {
    let mut tried: Vec<String> = Vec::new();

    if let Ok(raw) = std::env::var(ENV_MODEL_DIR) {
        let p = PathBuf::from(&raw);
        if p.is_dir() {
            return Ok(ResolvedModelDir {
                path: p,
                via: ResolutionVia::EnvOverride,
            });
        }
        tried.push(format!("{ENV_MODEL_DIR}={raw} （目录不存在）"));
    }

    match app.path().resource_dir() {
        Ok(res) => {
            let p = res.join("models");
            if p.is_dir() {
                return Ok(ResolvedModelDir {
                    path: p,
                    via: ResolutionVia::BundledResource,
                });
            }
            tried.push(format!("{} （不存在）", p.display()));
        }
        Err(e) => tried.push(format!("resource_dir() 解析失败：{e}")),
    }

    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/models");
    if p.is_dir() {
        return Ok(ResolvedModelDir {
            path: p,
            via: ResolutionVia::DevSourceTree,
        });
    }
    tried.push(format!("{} （不存在）", p.display()));

    Err(format!(
        "找不到模型根目录。已尝试：\n  - {}\n\n\
         若是全新克隆，先运行 `pnpm fetch:model` 下载模型。",
        tried.join("\n  - ")
    ))
}

pub fn resolve_model_dir(app: &AppHandle) -> Result<ResolvedModelDir, String> {
    let mut root = resolve_models_root(app)?;
    root.path = root.path.join(DEFAULT_MODEL_DIR_NAME);

    if !root.path.is_dir() {
        return Err(format!(
            "找不到 ASR 模型目录：{}\n→ 运行 `pnpm fetch:model` 下载",
            root.path.display()
        ));
    }
    Ok(root)
}

pub fn resolve_mt_model_root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(resolve_models_root(app)?.path.join("mt"))
}

pub fn model_dir_is_complete(dir: &Path) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("无法读取 {}：{e}", dir.display()))?;
    let names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();

    let mut missing = Vec::new();
    for prefix in ["encoder", "decoder", "joiner"] {
        if !names
            .iter()
            .any(|n| n.starts_with(prefix) && n.ends_with(".onnx"))
        {
            missing.push(format!("{prefix}*.onnx"));
        }
    }
    if !names.iter().any(|n| n == "tokens.txt") {
        missing.push("tokens.txt".to_string());
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "模型目录 {} 缺少：{}\n实际内容：{}",
            dir.display(),
            missing.join(", "),
            names.join(", ")
        ))
    }
}

pub fn log_startup_selfcheck(app: &AppHandle) {
    match resolve_model_dir(app) {
        Ok(resolved) => {
            tracing::info!(
                via = resolved.via.as_str(),
                path = %resolved.path.display(),
                "模型目录已解析"
            );
            if let Err(e) = model_dir_is_complete(&resolved.path) {
                tracing::error!(
                    "{e}\n→ 运行 `pnpm fetch:model` 补齐，或设置 {ENV_MODEL_DIR} 指向已有模型"
                );
            }
        }
        Err(e) => tracing::error!("{e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(files: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("paths-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in files {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        dir
    }

    #[test]
    fn complete_when_all_four_pieces_present() {
        let dir = temp_dir(&[
            "encoder-epoch-99-avg-1.int8.onnx",
            "decoder-epoch-99-avg-1.onnx",
            "joiner-epoch-99-avg-1.int8.onnx",
            "tokens.txt",
        ]);
        assert!(model_dir_is_complete(&dir).is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn incomplete_error_names_what_is_missing() {
        let dir = temp_dir(&["encoder-epoch-99-avg-1.int8.onnx", "tokens.txt"]);
        let msg = model_dir_is_complete(&dir).unwrap_err();
        assert!(msg.contains("decoder"), "{msg}");
        assert!(msg.contains("joiner"), "{msg}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn subdirectories_do_not_count_as_files() {
        let dir = temp_dir(&["tokens.txt"]);
        std::fs::create_dir_all(dir.join("encoder-x.onnx")).unwrap();
        let msg = model_dir_is_complete(&dir).unwrap_err();
        assert!(msg.contains("encoder"), "{msg}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_dir_is_an_error_not_a_panic() {
        assert!(model_dir_is_complete(Path::new("/nonexistent/models")).is_err());
    }
}
