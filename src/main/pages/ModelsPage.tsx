import { useEffect, useState } from 'react'
import { type ModelEntry, type ModelInfo, modelInfo } from '@/shared/ipc'
import { Page } from '@/ui/Page'
import { SettingsGroup, SettingsRow } from '@/ui/Settings'
import { StatusBadge } from '@/ui/StatusBadge'

export function ModelsPage() {
  const [info, setInfo] = useState<ModelInfo | null>(null)

  useEffect(() => {
    modelInfo()
      .then(setInfo)
      .catch(() => {})
  }, [])

  if (!info) {
    return (
      <Page title="模型">
        <p className="text-[var(--color-ink-2)] text-sm">读取中…</p>
      </Page>
    )
  }

  return (
    <Page title="模型">
      <SettingsGroup title="语音识别" footer="随安装包分发，离线运行，音频与文本不出本机。">
        {info.asr ? (
          <SettingsRow
            icon="waveform"
            title={info.asr.name}
            description={
              <>
                路径来源：<span className="font-mono">{info.asr.via}</span>
              </>
            }
            control={
              <StatusBadge tone={info.asr.complete ? 'ok' : 'danger'}>
                {info.asr.complete ? '已就绪' : '文件不完整'}
              </StatusBadge>
            }
          />
        ) : (
          <SettingsRow icon="alert" title="找不到模型" description="模型目录解析失败，详见日志。" />
        )}
      </SettingsGroup>

      <SettingsGroup title="翻译" footer="两个方向各一个模型，可只安装其一。">
        {(info.translate ?? []).map((m) => (
          <SettingsRow
            key={m.name}
            icon="translate"
            title={m.name}
            description={<LicenseTag entry={m} />}
            control={<StatusBadge tone={m.installed ? 'ok' : 'warn'}>{m.installed ? '已安装' : '未安装'}</StatusBadge>}
          />
        ))}
        {!info.translate && <SettingsRow icon="alert" title="找不到翻译模型目录" />}
      </SettingsGroup>

      <p className="px-1 text-[var(--color-ink-2)] text-[0.7rem] leading-relaxed">
        模型随应用版本更新；用户设置不在应用包内，升级不会丢失。
        {info.translate?.some((m) => !m.installed) && (
          <>
            {' '}
            缺哪个就运行 <span className="font-mono">pnpm fetch:mt</span> 下载并转换。
          </>
        )}
      </p>
    </Page>
  )
}

function LicenseTag({ entry }: { entry: ModelEntry }) {
  return (
    <span className="flex flex-wrap items-center gap-x-1.5 gap-y-1">
      <span className="font-mono">{entry.license}</span>
      {entry.requiresAttribution && '· 该许可要求署名'}
    </span>
  )
}
