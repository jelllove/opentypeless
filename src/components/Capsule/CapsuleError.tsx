import { useEffect, useState } from 'react'
import { Check, Copy, X } from 'lucide-react'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { useTranslation } from 'react-i18next'
import { useAppStore } from '../../stores/appStore'

export function CapsuleError() {
  const { t } = useTranslation()
  const pipelineError = useAppStore((s) => s.pipelineError)
  const details = useAppStore((s) => s.pipelineErrorDetails)
  const setPipelineError = useAppStore((s) => s.setPipelineError)
  const [copyState, setCopyState] = useState<'ready' | 'copying' | 'copied' | 'failed'>('ready')

  useEffect(() => {
    setCopyState('ready')
  }, [pipelineError, details])

  const text = `${pipelineError || t('capsule.errors.unknown')}${details ? `\n${details}` : ''}`
  const stopPointer = (event: React.PointerEvent) => event.stopPropagation()
  const copy = async () => {
    if (copyState === 'copying') return
    setCopyState('copying')
    try {
      await writeText(text)
      if (
        useAppStore.getState().pipelineError === pipelineError &&
        useAppStore.getState().pipelineErrorDetails === details
      )
        setCopyState('copied')
    } catch {
      if (
        useAppStore.getState().pipelineError === pipelineError &&
        useAppStore.getState().pipelineErrorDetails === details
      )
        setCopyState('failed')
    }
  }
  const copyLabel =
    copyState === 'copied'
      ? t('capsule.copiedText')
      : copyState === 'failed'
        ? t('capsule.copyFailed')
        : t('capsule.copyError', { defaultValue: 'Copy error' })
  const CopyIcon = copyState === 'copied' ? Check : Copy

  return (
    <div
      className="relative z-10 flex h-full flex-col gap-2 p-3 text-white"
      onPointerDown={stopPointer}
      onPointerUp={stopPointer}
    >
      <div className="flex shrink-0 items-center gap-2">
        <p role="alert" className="min-w-0 flex-1 truncate text-xs font-semibold">
          {pipelineError || t('capsule.errors.unknown')}
        </p>
        <button
          type="button"
          onClick={() => void copy()}
          disabled={copyState === 'copying'}
          aria-label={copyLabel}
          title={copyLabel}
          className="rounded-full border-none bg-white/10 p-1.5 hover:bg-white/20 focus-visible:outline-2 focus-visible:outline-white disabled:opacity-60"
        >
          <CopyIcon size={14} />
        </button>
        <button
          type="button"
          onClick={() => setPipelineError(null)}
          aria-label={t('capsule.dismissError', { defaultValue: 'Dismiss error' })}
          className="rounded-full border-none bg-transparent p-1.5 hover:bg-white/15 focus-visible:outline-2 focus-visible:outline-white"
        >
          <X size={14} />
        </button>
      </div>
      <p className="min-h-0 flex-1 overflow-y-auto whitespace-pre-wrap break-words rounded-lg bg-black/15 p-2 text-[11px] leading-relaxed">
        {details ||
          t('capsule.errorDetailsUnavailable', {
            defaultValue: 'No additional details. Check provider settings and try again.',
          })}
      </p>
    </div>
  )
}
