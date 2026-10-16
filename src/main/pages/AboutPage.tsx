import { useStore } from '@/main/store'
import { openRepo } from '@/shared/ipc'
import { Icon } from '@/ui/Icon'
import { Page } from '@/ui/Page'
import { SettingsGroup, SettingsRow } from '@/ui/Settings'

function describeBuildTime(ms: number | null): string {
  if (!ms) return '未知'
  const mins = Math.floor((Date.now() - ms) / 60_000)
  const at = new Date(ms).toLocaleString('zh-CN', { hour12: false })
  if (mins < 1) return `${at}（刚刚）`
  if (mins < 60) return `${at}（${mins} 分钟前）`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${at}（${hours} 小时前）`
  return `${at}（${Math.floor(hours / 24)} 天前）`
}

export function AboutPage() {
  const { info } = useStore()

  return (
    <Page title="关于">
      <SettingsGroup title="应用程序">
        <SettingsRow icon="info" title="版本" control={<Mono>{info?.version ?? '—'}</Mono>} />
        <SettingsRow
          icon="chip"
          title="平台"
          control={
            <Mono>
              {info?.platform ?? '—'} · {info?.arch ?? '—'}
            </Mono>
          }
        />
        <SettingsRow
          icon="refresh"
          title="构建时间"
          control={<Mono>{describeBuildTime(info?.builtAtMs ?? null)}</Mono>}
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
    </Page>
  )
}

function Mono({ children }: { children: React.ReactNode }) {
  return <span className="font-mono text-[var(--color-ink-1)] text-xs">{children}</span>
}
