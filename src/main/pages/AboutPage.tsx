import { type ReactNode, useEffect, useState } from 'react'
import { useStore } from '@/main/store'
import { formatCmdError, type ModelsList, modelsList, openRepo, openSponsor, openUrl } from '@/shared/ipc'
import { ModelKind } from '@/shared/types'
import { Button } from '@/ui/Button'
import { Icon } from '@/ui/Icon'
import { Page } from '@/ui/Page'
import { SettingsGroup, SettingsRow } from '@/ui/Settings'

const REPO_URL = 'https://github.com/niyongsheng/AudioScript'

const LATEST_RELEASE_URL = 'https://api.github.com/repos/niyongsheng/AudioScript/releases/latest'

const AFDIAN_BADGE = 'https://img.shields.io/badge/爱发电-Afdian-946ce6?style=for-the-badge&logo=github&logoColor=white'

type UpdateState =
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'upToDate' }
  | { kind: 'available'; version: string; url: string }
  | { kind: 'error'; message: string }

function compareVersions(a: string, b: string): number {
  const parts = (v: string) =>
    v
      .replace(/^v/, '')
      .split('.')
      .map((p) => Number.parseInt(p, 10) || 0)
  const pa = parts(a)
  const pb = parts(b)
  for (let i = 0; i < Math.max(pa.length, pb.length); i += 1) {
    const diff = (pa[i] ?? 0) - (pb[i] ?? 0)
    if (diff !== 0) return diff > 0 ? 1 : -1
  }
  return 0
}

function describeUpdate(state: UpdateState, current: string | null): ReactNode {
  switch (state.kind) {
    case 'checking':
      return '正在从 GitHub 获取最新版本…'
    case 'upToDate':
      return current ? `已是最新版本（v${current}）` : '已是最新版本'
    case 'available':
      return `发现新版本 v${state.version}`
    case 'error':
      return `检查失败：${state.message}`
    default:
      return '从 GitHub 获取最新版本'
  }
}

export function AboutPage() {
  const { info, setError } = useStore()
  const [update, setUpdate] = useState<UpdateState>({ kind: 'idle' })
  const [models, setModels] = useState<ModelsList | null>(null)
  const [modelsFailed, setModelsFailed] = useState(false)
  const [licensesOpen, setLicensesOpen] = useState(false)

  useEffect(() => {
    let disposed = false
    modelsList()
      .then((m) => {
        if (!disposed) setModels(m)
      })
      .catch(() => {
        if (!disposed) setModelsFailed(true)
      })
    return () => {
      disposed = true
    }
  }, [])

  async function checkForUpdates() {
    if (update.kind === 'checking' || !info) return
    setUpdate({ kind: 'checking' })
    const controller = new AbortController()
    const timer = setTimeout(() => controller.abort(), 10_000)
    try {
      const res = await fetch(LATEST_RELEASE_URL, {
        headers: { Accept: 'application/vnd.github+json' },
        signal: controller.signal,
      })
      if (!res.ok) {
        throw new Error(res.status === 403 ? 'GitHub 请求受限，请稍后重试' : `HTTP ${res.status}`)
      }
      const data = (await res.json()) as { tag_name?: string; html_url?: string }
      const latest = (data.tag_name ?? '').replace(/^v/, '')
      if (!latest) throw new Error('响应缺少版本号')
      const url = data.html_url ?? `${REPO_URL}/releases/tag/${data.tag_name}`
      setUpdate(
        compareVersions(latest, info.version) > 0 ? { kind: 'available', version: latest, url } : { kind: 'upToDate' },
      )
    } catch (e) {
      let message = String(e)
      if (e instanceof DOMException && e.name === 'AbortError') message = '检查超时，请检查网络后重试'
      else if (e instanceof TypeError) message = '网络不可用或无法访问 GitHub'
      else if (e instanceof Error) message = e.message
      setUpdate({ kind: 'error', message })
    } finally {
      clearTimeout(timer)
    }
  }

  async function openRelease(url: string) {
    try {
      await openUrl(url)
    } catch (e) {
      setError(formatCmdError(e))
    }
  }

  async function openSponsorPage() {
    try {
      await openSponsor()
    } catch (e) {
      setError(formatCmdError(e))
    }
  }

  const available = update.kind === 'available' ? update : null

  return (
    <Page title="关于">
      <SettingsGroup title="应用程序">
        <SettingsRow icon="info" title="版本" control={<Mono>{info?.version ?? '—'}</Mono>} />
        <SettingsRow
          icon="download"
          title="检查更新"
          description={describeUpdate(update, info?.version ?? null)}
          control={
            available ? (
              <Button size="sm" variant="primary" icon="external" onClick={() => void openRelease(available.url)}>
                下载 v{available.version}
              </Button>
            ) : (
              <Button
                size="sm"
                icon="refresh"
                onClick={() => void checkForUpdates()}
                disabled={!info}
                pending={update.kind === 'checking'}
              >
                检查
              </Button>
            )
          }
        />
        <SettingsRow
          icon="chip"
          title="平台"
          control={
            <Mono>
              {info?.platform ?? '—'} · {info?.arch ?? '—'}
            </Mono>
          }
        />
      </SettingsGroup>

      <SettingsGroup title="支持开发">
        <SettingsRow
          icon="heart"
          title="赞助"
          description="如果 AudioScript 帮到了你，欢迎在爱发电支持后续开发。"
          onClick={() => void openSponsorPage()}
          control={<img src={AFDIAN_BADGE} alt="爱发电" className="h-7 w-auto" />}
        />
      </SettingsGroup>

      <SettingsGroup title="开源许可">
        <SettingsRow
          icon="github"
          title={
            <span className="flex items-center gap-2">
              LICENSE
              <Mono>MIT</Mono>
            </span>
          }
          onClick={() => void openRepo()}
          control={<Icon name="chevron" size={16} className="mt-0.5 text-[var(--color-ink-2)]" />}
        />
      </SettingsGroup>

      <SettingsGroup
        title="模型与许可"
        footer={
          licensesOpen
            ? '本项目把 OPUS-MT 权重转换为 CTranslate2 int8，并把 SentencePiece 词表转换为 tokenizer JSON；未更改训练数据。'
            : undefined
        }
      >
        <SettingsRow
          icon="shield"
          title="模型许可"
          onClick={() => setLicensesOpen((open) => !open)}
          control={
            <Icon
              name="chevron"
              size={16}
              className={`mt-0.5 text-[var(--color-ink-2)] transition-transform ${licensesOpen ? 'rotate-90' : ''}`}
            />
          }
        />
        {licensesOpen &&
          (models ? (
            [...models.asr, ...models.mt].flatMap((m) =>
              m.licenses.map((l) => (
                <SettingsRow
                  key={`${m.id}-${l.component}`}
                  icon={m.kind === ModelKind.Asr ? 'waveform' : 'translate'}
                  title={l.component}
                  description={
                    <>
                      {m.title} · {l.license}
                      {l.requiresAttribution && ' · 需署名'}
                      {l.attribution && ` · ${l.attribution}`}
                    </>
                  }
                  control={
                    l.licenseUrl ? (
                      <Button
                        size="sm"
                        variant="ghost"
                        icon="external"
                        onClick={() => void openUrl(l.licenseUrl as string).catch((e) => setError(formatCmdError(e)))}
                      >
                        许可
                      </Button>
                    ) : undefined
                  }
                />
              )),
            )
          ) : (
            <SettingsRow title={modelsFailed ? '模型许可读取失败，请到「模型」页重试' : '读取中…'} />
          ))}
      </SettingsGroup>
    </Page>
  )
}

function Mono({ children }: { children: React.ReactNode }) {
  return <span className="font-mono text-[var(--color-ink-1)] text-xs">{children}</span>
}
