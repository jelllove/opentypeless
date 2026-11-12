import { describe, expect, it } from 'vitest'
// The production release tool is a dependency-free Node module.
// @ts-expect-error Release tooling is JavaScript rather than application TypeScript.
import { updateManifest, verifyUpdateSignature } from '../../../scripts/build-update-manifest.mjs'
import signature from '../../../src-tauri/tests/fixtures/updater-payload.txt.sig?raw'
import payload from '../../../src-tauri/tests/fixtures/updater-payload.txt?raw'
import config from '../../../src-tauri/tauri.conf.json'
import workflow from '../../../.github/workflows/release-fork.yml?raw'
import cargoManifest from '../../../src-tauri/Cargo.toml?raw'
import cargoLock from '../../../src-tauri/Cargo.lock?raw'

describe('fork updater release contract', () => {
  it('verifies fork signatures and rejects tampered payloads and trusted comments', () => {
    const bytes = new TextEncoder().encode(payload)
    expect(() =>
      verifyUpdateSignature(bytes, signature, config.plugins.updater.pubkey),
    ).not.toThrow()
    bytes[0] ^= 1
    expect(() => verifyUpdateSignature(bytes, signature, config.plugins.updater.pubkey)).toThrow()
    const altered = btoa(atob(signature.trim()).replace('timestamp:', 'tamperstamp:'))
    expect(() =>
      verifyUpdateSignature(
        new TextEncoder().encode(payload),
        altered,
        config.plugins.updater.pubkey,
      ),
    ).toThrow()
  })

  it('pins windows x64 payloads to the fork stable release', () => {
    const manifest = updateManifest('1.4.0', signature, '2026-11-12T00:00:00Z')
    expect(manifest.platforms['windows-x86_64']).toEqual({
      signature: signature.trim(),
      url: 'https://github.com/jelllove/opentypeless/releases/download/v1.4.0/OpenTypeless_1.4.0_x64-setup.exe',
    })
    expect(Object.keys(manifest.platforms)).toEqual(['windows-x86_64'])
    expect(manifest.version).toBe('1.4.0')
  })

  it('rejects prerelease tags and missing signatures', () => {
    for (const version of ['v1.4.0', '1.4.0-beta', '../release', '1.4']) {
      expect(() => updateManifest(version, signature)).toThrow()
    }
    expect(() => updateManifest('1.4.0', '')).toThrow()
  })

  it('keeps raw NSIS updates without compiling an unused ZIP extractor', () => {
    expect(cargoManifest).toContain(
      'tauri-plugin-updater = { version = "=2.10.1", default-features = false, features = ["rustls-tls"] }',
    )
    expect(cargoLock).not.toContain('name = "zip"')
    expect(cargoLock).toContain('name = "libsqlite3-sys"\nversion = "0.39.1"')
  })

  it('uses the fork feed and signs only in its dedicated release workflow', () => {
    expect(config.plugins.updater.endpoints).toEqual([
      'https://github.com/jelllove/opentypeless/releases/latest/download/latest.json',
    ])
    expect(config.plugins.updater.windows.installMode).toBe('passive')
    expect(workflow).toContain("github.repository == 'jelllove/opentypeless'")
    expect(workflow).toContain('secrets.TAURI_SIGNING_PRIVATE_KEY')
    expect(workflow).toContain('runs-on: windows-2022')
    expect(workflow).toContain('npm audit --audit-level=high')
    expect(workflow).toContain(
      'cargo audit --file src-tauri/Cargo.lock --ignore RUSTSEC-2026-0194 --ignore RUSTSEC-2026-0195',
    )
    expect(workflow).toContain('signer sign')
    expect(workflow).toContain('--verify-tag')
    expect(workflow).not.toContain('dangerousInsecureTransportProtocol')
  })
})
