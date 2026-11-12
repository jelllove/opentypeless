import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { UpdatePrompt } from '../UpdatePrompt'
import { useAppStore } from '../../stores/appStore'
import { useUpdateStore } from '../../stores/updateStore'

const mocks = vi.hoisted(() => ({ check: vi.fn(), install: vi.fn(), load: vi.fn(), set: vi.fn() }))
vi.mock('../../stores/updateStore', async (original) => ({
  ...(await original<typeof import('../../stores/updateStore')>()),
  checkForUpdates: mocks.check,
  installUpdate: mocks.install,
  loadUpdatePreferences: mocks.load,
  setAutomaticUpdates: mocks.set,
}))
vi.mock('@tauri-apps/plugin-store', () => ({ LazyStore: class {} }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }))
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))

beforeEach(() => {
  vi.stubEnv('DEV', false)
  useUpdateStore.setState(useUpdateStore.getInitialState())
  useAppStore.setState(useAppStore.getInitialState())
  mocks.load.mockResolvedValue(undefined)
})
afterEach(() => {
  cleanup()
  vi.clearAllMocks()
  vi.useRealTimers()
  vi.unstubAllEnvs()
})

describe('UpdatePrompt', () => {
  it('checks automatically at startup and every four hours when enabled', async () => {
    vi.useFakeTimers()
    render(<UpdatePrompt />)
    await act(async () => {})
    expect(mocks.check).toHaveBeenCalledTimes(1)
    await act(async () => vi.advanceTimersByTime(4 * 60 * 60 * 1000))
    expect(mocks.check).toHaveBeenCalledTimes(2)
    useUpdateStore.setState({ automatic: false })
    await act(async () => vi.advanceTimersByTime(4 * 60 * 60 * 1000))
    expect(mocks.check).toHaveBeenCalledTimes(2)
  })

  it('keeps quiet when current or a background check fails', async () => {
    render(<UpdatePrompt />)
    await waitFor(() => expect(mocks.load).toHaveBeenCalled())
    act(() => useUpdateStore.setState({ phase: 'error' }))
    expect(screen.queryByRole('region')).toBeNull()
  })

  it('offers explicit restart only for ready updates and disables it while recording', () => {
    useUpdateStore.setState({ phase: 'ready', version: '1.4.1' })
    useAppStore.setState({ pipelineState: 'recording' })
    render(<UpdatePrompt />)
    const button = screen.getByRole('button', { name: 'updates.restart' })
    expect(button).toBeDisabled()
    act(() => useAppStore.setState({ pipelineState: 'idle' }))
    fireEvent.click(button)
    expect(mocks.install).toHaveBeenCalledTimes(1)
  })

  it('exposes manual check and immediate preference changes in About', () => {
    useUpdateStore.setState({ loaded: true })
    render(<UpdatePrompt settings />)
    fireEvent.click(screen.getByRole('checkbox', { name: 'updates.automatic' }))
    expect(mocks.set).toHaveBeenCalledWith(false)
    fireEvent.click(screen.getByRole('button', { name: 'updates.checkNow' }))
    expect(mocks.check).toHaveBeenCalled()
  })
})
