import type { ReactNode } from 'react'
import { cn } from './cn'
import { Icon } from './Icon'

type Tone = 'error' | 'warn' | 'info'

interface Props {
  tone: Tone
  title?: ReactNode
  children?: ReactNode
}

export function Alert({ tone, title, children }: Props) {
  return (
    <div role={tone === 'error' ? 'alert' : 'status'} className={cn('border-b px-6 py-2.5', TONE[tone].wrap)}>
      {title && (
        <p className={cn('flex items-center gap-1.5 text-xs', TONE[tone].title)}>
          <Icon name="alert" size={13} />
          {title}
        </p>
      )}
      {children && (
        <div className={cn('text-xs leading-relaxed', TONE[tone].body, title && 'mt-1 pl-[1.15rem]')}>{children}</div>
      )}
    </div>
  )
}

const TONE: Record<Tone, { wrap: string; title: string; body: string }> = {
  error: {
    wrap: 'border-[var(--color-danger)] bg-[var(--color-danger-bg)]',
    title: 'font-medium text-[var(--color-danger-ink)]',
    body: 'text-[var(--color-danger-ink)]',
  },
  warn: {
    wrap: 'border-[var(--color-warn)] bg-[var(--color-warn-bg)]',
    title: 'font-medium text-[var(--color-warn-ink)]',
    body: 'text-[var(--color-warn-ink)]',
  },
  info: {
    wrap: 'border-[var(--color-border-subtle)] bg-[var(--color-surface-1)]',
    title: 'font-medium text-[var(--color-ink-1)]',
    body: 'text-[var(--color-ink-2)]',
  },
}
