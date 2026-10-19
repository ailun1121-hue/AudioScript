import { PermissionPanel } from '@/main/permissions/PermissionPanel'
import { useStore } from '@/main/store'
import { Button } from '@/ui/Button'
import { TitleBarSpacer } from '@/ui/TitleBarSpacer'
import markSvg from './logo-mark.svg?raw'

export function WelcomeGate() {
  const { info, completeOnboarding } = useStore()

  return (
    <main className="flex h-full flex-col bg-[var(--color-surface-0)]">
      <TitleBarSpacer />

      <div className="flex flex-1 items-start justify-center overflow-y-auto px-6 pb-10">
        <div className="w-full max-w-md space-y-6">
          <header className="space-y-3 pt-6 text-center">
            <span
              aria-hidden
              className="mx-auto block h-16 text-[var(--color-ink-0)] [&>svg]:block [&>svg]:h-full [&>svg]:w-auto"
              // biome-ignore lint/security/noDangerouslySetInnerHtml: 内容是构建期常量（`logo-mark.svg?raw`），不含任何用户输入
              dangerouslySetInnerHTML={{ __html: markSvg }}
            />
            <h1 className="font-semibold text-[var(--color-ink-0)] text-xl">欢迎使用 {info?.name ?? 'AudioScript'}</h1>
            <p className="text-[var(--color-ink-1)] text-sm">请先确认系统权限</p>
          </header>

          <PermissionPanel />

          <div className="flex items-center justify-between gap-3">
            <Button onClick={() => void completeOnboarding()}>稍后再说</Button>
            <Button variant="cta" onClick={() => void completeOnboarding()}>
              开始使用
            </Button>
          </div>

          <p className="text-center text-[var(--color-ink-2)] text-[0.7rem]">跳过之后可在「权限」页复查。</p>
        </div>
      </div>
    </main>
  )
}
