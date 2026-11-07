import { useTranslation } from 'react-i18next'
import { Check, Copy, X, AlertCircle } from 'lucide-react'
import { useEffect, useState } from 'react'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { useAppStore } from '../../stores/appStore'

export function CapsuleComplete() {
  const { t } = useTranslation()
  const result = useAppStore((s) => s.pendingOutput)
  const clearResult = useAppStore((s) => s.setPendingOutput)
  const [copyState, setCopyState] = useState<'ready' | 'copying' | 'copied' | 'failed'>('ready')

  useEffect(() => {
    setCopyState('ready')
  }, [result])

  useEffect(() => {
    if (
      !result ||
      result.status !== 'inserted' ||
      copyState === 'failed' ||
      copyState === 'copying'
    )
      return
    const timer = setTimeout(() => {
      if (useAppStore.getState().pendingOutput === result) clearResult(null)
    }, 8000)
    return () => clearTimeout(timer)
  }, [result, clearResult, copyState])

  const stopPointer = (event: React.PointerEvent) => event.stopPropagation()
  const label =
    copyState === 'copied'
      ? 'capsule.copiedText'
      : copyState === 'failed'
        ? 'capsule.copyFailed'
        : 'capsule.copyText'
  const Icon = copyState === 'copied' ? Check : copyState === 'failed' ? AlertCircle : Copy

  const copy = async () => {
    if (!result || copyState === 'copying') return
    setCopyState('copying')
    try {
      await writeText(result.text)
      if (useAppStore.getState().pendingOutput === result) setCopyState('copied')
    } catch {
      if (useAppStore.getState().pendingOutput === result) setCopyState('failed')
    }
  }

  if (!result)
    return (
      <div className="relative z-10 flex h-9 items-center gap-1.5 px-3 text-white/80">
        <span className="text-[11px]">{t('capsule.done')}</span>
      </div>
    )

  return (
    <div className="relative z-10 flex h-9 items-center gap-1 px-2">
      <button
        type="button"
        onPointerDown={stopPointer}
        onPointerUp={stopPointer}
        onClick={(event) => {
          event.stopPropagation()
          void copy()
        }}
        disabled={copyState === 'copying'}
        aria-label={t(label)}
        title={result.text}
        className="flex min-w-0 flex-1 items-center justify-center gap-2 rounded-full border-none bg-white/10 px-3 py-1.5 text-white transition-colors hover:bg-white/20 focus-visible:outline-2 focus-visible:outline-white disabled:opacity-60"
      >
        <Icon size={14} className="shrink-0" />
        <span className="truncate text-[11px] font-medium">{t(label)}</span>
      </button>
      <button
        type="button"
        onPointerDown={stopPointer}
        onPointerUp={stopPointer}
        onClick={(event) => {
          event.stopPropagation()
          clearResult(null)
        }}
        aria-label={t('capsule.dismissResult')}
        className="shrink-0 rounded-full border-none bg-transparent p-1.5 text-white/65 hover:bg-white/15 hover:text-white focus-visible:outline-2 focus-visible:outline-white"
      >
        <X size={12} />
      </button>
    </div>
  )
}
