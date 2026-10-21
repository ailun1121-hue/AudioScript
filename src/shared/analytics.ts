import { trackEvent } from '@aptabase/tauri'

type Props = Record<string, string | number>

let enabled = false

export function setTrackingEnabled(next: boolean) {
  enabled = next
}

export function track(name: string, props?: Props) {
  if (!enabled) return
  trackEvent(name, props).catch(() => {})
}
