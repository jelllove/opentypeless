import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  check: vi.fn(),
  invoke: vi.fn(),
  get: vi.fn(),
  set: vi.fn(),
  save: vi.fn(),
}))
vi.mock('@tauri-apps/plugin-updater', () => ({ check: mocks.check }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('@tauri-apps/plugin-store', () => ({
  LazyStore: class {
    get = mocks.get
    set = mocks.set
    save = mocks.save
  },
}))

let updates: typeof import('../updateStore')
let app: typeof import('../appStore')
const available = () => ({
  version: '1.4.1',
  download: vi.fn().mockResolvedValue(undefined),
  install: vi.fn().mockResolvedValue(undefined),
  close: vi.fn().mockResolvedValue(undefined),
})
beforeEach(async () => {
  vi.resetModules()
  vi.resetAllMocks()
  vi.stubEnv('DEV', false)
  mocks.get.mockResolvedValue(true)
  mocks.set.mockResolvedValue(undefined)
  mocks.save.mockResolvedValue(undefined)
  mocks.invoke.mockResolvedValue(undefined)
  mocks.check.mockResolvedValue(null)
  updates = await import('../updateStore')
  app = await import('../appStore')
})

describe('verified update lifecycle', () => {
  it('loads the persisted opt-out before automatic checks', async () => {
    mocks.get.mockResolvedValue(false)
    await updates.loadUpdatePreferences()
    expect(updates.useUpdateStore.getState()).toMatchObject({ automatic: false, loaded: true })
    expect(mocks.check).not.toHaveBeenCalled()
  })

  it('saves automatic preferences immediately without modifying app config', async () => {
    await updates.setAutomaticUpdates(false)
    expect(mocks.set).toHaveBeenCalledWith('automatic', false)
    expect(mocks.save).toHaveBeenCalled()
    expect(updates.useUpdateStore.getState().automatic).toBe(false)
    expect(mocks.invoke).not.toHaveBeenCalled()
  })

  it('does not silently enable downloads if preference loading fails', async () => {
    mocks.get.mockRejectedValue(new Error('disk'))
    await updates.loadUpdatePreferences()
    expect(updates.useUpdateStore.getState()).toMatchObject({
      automatic: false,
      preferenceError: true,
    })
  })

  it('reports up to date and forbids downgrades', async () => {
    await updates.checkForUpdates()
    expect(mocks.check).toHaveBeenCalledWith({ timeout: 30_000, allowDowngrades: false })
    expect(updates.useUpdateStore.getState().phase).toBe('upToDate')
  })

  it('downloads automatically but never installs or restarts without a user action', async () => {
    const update = available()
    mocks.check.mockResolvedValue(update)
    await updates.checkForUpdates()
    expect(update.download).toHaveBeenCalled()
    expect(update.install).not.toHaveBeenCalled()
    expect(mocks.invoke).not.toHaveBeenCalled()
    expect(updates.useUpdateStore.getState()).toMatchObject({ phase: 'ready', version: '1.4.1' })
    await updates.checkForUpdates()
    expect(mocks.check).toHaveBeenCalledTimes(1)
  })

  it('never marks an invalid signature ready even after the Finished download event', async () => {
    const update = available()
    update.download.mockImplementation(async (onEvent) => {
      onEvent({ event: 'Finished' })
      throw new Error('Signature verification failed')
    })
    mocks.check.mockResolvedValue(update)
    await updates.checkForUpdates()
    expect(updates.useUpdateStore.getState().phase).toBe('error')
    expect(update.close).toHaveBeenCalled()
    expect(update.install).not.toHaveBeenCalled()
  })

  it('does not start duplicate concurrent checks', async () => {
    let resolve!: (value: null) => void
    mocks.check.mockReturnValue(
      new Promise((done) => {
        resolve = done
      }),
    )
    const first = updates.checkForUpdates()
    await updates.checkForUpdates()
    expect(mocks.check).toHaveBeenCalledTimes(1)
    resolve(null)
    await first
  })

  it('blocks install during recording, then gates the native install and restart', async () => {
    const update = available()
    mocks.check.mockResolvedValue(update)
    await updates.checkForUpdates()
    app.useAppStore.setState({ pipelineState: 'recording' })
    await updates.installUpdate()
    expect(mocks.invoke).not.toHaveBeenCalled()
    app.useAppStore.setState({ pipelineState: 'idle' })
    await updates.installUpdate()
    expect(mocks.invoke).toHaveBeenNthCalledWith(1, 'begin_update_install')
    expect(update.install).toHaveBeenCalledTimes(1)
    expect(mocks.invoke).toHaveBeenNthCalledWith(2, 'finish_update_install')
  })

  it('does not install if the backend rejects busy Ask and releases the gate on failure', async () => {
    const update = available()
    mocks.check.mockResolvedValue(update)
    await updates.checkForUpdates()
    mocks.invoke.mockRejectedValueOnce(new Error('Ask busy'))
    await updates.installUpdate()
    expect(update.install).not.toHaveBeenCalled()
    expect(mocks.invoke).toHaveBeenCalledWith('cancel_update_install')
    expect(updates.useUpdateStore.getState().phase).toBe('error')
  })

  it('keeps verified payloads installable if saving the preference fails', async () => {
    const update = available()
    mocks.check.mockResolvedValue(update)
    await updates.checkForUpdates()
    mocks.save.mockRejectedValueOnce(new Error('disk full'))
    await updates.setAutomaticUpdates(false)
    expect(updates.useUpdateStore.getState()).toMatchObject({
      phase: 'ready',
      automatic: true,
      preferenceError: true,
      preferenceSaving: false,
    })
    expect(update.close).not.toHaveBeenCalled()
  })

  it('does not check for updates in development builds', async () => {
    vi.stubEnv('DEV', true)
    await updates.checkForUpdates()
    expect(mocks.check).not.toHaveBeenCalled()
  })
})
