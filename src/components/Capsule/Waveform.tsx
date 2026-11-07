import { useEffect, useRef } from 'react'
import { useReducedMotion } from 'framer-motion'
import { useTranslation } from 'react-i18next'
import { useAppStore } from '../../stores/appStore'

const WIDTH = 72
const HEIGHT = 20
const CENTER = HEIGHT / 2
const NOISE_FLOOR = 0.003

function signalLevel(rms: number): number {
  if (!Number.isFinite(rms) || rms <= NOISE_FLOOR) return 0
  return Math.min(1, Math.sqrt((rms - NOISE_FLOOR) / 0.25))
}

// Cubic curves form a rounded, tapered wave anchored around its centerline.
function wavePath(level: number, phase: number): string {
  const count = 24
  const points = Array.from({ length: count + 1 }, (_, index) => {
    const t = index / count
    const envelope = Math.sin(Math.PI * t) ** 1.5
    return { x: t * WIDTH, y: CENTER + Math.sin(t * Math.PI * 5 + phase) * envelope * level * 8 }
  })
  let path = `M 0 ${CENTER}`
  for (let index = 0; index < count; index++) {
    const p0 = points[Math.max(0, index - 1)]
    const p1 = points[index]
    const p2 = points[index + 1]
    const p3 = points[Math.min(count, index + 2)]
    path += ` C ${(p1.x + (p2.x - p0.x) / 6).toFixed(2)} ${(p1.y + (p2.y - p0.y) / 6).toFixed(2)} ${(p2.x - (p3.x - p1.x) / 6).toFixed(2)} ${(p2.y - (p3.y - p1.y) / 6).toFixed(2)} ${p2.x.toFixed(2)} ${p2.y.toFixed(2)}`
  }
  return path
}

export function Waveform() {
  const volume = useAppStore((state) => state.audioVolume)
  const level = signalLevel(volume)
  const { t } = useTranslation()
  const reduced = useReducedMotion()
  const pathRef = useRef<SVGPathElement>(null)
  const target = useRef(level)
  target.current = level
  const speaking = level > 0
  const amplitudeRef = useRef(0)
  const phaseRef = useRef(0)

  useEffect(() => {
    if (reduced) pathRef.current?.setAttribute('d', wavePath(level, 0))
  }, [reduced, level])

  useEffect(() => {
    if (reduced) return
    let frame = 0
    let phase = phaseRef.current
    let amplitude = amplitudeRef.current
    let previousTime = 0
    const animate = (time: number) => {
      const delta = previousTime ? Math.min(time - previousTime, 50) : 16
      previousTime = time
      const next = target.current
      amplitude += (next - amplitude) * (1 - Math.exp(-delta / (next > amplitude ? 55 : 110)))
      if (amplitude < 0.005 && next === 0) {
        amplitudeRef.current = 0
        pathRef.current?.setAttribute('d', wavePath(0, 0))
        return
      }
      amplitudeRef.current = amplitude
      phase += delta * 0.006
      phaseRef.current = phase
      pathRef.current?.setAttribute('d', wavePath(amplitude, phase))
      frame = requestAnimationFrame(animate)
    }
    if (speaking || amplitudeRef.current > 0.005) frame = requestAnimationFrame(animate)
    return () => cancelAnimationFrame(frame)
  }, [reduced, speaking])

  return (
    <svg
      role="img"
      aria-label={t('capsule.microphoneLevel')}
      viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
      width={WIDTH}
      height={HEIGHT}
      className="block w-full max-w-[72px] text-white/90"
    >
      <title>{t('capsule.microphoneLevelValue', { level: Math.round(level * 100) })}</title>
      <path
        ref={pathRef}
        d={wavePath(reduced ? level : 0, 0)}
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}
