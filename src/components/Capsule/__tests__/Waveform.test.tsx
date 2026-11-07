import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Waveform } from '../Waveform'
import { useAppStore } from '../../../stores/appStore'

const preferences = vi.hoisted(() => ({ reduced: false }))
vi.mock('framer-motion', () => ({ useReducedMotion: () => preferences.reduced }))
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))

beforeEach(() => {
  preferences.reduced = false
  useAppStore.setState({ audioVolume: 0 })
})
afterEach(cleanup)

function excursion(path: string) {
  const numbers = path.match(/-?\d+(?:\.\d+)?/g)?.map(Number) ?? []
  const ys = numbers.filter((_, index) => index % 2 === 1)
  return Math.max(...ys) - Math.min(...ys)
}

function signalPath() {
  return screen
    .getByRole('img', { name: 'capsule.microphoneLevel' })
    .querySelector('path')!
    .getAttribute('d')!
}

describe('microphone waveform', () => {
  it('draws a smooth centered curve rather than accumulating history from one side', () => {
    preferences.reduced = true
    useAppStore.getState().setAudioVolume(0.08)
    render(<Waveform />)
    expect(signalPath()).toContain('C')
    const svg = screen.getByRole('img', { name: 'capsule.microphoneLevel' })
    expect(svg).toHaveAttribute('viewBox', '0 0 72 20')
  })

  it('stays flat and does not schedule fake animation in silence', () => {
    const raf = vi.spyOn(window, 'requestAnimationFrame')
    render(<Waveform />)
    expect(excursion(signalPath())).toBe(0)
    expect(raf).not.toHaveBeenCalled()
    raf.mockRestore()
  })

  it('reflects louder microphone levels even with reduced motion enabled', () => {
    preferences.reduced = true
    render(<Waveform />)
    act(() => useAppStore.getState().setAudioVolume(0.02))
    const quiet = excursion(signalPath())
    expect(quiet).toBeGreaterThan(0)
    act(() => useAppStore.getState().setAudioVolume(0.15))
    expect(excursion(signalPath())).toBeGreaterThan(quiet)
    act(() => useAppStore.getState().setAudioVolume(0))
    expect(excursion(signalPath())).toBe(0)
  })

  it('moves continuously around the same center and settles when speech stops', () => {
    const frames = new Map<number, FrameRequestCallback>()
    let id = 0
    const request = vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
      frames.set(++id, callback)
      return id
    })
    const cancel = vi.spyOn(window, 'cancelAnimationFrame').mockImplementation((frame) => {
      frames.delete(frame)
    })
    const tick = (time: number) =>
      act(() => {
        const pending = [...frames.values()]
        frames.clear()
        pending.forEach((callback) => callback(time))
      })
    const { unmount } = render(<Waveform />)
    act(() => useAppStore.getState().setAudioVolume(0.1))
    tick(16)
    const first = signalPath()
    tick(32)
    expect(signalPath()).not.toBe(first)
    expect(signalPath()).toMatch(/^M 0 10/)
    expect(signalPath()).toMatch(/72\.00 10\.00$/)
    act(() => useAppStore.getState().setAudioVolume(0))
    for (let time = 48; time < 2500; time += 16) tick(time)
    expect(excursion(signalPath())).toBe(0)
    expect(frames.size).toBe(0)
    unmount()
    request.mockRestore()
    cancel.mockRestore()
  })

  it('renders repeated silence without residual motion or noise-floor jitter', () => {
    render(<Waveform />)
    act(() => useAppStore.getState().setAudioVolume(0.001))
    expect(excursion(signalPath())).toBe(0)
    act(() => useAppStore.getState().setAudioVolume(0.002))
    expect(excursion(signalPath())).toBe(0)
  })

  it('never renders non-finite coordinates', () => {
    render(<Waveform />)
    act(() => useAppStore.getState().setAudioVolume(Number.NaN))
    expect(signalPath()).not.toMatch(/NaN|Infinity/)
    expect(excursion(signalPath())).toBe(0)
  })
})
