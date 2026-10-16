import type { ReactNode } from 'react'

interface Props {
  title: string
  children: ReactNode
}

export function Page({ title, children }: Props) {
  return (
    <div className="mx-auto max-w-2xl space-y-6 px-6 py-6">
      <header className="space-y-1">
        <h2 className="font-semibold text-[var(--color-ink-0)] text-base">{title}</h2>
      </header>
      {children}
    </div>
  )
}
