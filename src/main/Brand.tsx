import { useStore } from '@/main/store'
import markSvg from './logo-mark.svg?raw'

export function LogoMark({ className }: { className: string }) {
  return (
    <span
      aria-hidden
      className={className}
      // biome-ignore lint/security/noDangerouslySetInnerHtml: 内容是构建期常量（`logo-mark.svg?raw`），不含任何用户输入
      dangerouslySetInnerHTML={{ __html: markSvg }}
    />
  )
}

export function Brand() {
  const { info } = useStore()

  return (
    <div className="flex items-center gap-2">
      <LogoMark className="h-6 text-[var(--color-ink-0)] [&>svg]:block [&>svg]:h-full [&>svg]:w-auto" />
      <span className="truncate font-semibold text-[var(--color-ink-0)] text-lg">{info?.name}</span>
    </div>
  )
}
