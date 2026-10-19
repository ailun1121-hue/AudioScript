import { useEffect, useState } from 'react'
import { EVENT, on, sessionSnapshot } from './ipc'
import { mergeDelta } from './transcript'
import type { TranscriptSegment } from './types'

export function useTranscriptStream(): {
  segments: TranscriptSegment[]
  clear: () => void
} {
  const [segments, setSegments] = useState<TranscriptSegment[]>([])

  useEffect(() => {
    let disposed = false
    let gotBroadcast = false
    let unlisten: (() => void) | null = null

    on<TranscriptSegment[]>(EVENT.segments, (delta) => {
      gotBroadcast = true
      if (!disposed) setSegments((prev) => (delta.length === 0 ? [] : mergeDelta(prev, delta)))
    })
      .then((fn) => {
        if (disposed) fn()
        else unlisten = fn
      })
      .catch(() => {})

    sessionSnapshot()
      .then((snap) => {
        if (!disposed && !gotBroadcast) setSegments(snap)
      })
      .catch(() => {})

    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  return { segments, clear: () => setSegments([]) }
}
