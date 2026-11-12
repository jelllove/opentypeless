import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import { LazyStore } from '@tauri-apps/plugin-store'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { useAppStore } from './appStore'

export const UPDATE_CHECK_INTERVAL = 4 * 60 * 60 * 1000
export type UpdatePhase =
  | 'idle'
  | 'checking'
  | 'upToDate'
  | 'downloading'
  | 'ready'
  | 'installing'
  | 'error'

interface UpdateState {
  phase: UpdatePhase
  automatic: boolean
  loaded: boolean
  preferenceSaving: boolean
  preferenceError: boolean
  version: string | null
  percent: number | null
  dismissed: boolean
}

const preferences = new LazyStore('update-settings.json', {
  defaults: { automatic: true },
  autoSave: false,
})
let pending: Update | null = null
let checking = false
let saving = false
let loading: Promise<void> | null = null

export const useUpdateStore = create<UpdateState>(() => ({
  phase: 'idle',
  automatic: true,
  loaded: false,
  preferenceSaving: false,
  preferenceError: false,
  version: null,
  percent: null,
  dismissed: false,
}))

export function loadUpdatePreferences(): Promise<void> {
  if (!loading) {
    loading = (async () => {
      try {
        const automatic = (await preferences.get<boolean>('automatic')) !== false
        useUpdateStore.setState({ automatic, loaded: true })
      } catch {
        // Fail closed: do not download automatically if the preference cannot be read.
        useUpdateStore.setState({ automatic: false, loaded: true, preferenceError: true })
      }
    })()
  }
  return loading
}

export async function setAutomaticUpdates(automatic: boolean): Promise<void> {
  if (saving) return
  saving = true
  useUpdateStore.setState({ preferenceSaving: true, preferenceError: false })
  const previous = useUpdateStore.getState().automatic
  let saved = false
  try {
    await preferences.set('automatic', automatic)
    await preferences.save()
    saved = true
    useUpdateStore.setState({ automatic })
  } catch {
    await preferences.set('automatic', previous).catch(() => {})
    // Do not discard a ready/ongoing download when only preference persistence failed.
    useUpdateStore.setState({ preferenceError: true })
  } finally {
    saving = false
    useUpdateStore.setState({ preferenceSaving: false })
  }
  if (saved && automatic) await checkForUpdates()
}

export async function checkForUpdates(): Promise<void> {
  if (import.meta.env.DEV || checking) return
  const { phase } = useUpdateStore.getState()
  if (phase === 'installing' || phase === 'ready') return
  checking = true
  useUpdateStore.setState({ phase: 'checking', percent: null, dismissed: false })
  try {
    if (pending) {
      await pending.close()
      pending = null
    }
    const update = await check({ timeout: 30_000, allowDowngrades: false })
    if (!update) {
      useUpdateStore.setState({ phase: 'upToDate', version: null })
      return
    }
    pending = update
    useUpdateStore.setState({ phase: 'downloading', version: update.version })
    let downloaded = 0
    let total: number | undefined
    await update.download(
      (event) => {
        if (event.event === 'Started') total = event.data.contentLength
        if (event.event === 'Progress') downloaded += event.data.chunkLength
        // Finished only means transport finished. Ready waits for signature verification.
        if (total) useUpdateStore.setState({ percent: Math.min(100, (downloaded / total) * 100) })
      },
      { timeout: 10 * 60 * 1000 },
    )
    useUpdateStore.setState({ phase: 'ready', percent: 100 })
  } catch (error) {
    console.warn('Update check or verified download failed:', error)
    if (pending) await pending.close().catch(() => {})
    pending = null
    useUpdateStore.setState({ phase: 'error', percent: null })
  } finally {
    checking = false
  }
}

export async function installUpdate(): Promise<void> {
  if (
    !pending ||
    useUpdateStore.getState().phase !== 'ready' ||
    useAppStore.getState().pipelineState !== 'idle'
  )
    return
  useUpdateStore.setState({ phase: 'installing' })
  try {
    // Native gate rechecks real dictation/Ask activity and blocks new hotkey starts.
    await invoke('begin_update_install')
    await pending.install()
    await invoke('finish_update_install')
  } catch (error) {
    await invoke('cancel_update_install').catch(() => {})
    console.warn('Update installation failed:', error)
    useUpdateStore.setState({ phase: 'error' })
    await pending?.close().catch(() => {})
    pending = null
  }
}
