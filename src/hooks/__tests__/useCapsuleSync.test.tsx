import { act, cleanup, render, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useCapsuleSync, type CapsuleSnapshot } from '../useCapsuleSync'
import { useAppStore } from '../../stores/appStore'

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  stop: vi.fn(),
  handler: null as null | ((event: { payload: unknown }) => void),
}))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((_event, handler) => {
    mocks.handler = handler
    return Promise.resolve(mocks.stop)
  }),
}))
vi.mock('react-i18next', () => {
  const t = (key: string) => key
  return { useTranslation: () => ({ t }) }
})

const snapshot = (patch: Partial<CapsuleSnapshot> = {}): CapsuleSnapshot => ({
  revision: 1,
  state: 'recording',
  deadline: null,
  voiceMode: 'dictate',
  error: null,
  output: null,
  finalTranscript: '',
  ...patch,
})
function Harness() {
  useCapsuleSync()
  return null
}

beforeEach(() => {
  vi.clearAllMocks()
  mocks.handler = null
  useAppStore.setState(useAppStore.getInitialState())
  mocks.invoke.mockResolvedValue(snapshot())
})
afterEach(() => {
  cleanup()
  vi.useRealTimers()
})

describe('capsule state recovery', () => {
  it('replays a recording started before the hidden renderer registered listeners', async () => {
    render(<Harness />)
    await waitFor(() => expect(useAppStore.getState().pipelineState).toBe('recording'))
    expect(mocks.invoke).toHaveBeenCalledWith('capsule_sync', {
      revision: null,
      errorVisible: false,
      outputVisible: false,
    })
  })

  it('ignores a stale idle response that arrives after a newer recording event', async () => {
    let resolve!: (value: CapsuleSnapshot) => void
    mocks.invoke.mockReturnValue(
      new Promise((done) => {
        resolve = done
      }),
    )
    render(<Harness />)
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalled())
    act(() => mocks.handler?.({ payload: snapshot({ revision: 5 }) }))
    await act(async () => resolve(snapshot({ revision: 4, state: 'idle' })))
    expect(useAppStore.getState().pipelineState).toBe('recording')
  })

  it('restores retained error and copy-ready output after a renderer reload', async () => {
    mocks.invoke.mockResolvedValue(
      snapshot({
        state: 'idle',
        error: { code: 'stt_failed', details: 'Azure Speech — HTTP 429' },
        output: { text: 'Recovered output', status: 'failed' },
      }),
    )
    render(<Harness />)
    await waitFor(() => expect(useAppStore.getState().pendingOutput?.text).toBe('Recovered output'))
    expect(useAppStore.getState().pipelineErrorDetails).toContain('HTTP 429')
  })

  it('does not resurrect a dismissed error at the same native revision', async () => {
    vi.useFakeTimers()
    mocks.invoke.mockResolvedValue(
      snapshot({ state: 'idle', error: { code: 'stt_failed', details: 'HTTP 429' } }),
    )
    render(<Harness />)
    await act(async () => {})
    act(() => useAppStore.getState().setPipelineError(null))
    await act(async () => vi.advanceTimersByTime(1000))
    expect(mocks.invoke).toHaveBeenLastCalledWith('capsule_sync', {
      revision: 1,
      errorVisible: false,
      outputVisible: false,
    })
    expect(useAppStore.getState().pipelineError).toBeNull()
  })

  it('recovers a missed stop event by polling without replacing an unchanged pending result', async () => {
    vi.useFakeTimers()
    mocks.invoke.mockResolvedValue(snapshot())
    render(<Harness />)
    await act(async () => {})
    mocks.invoke.mockResolvedValue(
      snapshot({ revision: 2, state: 'idle', output: { text: 'Ready', status: 'inserted' } }),
    )
    await act(async () => vi.advanceTimersByTime(1000))
    const result = useAppStore.getState().pendingOutput
    expect(useAppStore.getState().pipelineState).toBe('idle')
    mocks.invoke.mockResolvedValue(
      snapshot({ revision: 3, state: 'idle', output: { text: 'Ready', status: 'inserted' } }),
    )
    await act(async () => vi.advanceTimersByTime(1000))
    expect(useAppStore.getState().pendingOutput).toBe(result)
  })

  it('resynchronizes on visibility resume and unregisters when unmounted', async () => {
    const { unmount } = render(<Harness />)
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledTimes(1))
    mocks.invoke.mockResolvedValue(snapshot({ revision: 2, state: 'transcribing' }))
    await act(async () => document.dispatchEvent(new Event('visibilitychange')))
    expect(useAppStore.getState().pipelineState).toBe('transcribing')
    unmount()
    await act(async () => {})
    expect(mocks.stop).toHaveBeenCalled()
  })
})
