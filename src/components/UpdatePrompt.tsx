import { useEffect } from 'react'
import { Download, RefreshCw, X } from 'lucide-react'
import { openUrl } from '@tauri-apps/plugin-opener'
import { useTranslation } from 'react-i18next'
import { useAppStore } from '../stores/appStore'
import {
  checkForUpdates,
  installUpdate,
  loadUpdatePreferences,
  setAutomaticUpdates,
  UPDATE_CHECK_INTERVAL,
  useUpdateStore,
} from '../stores/updateStore'
import { APP_REPO_URL } from '../lib/constants'

export function UpdatePrompt({ settings = false }: { settings?: boolean }) {
  const { t } = useTranslation()
  const {
    phase,
    automatic,
    loaded,
    preferenceSaving,
    preferenceError,
    version,
    percent,
    dismissed,
  } = useUpdateStore()
  const busy = useAppStore((s) => s.pipelineState !== 'idle')

  useEffect(() => {
    if (settings || import.meta.env.DEV) return
    let cancelled = false
    const automaticCheck = () => {
      if (!cancelled && useUpdateStore.getState().automatic) void checkForUpdates()
    }
    void loadUpdatePreferences().then(automaticCheck)
    const timer = setInterval(automaticCheck, UPDATE_CHECK_INTERVAL)
    return () => {
      cancelled = true
      clearInterval(timer)
    }
  }, [settings])

  if (!settings && (dismissed || !['downloading', 'ready', 'installing'].includes(phase)))
    return null

  const working = ['checking', 'downloading', 'installing'].includes(phase)
  const ready = phase === 'ready'
  return (
    <section
      className={
        settings
          ? 'rounded-[10px] border border-border p-3'
          : 'fixed right-4 bottom-4 z-[9998] w-[min(360px,calc(100vw-32px))] rounded-[8px] border border-border bg-bg-secondary p-4 shadow-xl'
      }
      aria-label={t('updates.title')}
    >
      <div className="flex items-center justify-between gap-3">
        <h2 className="text-[14px] font-semibold text-text-primary">{t('updates.title')}</h2>
        {!settings && phase !== 'installing' && (
          <button
            type="button"
            onClick={() => useUpdateStore.setState({ dismissed: true })}
            aria-label={t('updates.dismiss')}
            className="rounded p-1 text-text-secondary hover:text-text-primary"
          >
            <X size={15} />
          </button>
        )}
      </div>
      {settings && (
        <label className="mt-3 flex items-center justify-between gap-3 text-text-primary">
          <span>{t('updates.automatic')}</span>
          <input
            type="checkbox"
            checked={automatic}
            disabled={!loaded || preferenceSaving || import.meta.env.DEV || phase === 'installing'}
            onChange={(event) => void setAutomaticUpdates(event.target.checked)}
          />
        </label>
      )}
      <p className="mt-2 text-[12px] leading-5 text-text-secondary">{t('updates.policy')}</p>
      <p role="status" className="mt-2 text-[12px] text-text-primary">
        {import.meta.env.DEV
          ? t('updates.development')
          : t(`updates.status.${phase}`, { version, percent: Math.round(percent ?? 0) })}
      </p>
      {settings && preferenceError && (
        <p role="alert" className="mt-2 text-[12px] text-error">
          {t('common.saveFailed')}
        </p>
      )}
      {busy && ready && <p className="mt-2 text-[12px] text-text-secondary">{t('updates.busy')}</p>}
      <div className="mt-3 flex flex-wrap items-center gap-2">
        {ready ? (
          <button
            type="button"
            disabled={busy}
            onClick={() => void installUpdate()}
            className="inline-flex items-center gap-2 rounded-[7px] bg-accent px-3 py-2 text-[13px] text-white disabled:opacity-50"
          >
            <RefreshCw size={14} />
            {t('updates.restart')}
          </button>
        ) : settings || phase === 'error' ? (
          <button
            type="button"
            disabled={working || import.meta.env.DEV}
            onClick={() => void checkForUpdates()}
            className="inline-flex items-center gap-2 rounded-[7px] bg-accent px-3 py-2 text-[13px] text-white disabled:opacity-50"
          >
            <Download size={14} />
            {t('updates.checkNow')}
          </button>
        ) : null}
        <button
          type="button"
          onClick={() => void openUrl(`${APP_REPO_URL}/releases/latest`).catch(() => {})}
          className="rounded-[7px] border border-border px-3 py-2 text-[12px] text-text-secondary"
        >
          {t('updates.viewRelease')}
        </button>
      </div>
    </section>
  )
}
