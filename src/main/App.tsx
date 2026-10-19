import { useState } from 'react'
import { TaskStatus } from '@/shared/types'
import { Alert } from '@/ui/Alert'
import { Button } from '@/ui/Button'
import { PrimaryAction } from '@/ui/PrimaryAction'
import { Sidebar } from '@/ui/Sidebar'
import { TitleBarSpacer } from '@/ui/TitleBarSpacer'
import { Brand } from './Brand'
import { NAV, navItem, type PageId } from './navigation'
import { AboutPage } from './pages/AboutPage'
import { AudioPage } from './pages/AudioPage'
import { GeneralPage } from './pages/GeneralPage'
import { ModelsPage } from './pages/ModelsPage'
import { OverlayPage } from './pages/OverlayPage'
import { PermissionsPage } from './pages/PermissionsPage'
import { TranscribePage } from './pages/TranscribePage'
import { TranslatePage } from './pages/TranslatePage'
import { StoreProvider, useStore } from './store'
import { WelcomeGate } from './WelcomeGate'

export function App() {
  return (
    <StoreProvider>
      <Root />
    </StoreProvider>
  )
}

function Root() {
  const { prefsReady, onboardingDone } = useStore()

  if (!prefsReady) return <BootSplash />
  return onboardingDone ? <Shell /> : <WelcomeGate />
}

function BootSplash() {
  return (
    <main className="flex h-full">
      <section className="flex min-w-0 flex-1 flex-col bg-[var(--color-surface-0)]">
        <TitleBarSpacer />
      </section>
    </main>
  )
}

function Shell() {
  const [page, setPage] = useState<PageId>('transcribe')
  const current = navItem(page)

  return (
    <main className="flex h-full">
      <Sidebar items={NAV} current={page} onNavigate={setPage} brand={<Brand />} header={<PrimaryActionSlot />} />

      <section className="flex min-w-0 flex-1 flex-col bg-[var(--color-surface-0)]">
        <TitleBarSpacer />

        <Alerts onOpenPermissions={() => setPage('permissions')} />

        <div className="flex-1 overflow-y-auto">
          <PageSwitch page={page} title={current.label} onOpenPermissions={() => setPage('permissions')} />
        </div>
      </section>
    </main>
  )
}

function PrimaryActionSlot() {
  const { status, pending, activeDevice, toggleSession } = useStore()
  return (
    <PrimaryAction
      capturing={status === TaskStatus.Capturing}
      pending={pending}
      activeDevice={activeDevice}
      onToggle={() => void toggleSession()}
    />
  )
}

interface PageSwitchProps {
  page: PageId
  title: string
  onOpenPermissions: () => void
}

function PageSwitch({ page, title, onOpenPermissions }: PageSwitchProps) {
  switch (page) {
    case 'transcribe':
      return <TranscribePage />
    case 'audio':
      return <AudioPage onOpenPermissions={onOpenPermissions} />
    case 'permissions':
      return <PermissionsPage />
    case 'translate':
      return <TranslatePage />
    case 'overlay':
      return <OverlayPage />
    case 'general':
      return <GeneralPage />
    case 'models':
      return <ModelsPage />
    case 'about':
      return <AboutPage />
    default:
      return <p className="p-6 text-[var(--color-ink-2)] text-sm">未知页面：{title}</p>
  }
}

function Alerts({ onOpenPermissions }: { onOpenPermissions: () => void }) {
  const { error, setError, hotkeyProblems, audio, status, permissionError, setPermissionError } = useStore()

  return (
    <>
      {error && (
        <Alert tone="error" title="操作失败">
          <p className="font-mono">{error}</p>
          <button type="button" onClick={() => setError(null)} className="mt-1 underline opacity-70 hover:opacity-100">
            关闭
          </button>
        </Alert>
      )}

      {permissionError && (
        <Alert tone="warn" title="缺少系统权限">
          <p>{permissionError.message}</p>
          <div className="mt-1.5 flex gap-1.5">
            <Button size="sm" onClick={onOpenPermissions}>
              去「权限」页处理
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setPermissionError(null)}>
              关闭
            </Button>
          </div>
        </Alert>
      )}

      {hotkeyProblems.length > 0 && (
        <Alert tone="warn" title={`${hotkeyProblems.length} 个全局热键注册失败`}>
          {hotkeyProblems.map((h) => `${h.accelerator} ${h.label}`).join('、')}
          。悬浮窗将无法用热键操作，请在「悬浮窗」页重新绑定。
        </Alert>
      )}

      {audio?.silent && audio.segments === 0 && status === TaskStatus.Capturing && (
        <Alert tone="warn" title="收不到声音（电平 ≈ 0）">
          到「音频」页查看排查步骤。
        </Alert>
      )}
    </>
  )
}
