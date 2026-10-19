use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

use crate::asr::{AsrEngine, TARGET_SAMPLE_RATE};
use crate::audio::{self, CaptureHandle, SILENCE_RMS};
use crate::config::{
    target_lang_for, AsrConfig, EngineType, SessionConfig, TranslateConfig, TranslateEngine,
};
use crate::paths;
use crate::transcript::{TranscriptSegment, TranscriptStore, TranslationState};
use crate::translate::queue::{Job, JobQueue};
use crate::translate::Translator;

const TRANSLATE_QUEUE_CAPACITY: usize = 32;

fn build_translator(
    app: &AppHandle,
    cfg: &TranslateConfig,
) -> Result<Arc<Mutex<Box<dyn Translator>>>, String> {
    let mut engine: Box<dyn Translator> = match cfg.engine {
        TranslateEngine::LocalNmt => {
            let root = paths::resolve_mt_model_root(app)?;
            Box::new(crate::translate::local_nmt::LocalNmt::new(root))
        }
        TranslateEngine::Mock => {
            let fail_every = std::env::var(crate::translate::mock::ENV_FAIL_EVERY)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            Box::new(crate::translate::mock::MockTranslator::new(
                crate::translate::mock::DEFAULT_DELAY,
                fail_every,
            ))
        }
        TranslateEngine::Off => Box::new(crate::translate::mock::MockTranslator::new(
            std::time::Duration::ZERO,
            0,
        )),
        TranslateEngine::RemoteApi => {
            return Err(format!(
                "翻译引擎 {} 尚未接入。可选值：local-nmt（本地离线）、mock（开发用）、off（仅显示原文）",
                cfg.engine.as_str()
            ));
        }
    };

    engine
        .init(cfg)
        .map_err(|e| format!("翻译引擎初始化失败：{e}"))?;

    Ok(Arc::new(Mutex::new(engine)))
}

pub const EVENT_SEGMENTS: &str = "transcript://segments";
pub const EVENT_TASK_STATE: &str = "transcript://state";
pub const EVENT_AUDIO: &str = "transcript://audio";
pub const EVENT_LEVEL: &str = "transcript://level";

const FIRST_TEXT_BUDGET_MS: u128 = 200;

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStatus {
    pub silent: bool,
    pub rms: f64,
    pub segments: u32,
}

const EMIT_INTERVAL: Duration = Duration::from_millis(100);

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

const LEVEL_INTERVAL: Duration = Duration::from_millis(100);

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    #[default]
    Ready,
    Capturing,
    Error,
}

struct RunningSession {
    capture: CaptureHandle,
    engine: Arc<Mutex<Box<dyn AsrEngine>>>,
    device: String,
    device_sample_rate: u32,

    queue: Arc<JobQueue>,
    translator: Arc<Mutex<Box<dyn Translator>>>,
    translate_worker: Option<std::thread::JoinHandle<()>>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub status: TaskStatus,
    pub device: Option<String>,
    pub device_sample_rate: Option<u32>,
}

#[derive(Default)]
pub struct SessionState {
    store: Arc<Mutex<TranscriptStore>>,
    running: Mutex<Option<RunningSession>>,
    status: Mutex<TaskStatus>,
    started_at: Mutex<Option<Instant>>,
    first_text_logged: AtomicBool,
    audio_started_at: Mutex<Option<Instant>>,
}

impl SessionState {
    fn note_first_text(&self) {
        if self.first_text_logged.swap(true, Ordering::SeqCst) {
            return;
        }
        let t0 = *self
            .started_at
            .lock()
            .expect("session start mutex poisoned");
        if let Some(t0) = t0 {
            let ms = t0.elapsed().as_millis();
            if ms <= FIRST_TEXT_BUDGET_MS {
                tracing::info!(ms, budget = FIRST_TEXT_BUDGET_MS, "首次出字：达标");
            } else {
                tracing::warn!(
                    ms,
                    budget = FIRST_TEXT_BUDGET_MS,
                    "首次出字：**超出预算**（含引擎加载——模型是在点开始时才加载的）"
                );
            }
        }
    }
}

impl SessionState {
    pub fn status(&self) -> TaskStatus {
        *self.status.lock().expect("session status mutex poisoned")
    }

    fn set_status(&self, s: TaskStatus) {
        *self.status.lock().expect("session status mutex poisoned") = s;
    }

    pub fn snapshot(&self) -> Vec<TranscriptSegment> {
        self.store
            .lock()
            .expect("transcript store mutex poisoned")
            .snapshot()
            .to_vec()
    }

    pub fn is_running(&self) -> bool {
        self.running
            .lock()
            .expect("session running mutex poisoned")
            .is_some()
    }

    pub fn info(&self) -> SessionInfo {
        let guard = self.running.lock().expect("session running mutex poisoned");
        let status = self.status();
        match guard.as_ref() {
            Some(r) => SessionInfo {
                status,
                device: Some(r.device.clone()),
                device_sample_rate: Some(r.device_sample_rate),
            },
            None => SessionInfo {
                status,
                device: None,
                device_sample_rate: None,
            },
        }
    }
}

impl std::fmt::Debug for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionState")
            .field("status", &self.status())
            .field("running", &self.is_running())
            .field("segments", &self.snapshot().len())
            .finish()
    }
}

pub fn start(
    app: &AppHandle,
    cfg: SessionConfig,
    source: audio::AudioSourceKind,
    device: Option<String>,
) -> Result<(), String> {
    let mut cfg = cfg;
    cfg.translate.target_lang = target_lang_for(&cfg.translate.source_lang).to_owned();

    let state = app.state::<SessionState>();

    if state.is_running() {
        return Err("已在采集中".into());
    }

    let t0 = Instant::now();
    *state
        .started_at
        .lock()
        .expect("session start mutex poisoned") = Some(t0);
    state.first_text_logged.store(false, Ordering::SeqCst);
    *state
        .audio_started_at
        .lock()
        .expect("audio start mutex poisoned") = None;

    if !matches!(cfg.asr.engine_type, EngineType::LocalStreaming) {
        return Err(format!(
            "引擎 {} 尚未接入（当前仅支持 local-streaming）",
            cfg.asr.engine_type.as_str()
        ));
    }

    let resolved = paths::resolve_model_dir(app)?;
    tracing::info!(
        via = resolved.via.as_str(),
        path = %resolved.path.display(),
        "会话使用模型"
    );

    let mut engine = crate::asr::streaming::StreamingAdapter::new();
    let engine_cfg = AsrConfig {
        model_dir: Some(resolved.path.to_string_lossy().into_owned()),
        ..cfg.asr.clone()
    };
    engine
        .init(&engine_cfg)
        .map_err(|e| format!("引擎初始化失败：{e}"))?;
    tracing::info!(
        caps = ?engine.capabilities(),
        ms = t0.elapsed().as_millis(),
        "ASR 引擎已就绪"
    );

    let engine: Arc<Mutex<Box<dyn AsrEngine>>> = Arc::new(Mutex::new(Box::new(engine)));

    let translator = build_translator(app, &cfg.translate)?;
    let caps = translator
        .lock()
        .expect("translator mutex poisoned")
        .capabilities();
    tracing::info!(
        engine = cfg.translate.engine.as_str(),
        ?caps,
        ms = t0.elapsed().as_millis(),
        "翻译引擎已就绪"
    );

    let queue = Arc::new(JobQueue::new(TRANSLATE_QUEUE_CAPACITY));

    let translate_worker = {
        let queue = Arc::clone(&queue);
        let translator = Arc::clone(&translator);
        let store = Arc::clone(&state.store);
        let app_for_worker = app.clone();

        std::thread::Builder::new()
            .name("translate-worker".into())
            .spawn(move || {
                while let Some(job) = queue.pop_blocking() {
                    let result = translator
                        .lock()
                        .expect("translator mutex poisoned")
                        .translate(&job.text)
                        .map_err(|e| e.to_string());

                    let updated = {
                        let mut s = store.lock().expect("transcript store mutex poisoned");
                        s.set_translation(&job.segment_id, result).ok()
                    };

                    if let Some(seg) = updated {
                        match seg.translation_state {
                            TranslationState::Done => {
                                tracing::info!(
                                    "[译文] {}",
                                    seg.translation.as_deref().unwrap_or("")
                                )
                            }
                            TranslationState::Failed => {
                                tracing::warn!(id = %seg.id, "[译文] 失败，该段只显示原文")
                            }
                            _ => {}
                        }
                        emit_segments(&app_for_worker, vec![seg]);
                    }
                }
                tracing::info!("翻译工作线程退出");
            })
            .map_err(|e| format!("启动翻译线程失败：{e}"))?
    };

    let store = Arc::clone(&state.store);
    let engine_for_sink = Arc::clone(&engine);
    let app_for_sink = app.clone();
    let queue_for_sink = Arc::clone(&queue);

    let mut last_emit = Instant::now()
        .checked_sub(EMIT_INTERVAL)
        .unwrap_or_else(Instant::now);
    let mut pending: Vec<TranscriptSegment> = Vec::new();

    let mut last_beat = Instant::now();
    let mut energy = 0.0f64;
    let mut energy_n = 0usize;
    let mut was_silent: Option<bool> = None;
    let mut last_level = Instant::now();
    let mut level_sq = 0.0f64;
    let mut level_n = 0usize;
    let mut level_emit_failed = false;

    let capture = audio::start(source, device.as_deref(), move |chunk: &[f32]| {
        {
            let state = app_for_sink.state::<SessionState>();
            let mut clock = state
                .audio_started_at
                .lock()
                .expect("audio start mutex poisoned");
            if clock.is_none() {
                *clock = Some(Instant::now());
            }
        }

        let sum_sq = chunk.iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>();
        energy += sum_sq;
        energy_n += chunk.len();
        level_sq += sum_sq;
        level_n += chunk.len();

        if last_level.elapsed() >= LEVEL_INTERVAL {
            let level = if level_n > 0 {
                (level_sq / level_n as f64).sqrt()
            } else {
                0.0
            };
            if let Err(e) = app_for_sink.emit(EVENT_LEVEL, level) {
                if !level_emit_failed {
                    level_emit_failed = true;
                    tracing::error!("广播电平失败（后续同类失败不再记录）：{e}");
                }
            }
            last_level = Instant::now();
            level_sq = 0.0;
            level_n = 0;
        }

        if last_beat.elapsed() >= HEARTBEAT_INTERVAL {
            let rms = if energy_n > 0 {
                (energy / energy_n as f64).sqrt()
            } else {
                0.0
            };
            let count = store.lock().expect("transcript store mutex poisoned").len();
            let silent = rms < SILENCE_RMS;

            if silent {
                let hint = if count > 0 {
                    "已转出过文字，多半只是暂时没有音频（暂停或静音段落），不一定是故障"
                } else if matches!(
                    source,
                    audio::AudioSourceKind::Loopback | audio::AudioSourceKind::Mix
                ) {
                    "还没转出任何文字。确认播放器在出声；若确实在播，检查「屏幕与系统音频录制」权限"
                } else {
                    "还没转出任何文字，麦克风可能未授权、被占用，或所选设备是虚拟回环"
                };

                if count > 0 {
                    if was_silent != Some(true) {
                        tracing::debug!(
                            rms = format!("{rms:.6}"),
                            segments = count,
                            source = source.as_str(),
                            "音频电平接近零：{hint}"
                        );
                    }
                } else {
                    tracing::warn!(
                        rms = format!("{rms:.6}"),
                        segments = count,
                        source = source.as_str(),
                        "**音频电平接近零**：{hint}"
                    );
                }
            } else {
                tracing::info!(
                    rms = format!("{rms:.4}"),
                    segments = count,
                    source = source.as_str(),
                    "心跳"
                );
            }

            if was_silent != Some(silent) {
                was_silent = Some(silent);
                if let Err(e) = app_for_sink.emit(
                    EVENT_AUDIO,
                    AudioStatus {
                        silent,
                        rms,
                        segments: count as u32,
                    },
                ) {
                    tracing::debug!("广播音频状态失败（可能没有窗口）：{e}");
                }
            }

            last_beat = Instant::now();
            energy = 0.0;
            energy_n = 0;
        }

        let deltas = {
            let mut eng = engine_for_sink.lock().expect("engine mutex poisoned");
            match eng.feed_pcm(chunk) {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("引擎处理音频失败：{e}");
                    return;
                }
            }
        };
        if deltas.is_empty() {
            return;
        }

        let to_translate = {
            let mut s = store.lock().expect("transcript store mutex poisoned");
            match s.apply(deltas) {
                Ok(changed) => {
                    for seg in &changed {
                        if seg.state != crate::transcript::SegmentState::Partial {
                            let audio_ms: i128 = app_for_sink
                                .state::<SessionState>()
                                .audio_started_at
                                .lock()
                                .expect("audio start mutex poisoned")
                                .map(|a| a.elapsed().as_millis() as i128)
                                .unwrap_or(0);
                            tracing::info!(
                                start_ms = seg.start_ms,
                                end_ms = seg.end_ms,
                                gap_ms = audio_ms - seg.end_ms as i128,
                                "[定稿] {}",
                                seg.text
                            );
                        }
                    }

                    let ids: Vec<String> = changed
                        .iter()
                        .filter(|s| {
                            s.state.is_frozen() && s.translation_state == TranslationState::Idle
                        })
                        .map(|s| s.id.clone())
                        .collect();

                    let marked = s.mark_pending_translation(&ids);
                    pending.extend(changed);
                    marked
                }
                Err(e) => {
                    tracing::error!("段落状态机拒绝了引擎输出：{e}");
                    return;
                }
            }
        };

        for seg in to_translate {
            let dropped = queue_for_sink.push(Job {
                segment_id: seg.id.clone(),
                text: seg.text.clone(),
            });
            if let Some(d) = dropped {
                tracing::warn!(
                    dropped_ms = d.segment_id,
                    "翻译队列溢出，丢弃最旧的任务（翻译跟不上实时）"
                );
            }
            pending.push(seg);
        }

        if last_emit.elapsed() >= EMIT_INTERVAL && !pending.is_empty() {
            let batch = coalesce(std::mem::take(&mut pending));
            emit_segments(&app_for_sink, batch);
            last_emit = Instant::now();
        }
    })
    .map_err(|e| format!("启动采集失败：{e}"))?;

    let device_name = capture.device_name().to_string();
    let device_sample_rate = capture.device_sample_rate();

    tracing::info!(
        device = %device_name,
        device_sample_rate,
        target_sample_rate = TARGET_SAMPLE_RATE,
        setup_ms = t0.elapsed().as_millis(),
        "转写会话已开始"
    );

    *state
        .running
        .lock()
        .expect("session running mutex poisoned") = Some(RunningSession {
        capture,
        engine,
        device: device_name,
        device_sample_rate,
        queue,
        translator,
        translate_worker: Some(translate_worker),
    });
    state.set_status(TaskStatus::Capturing);
    emit_status(app, TaskStatus::Capturing);

    Ok(())
}

pub fn stop(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<SessionState>();

    let Some(session) = state
        .running
        .lock()
        .expect("session running mutex poisoned")
        .take()
    else {
        state.set_status(TaskStatus::Ready);
        return Ok(());
    };

    state.set_status(TaskStatus::Ready);
    emit_status(app, TaskStatus::Ready);
    tracing::info!("转写会话已停止，收尾转入后台");

    // 收尾要 join 音频线程和翻译线程、释放两套原生模型，其中任何一步都可能在
    // Windows 设备层卡住；放进后台线程，命令线程和 UI 不再被拖住。
    let pending = Arc::new(Mutex::new(Some(session)));
    let app_for_teardown = app.clone();
    let pending_for_teardown = Arc::clone(&pending);

    let spawned = std::thread::Builder::new()
        .name("session-teardown".into())
        .spawn(move || {
            let session = pending_for_teardown
                .lock()
                .expect("teardown slot mutex poisoned")
                .take();
            if let Some(session) = session {
                teardown(&app_for_teardown, session);
            }
        });

    if let Err(e) = spawned {
        tracing::error!("启动收尾线程失败，改为在命令线程收尾：{e}");
        let session = { pending.lock().expect("teardown slot mutex poisoned").take() };
        if let Some(session) = session {
            teardown(app, session);
        }
    }

    Ok(())
}

fn teardown(app: &AppHandle, session: RunningSession) {
    let RunningSession {
        capture,
        engine,
        queue,
        translator,
        translate_worker,
        ..
    } = session;
    let t0 = Instant::now();

    tracing::info!("收尾：等待采集线程退出");
    drop(capture);
    tracing::info!(ms = t0.elapsed().as_millis(), "收尾：采集线程已退出");

    tracing::info!("收尾：冲刷引擎");
    let tail = {
        let mut eng = engine.lock().expect("engine mutex poisoned");
        let flushed = eng.flush();
        eng.shutdown();
        flushed
    };
    match tail {
        Ok(tail) if !tail.is_empty() => {
            let state = app.state::<SessionState>();
            let changed = {
                let mut s = state.store.lock().expect("transcript store mutex poisoned");
                s.apply(tail)
            };
            match changed {
                Ok(changed) => emit_segments(app, changed),
                Err(e) => tracing::error!("收尾段落被状态机拒绝：{e}"),
            }
        }
        Ok(_) => {}
        Err(e) => tracing::error!("冲刷引擎失败：{e}"),
    }
    tracing::info!(ms = t0.elapsed().as_millis(), "收尾：引擎已关闭");

    let dropped = queue.clear();
    if dropped > 0 {
        tracing::info!(dropped, "停止时丢弃了积压的翻译任务");
    }
    queue.close();

    if let Some(handle) = translate_worker {
        tracing::info!("收尾：等待翻译线程退出");
        if let Err(e) = handle.join() {
            tracing::error!("翻译线程异常退出：{e:?}");
        }
        tracing::info!(ms = t0.elapsed().as_millis(), "收尾：翻译线程已退出");
    }

    tracing::info!("收尾：释放翻译引擎");
    translator
        .lock()
        .expect("translator mutex poisoned")
        .shutdown();
    tracing::info!(ms = t0.elapsed().as_millis(), "收尾完成");
}

pub fn clear(app: &AppHandle) {
    let state = app.state::<SessionState>();
    {
        let mut store = state.store.lock().expect("transcript store mutex poisoned");
        *store = TranscriptStore::new();
    }
    emit_segments(app, Vec::new());
}

fn coalesce(batch: Vec<TranscriptSegment>) -> Vec<TranscriptSegment> {
    let mut latest: HashMap<String, TranscriptSegment> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for seg in batch {
        if !latest.contains_key(&seg.id) {
            order.push(seg.id.clone());
        }
        let keep = latest
            .get(&seg.id)
            .is_none_or(|e| seg.revision >= e.revision);
        if keep {
            latest.insert(seg.id.clone(), seg);
        }
    }

    order
        .into_iter()
        .filter_map(|id| latest.remove(&id))
        .collect()
}

fn emit_segments(app: &AppHandle, segments: Vec<TranscriptSegment>) {
    if segments.iter().any(|s| !s.text.trim().is_empty()) {
        app.state::<SessionState>().note_first_text();
    }

    if let Err(e) = app.emit(EVENT_SEGMENTS, segments) {
        tracing::debug!("广播段落失败（可能没有窗口在监听）：{e}");
    }
}

fn emit_status(app: &AppHandle, status: TaskStatus) {
    if let Err(e) = app.emit(EVENT_TASK_STATE, status) {
        tracing::debug!("广播任务状态失败（可能没有窗口在监听）：{e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::SegmentState;

    fn seg(id: &str, text: &str, rev: u32) -> TranscriptSegment {
        TranscriptSegment {
            id: id.into(),
            start_ms: 0,
            end_ms: 100,
            text: text.into(),
            translation: None,
            translation_state: crate::transcript::TranslationState::Idle,
            state: SegmentState::Partial,
            revision: rev,
        }
    }

    #[test]
    fn coalesce_keeps_latest_revision_per_id() {
        let batch = vec![
            seg("a", "今", 1),
            seg("a", "今天", 2),
            seg("b", "另", 1),
            seg("a", "今天下午", 3),
        ];
        let out = coalesce(batch);

        assert_eq!(out.len(), 2, "应按 id 去重");
        assert_eq!(out[0].id, "a");
        assert_eq!(out[0].text, "今天下午", "应保留最新 revision");
        assert_eq!(out[1].id, "b", "顺序应按首次出现");
    }

    #[test]
    fn coalesce_does_not_let_an_older_revision_win() {
        let out = coalesce(vec![seg("a", "新", 5), seg("a", "旧", 2)]);
        assert_eq!(out[0].text, "新");
    }
}
