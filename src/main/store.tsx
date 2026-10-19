import { createContext, type ReactNode, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react'
import {
  type AppInfo,
  AudioSourceKind,
  type AudioStatus,
  appInfo,
  audioDevices,
  type CmdPermissionError,
  EVENT,
  formatCmdError,
  type HotkeyFailure,
  type HotkeyInfo,
  hotkeyFailures,
  type InputDevice,
  type OverlayConfig,
  on,
  overlayConfigGet,
  overlayConfigSet,
  overlayIsVisible,
  overlaySetMode,
  overlayToggle,
  type PermissionSnapshot,
  type Preferences,
  type PrefsPatch,
  permissionOf,
  permissionStatus,
  prefsGet,
  prefsSet,
  sessionClear,
  sessionStart,
  sessionStatus,
  sessionStop,
  shortcutsGet,
  Theme,
  TranslateEngine,
} from '@/shared/ipc'
import { withTimeout } from '@/shared/timeout'
import { OverlayMode, TaskStatus } from '@/shared/types'

export type EndpointConfig = Preferences['endpoint']

interface Store {
  info: AppInfo | null

  prefsReady: boolean
  audioSource: AudioSourceKind
  device: string
  translateEngine: TranslateEngine
  sourceLang: string
  theme: Theme
  sidebarVisible: boolean
  endpoint: EndpointConfig

  status: TaskStatus
  activeDevice: string | null
  audio: AudioStatus | null

  devices: InputDevice[]
  deviceError: string | null

  overlayCfg: OverlayConfig | null
  overlayVisible: boolean

  hotkeys: HotkeyInfo[]
  hotkeyProblems: HotkeyFailure[]

  onboardingDone: boolean
  completeOnboarding: () => Promise<void>

  permissions: PermissionSnapshot | null
  refreshPermission: () => Promise<void>
  permissionError: CmdPermissionError | null
  setPermissionError: (e: CmdPermissionError | null) => void

  error: string | null
  setError: (e: string | null) => void
  pending: boolean

  toggleSession: () => Promise<void>
  clearAll: () => Promise<void>
  setAudioSource: (v: AudioSourceKind) => void
  setDevice: (v: string) => void
  setTranslateEngine: (v: TranslateEngine) => void
  setSourceLang: (v: string) => void
  changeTheme: (v: Theme) => Promise<void>
  toggleSidebar: () => void
  setEndpoint: (v: EndpointConfig) => Promise<void>
  updateOverlay: (patch: Partial<OverlayConfig>) => Promise<void>
  toggleOverlay: () => Promise<void>
  toggleMode: () => Promise<void>
  refreshDevices: () => Promise<void>
  refreshHotkeys: () => Promise<void>
}

const STOP_TIMEOUT_MS = 8_000

const START_TIMEOUT_MS = 60_000

const Ctx = createContext<Store | null>(null)

export function useStore(): Store {
  const s = useContext(Ctx)
  if (!s) throw new Error('useStore 必须在 <StoreProvider> 内使用')
  return s
}

export function StoreProvider({ children }: { children: ReactNode }) {
  const [info, setInfo] = useState<AppInfo | null>(null)
  const [prefsReady, setPrefsReady] = useState(false)

  const [audioSource, setAudioSource] = useState<AudioSourceKind>(AudioSourceKind.Loopback)
  const [device, setDevice] = useState('')
  const [translateEngine, setTranslateEngine] = useState<TranslateEngine>(TranslateEngine.LocalNmt)
  const [sourceLang, setSourceLang] = useState('en')
  const [theme, setTheme] = useState<Theme>(Theme.System)
  const [sidebarVisible, setSidebarVisible] = useState(true)
  const [endpoint, setEndpoint] = useState<EndpointConfig>({
    trailingSilenceMs: 800,
    maxUtteranceMs: 8000,
  })

  const [status, setStatus] = useState<TaskStatus>(TaskStatus.Ready)
  const [activeDevice, setActiveDevice] = useState<string | null>(null)
  const [audio, setAudio] = useState<AudioStatus | null>(null)

  const [devices, setDevices] = useState<InputDevice[]>([])
  const [deviceError, setDeviceError] = useState<string | null>(null)

  const [overlayCfg, setOverlayCfg] = useState<OverlayConfig | null>(null)
  const [overlayVisible, setOverlayVisible] = useState(false)

  const [hotkeys, setHotkeys] = useState<HotkeyInfo[]>([])
  const [hotkeyProblems, setHotkeyProblems] = useState<HotkeyFailure[]>([])

  const [onboardingDone, setOnboardingDone] = useState(false)
  const [permissions, setPermissions] = useState<PermissionSnapshot | null>(null)
  const [permissionError, setPermissionError] = useState<CmdPermissionError | null>(null)

  const [error, setError] = useState<string | null>(null)
  const [pending, setPending] = useState(false)
  const pendingRef = useRef(false)

  useEffect(() => {
    appInfo()
      .then(setInfo)
      .catch((e) => setError(formatCmdError(e)))
    hotkeyFailures()
      .then(setHotkeyProblems)
      .catch(() => {})
    shortcutsGet()
      .then(setHotkeys)
      .catch(() => {})

    const unlisten: Array<() => void> = []
    let disposed = false
    const keep = (fn: () => void) => {
      if (disposed) fn()
      else unlisten.push(fn)
    }

    on<TaskStatus>(EVENT.taskState, (s) => {
      setStatus(s)
      if (s !== TaskStatus.Capturing) setAudio(null)
    }).then(keep)

    on<AudioStatus>(EVENT.audio, setAudio).then(keep)

    on<OverlayConfig>(EVENT.overlayState, setOverlayCfg).then(keep)

    on<boolean>(EVENT.overlayVisibility, setOverlayVisible).then(keep)

    return () => {
      disposed = true
      for (const fn of unlisten) fn()
    }
  }, [])

  useEffect(() => {
    prefsGet()
      .then((p) => {
        setAudioSource(p.audioSource as AudioSourceKind)
        setTranslateEngine(p.translateEngine)
        setSourceLang(p.sourceLang)
        setTheme(p.theme)
        setSidebarVisible(p.sidebarVisible)
        setEndpoint(p.endpoint)
        setOnboardingDone(p.onboardingDone)
        if (p.deviceId) setDevice(p.deviceId)
      })
      .catch(() => {})
      .finally(() => setPrefsReady(true))

    overlayConfigGet()
      .then(setOverlayCfg)
      .catch(() => {})
    overlayIsVisible()
      .then(setOverlayVisible)
      .catch(() => {})
  }, [])

  const refreshPermission = useCallback(async () => {
    try {
      setPermissions(await permissionStatus())
    } catch {
      setPermissions(null)
    }
  }, [])

  useEffect(() => {
    void refreshPermission()
    const onFocus = () => void refreshPermission()
    window.addEventListener('focus', onFocus)
    return () => window.removeEventListener('focus', onFocus)
  }, [refreshPermission])

  const applyTheme = useCallback((pref: Theme) => {
    const systemDark = window.matchMedia('(prefers-color-scheme: dark)').matches
    const dark = pref === Theme.Dark || (pref === Theme.System && systemDark)
    document.documentElement.classList.toggle('theme-light', !dark)
    try {
      localStorage.setItem('audioscript.theme', pref)
    } catch {}
  }, [])

  useEffect(() => {
    applyTheme(theme)
    if (theme !== Theme.System) return
    const mq = window.matchMedia('(prefers-color-scheme: dark)')
    const onChange = () => applyTheme(Theme.System)
    mq.addEventListener('change', onChange)
    return () => mq.removeEventListener('change', onChange)
  }, [theme, applyTheme])

  const patchPrefs = useCallback(async (patch: PrefsPatch) => {
    try {
      const p = await prefsSet(patch)
      if (patch.theme !== undefined) setTheme(p.theme)
      if (patch.endpoint !== undefined) setEndpoint(p.endpoint)
    } catch (e) {
      setError(formatCmdError(e))
    }
  }, [])

  const completeOnboarding = useCallback(async () => {
    setOnboardingDone(true)
    try {
      await prefsSet({ onboardingDone: true })
    } catch (e) {
      setError(formatCmdError(e))
    }
  }, [])

  const refreshDevices = useCallback(async () => {
    try {
      const list = await audioDevices(audioSource)
      setDevices(list)
      setDeviceError(null)
      setDevice((cur) => cur || (list.find((d) => d.isDefault)?.id ?? ''))
    } catch (e) {
      setDeviceError(String(e))
    }
  }, [audioSource])

  useEffect(() => {
    if (!prefsReady) return
    refreshDevices()
  }, [prefsReady, refreshDevices])

  const toggleSession = useCallback(async () => {
    if (pendingRef.current) return
    pendingRef.current = true
    setPending(true)
    setError(null)
    setPermissionError(null)
    try {
      if (status === TaskStatus.Capturing) {
        setStatus(await withTimeout(sessionStop(), STOP_TIMEOUT_MS, '停止请求超时：请重试，或使用托盘「停止转写」'))
        setActiveDevice(null)
      } else {
        setStatus(
          await withTimeout(
            sessionStart(device || undefined, translateEngine, sourceLang, audioSource),
            START_TIMEOUT_MS,
            '开始转写超时，请重试',
          ),
        )
        setActiveDevice((await sessionStatus()).device ?? null)
        void refreshPermission()
      }
    } catch (e) {
      const denied = permissionOf(e)
      if (denied) {
        setPermissionError(denied)
        void refreshPermission()
      } else {
        setError(formatCmdError(e))
      }
      sessionStatus()
        .then((i) => setStatus(i.status))
        .catch(() => {})
    } finally {
      pendingRef.current = false
      setPending(false)
    }
  }, [status, device, translateEngine, sourceLang, audioSource, refreshPermission])

  const clearAll = useCallback(async () => {
    await sessionClear()
  }, [])

  const updateOverlay = useCallback(
    async (patch: Partial<OverlayConfig>) => {
      if (!overlayCfg) return
      try {
        setOverlayCfg(await overlayConfigSet({ ...overlayCfg, ...patch }))
      } catch (e) {
        setError(formatCmdError(e))
      }
    },
    [overlayCfg],
  )

  const toggleOverlay = useCallback(async () => {
    try {
      setOverlayVisible(await overlayToggle())
    } catch (e) {
      setError(formatCmdError(e))
    }
  }, [])

  const toggleMode = useCallback(async () => {
    try {
      const current = overlayCfg?.mode ?? OverlayMode.Edit
      const next = current === OverlayMode.Locked ? OverlayMode.Edit : OverlayMode.Locked
      await overlaySetMode(next)
    } catch (e) {
      setError(formatCmdError(e))
    }
  }, [overlayCfg])

  const refreshHotkeys = useCallback(async () => {
    try {
      setHotkeys(await shortcutsGet())
      setHotkeyProblems(await hotkeyFailures())
    } catch (e) {
      setError(formatCmdError(e))
    }
  }, [])

  const changeTheme = useCallback(
    async (next: Theme) => {
      setTheme(next)
      await patchPrefs({ theme: next })
    },
    [patchPrefs],
  )

  const toggleSidebar = useCallback(() => {
    const next = !sidebarVisible
    setSidebarVisible(next)
    void patchPrefs({ sidebarVisible: next })
  }, [patchPrefs, sidebarVisible])

  const changeAudioSource = useCallback(
    (v: AudioSourceKind) => {
      setAudioSource(v)
      setDevice('')
      void patchPrefs({ audioSource: v, deviceId: '' })
    },
    [patchPrefs],
  )

  const changeDevice = useCallback(
    (v: string) => {
      setDevice(v)
      void patchPrefs({ deviceId: v })
    },
    [patchPrefs],
  )

  const changeTranslateEngine = useCallback(
    (v: TranslateEngine) => {
      setTranslateEngine(v)
      void patchPrefs({ translateEngine: v })
    },
    [patchPrefs],
  )

  const changeSourceLang = useCallback(
    (v: string) => {
      setSourceLang(v)
      void patchPrefs({ sourceLang: v })
    },
    [patchPrefs],
  )

  const changeEndpoint = useCallback(
    async (v: EndpointConfig) => {
      setEndpoint(v)
      await patchPrefs({ endpoint: v })
    },
    [patchPrefs],
  )

  const value = useMemo<Store>(
    () => ({
      info,
      prefsReady,
      audioSource,
      device,
      translateEngine,
      sourceLang,
      theme,
      sidebarVisible,
      endpoint,
      status,
      activeDevice,
      audio,
      devices,
      deviceError,
      overlayCfg,
      overlayVisible,
      hotkeys,
      hotkeyProblems,
      onboardingDone,
      completeOnboarding,
      permissions,
      refreshPermission,
      permissionError,
      setPermissionError,
      error,
      setError,
      pending,
      toggleSession,
      clearAll,
      setAudioSource: changeAudioSource,
      setDevice: changeDevice,
      setTranslateEngine: changeTranslateEngine,
      setSourceLang: changeSourceLang,
      changeTheme,
      toggleSidebar,
      setEndpoint: changeEndpoint,
      updateOverlay,
      toggleOverlay,
      toggleMode,
      refreshDevices,
      refreshHotkeys,
    }),
    [
      info,
      prefsReady,
      audioSource,
      device,
      translateEngine,
      sourceLang,
      theme,
      sidebarVisible,
      endpoint,
      status,
      activeDevice,
      audio,
      devices,
      deviceError,
      overlayCfg,
      overlayVisible,
      hotkeys,
      hotkeyProblems,
      onboardingDone,
      completeOnboarding,
      permissions,
      refreshPermission,
      permissionError,
      error,
      pending,
      toggleSession,
      clearAll,
      changeAudioSource,
      changeDevice,
      changeTranslateEngine,
      changeSourceLang,
      changeTheme,
      toggleSidebar,
      changeEndpoint,
      updateOverlay,
      toggleOverlay,
      toggleMode,
      refreshDevices,
      refreshHotkeys,
    ],
  )

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>
}
