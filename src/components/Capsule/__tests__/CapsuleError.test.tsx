import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { CapsuleError } from '../CapsuleError'
import { useAppStore } from '../../../stores/appStore'

vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({
  writeText: vi.fn().mockResolvedValue(undefined),
}))
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))

beforeEach(() => {
  vi.useFakeTimers()
  useAppStore.setState(useAppStore.getInitialState())
  useAppStore
    .getState()
    .setPipelineError('STT failed', 'Azure Speech — HTTP 429\ncode: TooManyRequests')
})
afterEach(() => {
  cleanup()
  vi.useRealTimers()
  vi.clearAllMocks()
})

describe('inspectable capsule error', () => {
  it('keeps details visible until explicitly dismissed, without changing pipeline state', () => {
    render(<CapsuleError />)
    act(() => vi.advanceTimersByTime(60_000))
    expect(screen.getByText(/HTTP 429/)).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'capsule.dismissError' }))
    expect(useAppStore.getState().pipelineError).toBeNull()
    expect(useAppStore.getState().pipelineErrorDetails).toBeNull()
    expect(useAppStore.getState().pipelineState).toBe('idle')
  })

  it('copies details without triggering capsule recording controls', async () => {
    const onPointerUp = vi.fn()
    render(
      <div onPointerUp={onPointerUp}>
        <CapsuleError />
      </div>,
    )
    const button = screen.getByRole('button', { name: 'capsule.copyError' })
    fireEvent.pointerUp(button)
    await act(async () => {
      fireEvent.click(button)
    })
    expect(writeText).toHaveBeenCalledWith(
      'STT failed\nAzure Speech — HTTP 429\ncode: TooManyRequests',
    )
    expect(onPointerUp).not.toHaveBeenCalled()
    expect(screen.getByRole('button', { name: 'capsule.copiedText' })).toBeInTheDocument()
  })

  it('retains the error and offers retry if copying fails', async () => {
    vi.mocked(writeText).mockRejectedValueOnce(new Error('Clipboard busy'))
    render(<CapsuleError />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'capsule.copyError' }))
    })
    expect(screen.getByRole('button', { name: 'capsule.copyFailed' })).toBeInTheDocument()
    expect(useAppStore.getState().pipelineErrorDetails).toContain('HTTP 429')
  })
})
