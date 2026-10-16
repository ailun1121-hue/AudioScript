import { useEffect, useState } from 'react'
import { Button } from './Button'
import { cn } from './cn'

interface Props {
  pretty: string
  onRecord: (accelerator: string) => void
  disabled?: boolean
}

export function ShortcutRecorder({ pretty, onRecord, disabled }: Props) {
  const [recording, setRecording] = useState(false)

  useEffect(() => {
    if (!recording) return

    const onKey = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()

      if (e.key === 'Escape') {
        setRecording(false)
        return
      }

      if (['Shift', 'Control', 'Alt', 'Meta'].includes(e.key)) return

      const parts: string[] = []
      const isMac = navigator.platform.toUpperCase().includes('MAC')
      if (isMac ? e.metaKey : e.ctrlKey) parts.push('CmdOrControl')
      else if (isMac ? e.ctrlKey : e.metaKey) parts.push('Control')
      if (e.altKey) parts.push('Alt')
      if (e.shiftKey) parts.push('Shift')

      const isFunctionKey = /^F\d{1,2}$/.test(e.key)
      if (parts.length === 0 && !isFunctionKey) {
        return
      }

      const code = e.code || e.key
      parts.push(code)
      onRecord(parts.join('+'))
      setRecording(false)
    }

    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  }, [recording, onRecord])

  return (
    <div className="flex items-center gap-2">
      <span
        className={cn(
          'min-w-[5.5rem] rounded-[var(--radius-ctl)] border px-2 py-1 text-center font-mono text-xs',
          recording
            ? 'border-[var(--color-accent)] bg-[var(--color-surface-0)] text-[var(--color-accent)]'
            : 'border-[var(--color-border-subtle)] text-[var(--color-ink-1)]',
        )}
      >
        {recording ? '按下组合键…' : pretty}
      </span>
      {recording ? (
        <Button size="sm" onClick={() => setRecording(false)}>
          取消
        </Button>
      ) : (
        <Button size="sm" disabled={disabled} onClick={() => setRecording(true)}>
          修改
        </Button>
      )}
    </div>
  )
}
