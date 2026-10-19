import { isMacOS } from '@/shared/platform'

export function TitleBarSpacer() {
  if (!isMacOS) return null
  return <div data-tauri-drag-region className="h-11 shrink-0" />
}
