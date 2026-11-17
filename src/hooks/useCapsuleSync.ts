import { useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useTranslation } from 'react-i18next'
import {
  useAppStore,
  type PendingOutput,
  type PipelineState,
  type RecordingDeadlineSnapshot,
  type VoiceMode,
} from '../stores/appStore'
import { capsuleErrorKeyFromPayload, type PipelineErrorPayload } from '../lib/capsuleError'

export interface CapsuleSnapshot {
  revision: number
  state: PipelineState
  deadline: RecordingDeadlineSnapshot | null
  voiceMode: VoiceMode | null
  error: PipelineErrorPayload | null
  output: PendingOutput | null
  finalTranscript: string
}

// State is replayed after listeners are ready, on visibility/resume and periodically.
// This also acknowledges a live renderer so native recovery never reloads a healthy UI.
export function useCapsuleSync() {
  const { t } = useTranslation()
  useEffect(() => {
    let cancelled = false
    let revision: number | null = null
    let inFlight = false
    const apply = (snapshot: CapsuleSnapshot) => {
      if (cancelled || (revision !== null && snapshot.revision <= revision)) return
      revision = snapshot.revision
      const store = useAppStore.getState()
      const nextError = snapshot.error
      const details = nextError && typeof nextError !== 'string' ? nextError.details : null
      const safeDetails =
        details && (details.startsWith('Azure Speech') || /^HTTP \d{3}$/.test(details))
          ? details
          : null
      const starting =
        snapshot.state === 'preparing' ||
        (snapshot.state === 'ask_recording' && store.pipelineState !== 'ask_recording')
      if (starting) store.resetRecording()
      store.setPipelineState(snapshot.state)
      store.setRecordingDeadline(snapshot.deadline)
      store.setActiveVoiceMode(snapshot.voiceMode)
      store.setAudioVolume(0)
      store.setFinalTranscript(snapshot.finalTranscript)
      if (JSON.stringify(store.pendingOutput) !== JSON.stringify(snapshot.output)) {
        store.setPendingOutput(snapshot.output)
      }
      if (snapshot.output) store.setPolishedText(snapshot.output.text)
      store.setPipelineError(
        nextError ? t(`capsule.errors.${capsuleErrorKeyFromPayload(nextError)}`) : null,
        safeDetails,
      )
    }
    const sync = async () => {
      if (cancelled || inFlight) return
      inFlight = true
      try {
        const store = useAppStore.getState()
        const snapshot = await invoke<CapsuleSnapshot>('capsule_sync', {
          revision,
          errorVisible: store.pipelineError !== null,
          outputVisible: store.pendingOutput !== null,
        })
        apply(snapshot)
      } catch (error) {
        console.warn('Capsule synchronization failed:', error)
      } finally {
        inFlight = false
      }
    }
    const unlisten = listen<CapsuleSnapshot>('capsule:sync', (event) => apply(event.payload))
    void unlisten
      .then(() => sync())
      .catch((error) => console.warn('Capsule recovery listener failed:', error))
    const timer = setInterval(() => void sync(), 1000)
    const resume = () => void sync()
    document.addEventListener('visibilitychange', resume)
    window.addEventListener('focus', resume)
    return () => {
      cancelled = true
      clearInterval(timer)
      document.removeEventListener('visibilitychange', resume)
      window.removeEventListener('focus', resume)
      void unlisten.then((stop) => stop()).catch(() => {})
    }
  }, [t])
}
