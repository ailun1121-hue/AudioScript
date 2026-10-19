import type { ReactNode } from 'react'
import { cn } from './cn'

interface GroupProps {
  label: string
  children: ReactNode
}

export function ChoiceGroup({ label, children }: GroupProps) {
  return (
    <div role="radiogroup" aria-label={label} className="space-y-2.5">
      {children}
    </div>
  )
}

interface Props {
  title: ReactNode
  description?: ReactNode
  selected: boolean
  onSelect: () => void
  disabled?: boolean
}

export function ChoiceCard({ title, description, selected, onSelect, disabled }: Props) {
  return (
    // biome-ignore lint/a11y/useSemanticElements: 原生 radio 排不出卡片布局；role 与键盘行为由这里显式补齐
    <div
      role="radio"
      aria-checked={selected}
      aria-disabled={disabled}
      tabIndex={disabled ? -1 : 0}
      onClick={() => !disabled && onSelect()}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          if (!disabled) onSelect()
        }
      }}
      className={cn(
        'flex cursor-pointer items-start gap-3 rounded-[var(--radius-card)] border-2 p-4 transition',
        selected
          ? 'border-[var(--color-accent)] bg-[var(--color-surface-selected)]'
          : 'border-transparent bg-[var(--color-surface-1)] hover:bg-[var(--color-surface-2)]',
        disabled && 'cursor-not-allowed opacity-50',
      )}
    >
      <div className="min-w-0 flex-1">
        <div className="font-semibold text-[15px] text-[var(--color-ink-0)] leading-snug">{title}</div>
        {description && (
          <div className="mt-0.5 text-[var(--color-ink-1)] text-[13px] leading-relaxed">{description}</div>
        )}
      </div>
    </div>
  )
}
