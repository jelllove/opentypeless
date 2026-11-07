import React from 'react'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { CapsuleComplete } from '../CapsuleComplete'
import { useAppStore } from '../../../stores/appStore'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'

vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({
  writeText: vi.fn().mockResolvedValue(undefined),
}))
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))
vi.mock('framer-motion', () => ({
  motion: new Proxy(
    {},
    {
      get:
        (_target, tag: string) =>
        ({ children, ...props }: React.HTMLAttributes<HTMLElement>) =>
          React.createElement(tag, props, children),
    },
  ),
}))

beforeEach(() => {
  vi.useFakeTimers()
  useAppStore.setState(useAppStore.getInitialState())
  useAppStore
    .getState()
    .setPendingOutput({ text: 'Final cleaned text', status: 'clipboard_fallback' })
})
afterEach(() => {
  cleanup()
  vi.useRealTimers()
  vi.clearAllMocks()
})

describe('copy-ready capsule', () => {
  it('retains fallback text until copied or dismissed', () => {
    render(<CapsuleComplete />)
    act(() => vi.advanceTimersByTime(30_000))
    expect(screen.getByRole('button', { name: 'capsule.copyText' })).toBeInTheDocument()
    expect(useAppStore.getState().pendingOutput?.text).toBe('Final cleaned text')
    fireEvent.click(screen.getByRole('button', { name: 'capsule.dismissResult' }))
    expect(useAppStore.getState().pendingOutput).toBeNull()
  })

  it('copies the final text and does not bubble pointer events', async () => {
    const onPointerDown = vi.fn()
    render(
      <div onPointerDown={onPointerDown}>
        <CapsuleComplete />
      </div>,
    )
    const button = screen.getByRole('button', { name: 'capsule.copyText' })
    fireEvent.pointerDown(button)
    await act(async () => {
      fireEvent.click(button)
    })
    expect(writeText).toHaveBeenCalledWith('Final cleaned text')
    expect(onPointerDown).not.toHaveBeenCalled()
    expect(screen.getByRole('button', { name: 'capsule.copiedText' })).toBeInTheDocument()
  })

  it('keeps a retryable copy action when clipboard write fails', async () => {
    vi.mocked(writeText).mockRejectedValueOnce(new Error('Clipboard busy'))
    render(<CapsuleComplete />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'capsule.copyText' }))
    })
    expect(useAppStore.getState().pendingOutput?.text).toBe('Final cleaned text')
    expect(screen.getByRole('button', { name: 'capsule.copyFailed' })).toBeInTheDocument()
  })

  it('expires a successful insertion without changing backend state', () => {
    useAppStore.getState().setPendingOutput({ text: 'Inserted text', status: 'inserted' })
    render(<CapsuleComplete />)
    act(() => vi.advanceTimersByTime(8000))
    expect(useAppStore.getState().pendingOutput).toBeNull()
    expect(useAppStore.getState().pipelineState).toBe('idle')
  })
})
