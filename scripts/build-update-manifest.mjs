import { createHash, createPublicKey, verify } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

export function updateManifest(version, signature, publishedAt = new Date().toISOString()) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error('Expected stable x.y.z version')
  const decoded = Buffer.from(signature.trim(), 'base64').toString('utf8')
  if (!decoded.startsWith('untrusted comment:') || !decoded.includes('trusted comment:')) {
    throw new Error('Expected a Tauri minisign signature')
  }
  return {
    version,
    notes: `See https://github.com/jelllove/opentypeless/releases/tag/v${version}`,
    pub_date: publishedAt,
    platforms: {
      'windows-x86_64': {
        signature: signature.trim(),
        url: `https://github.com/jelllove/opentypeless/releases/download/v${version}/OpenTypeless_${version}_x64-setup.exe`,
      },
    },
  }
}

// Match the native updater's minisign payload and trusted-comment verification.
export function verifyUpdateSignature(payload, signature, publicKey) {
  const lines = Buffer.from(signature.trim(), 'base64').toString('utf8').trimEnd().split(/\r?\n/)
  const keyLines = Buffer.from(publicKey.trim(), 'base64').toString('utf8').trimEnd().split(/\r?\n/)
  const key = Buffer.from(keyLines[1] ?? '', 'base64')
  const signed = Buffer.from(lines[1] ?? '', 'base64')
  const global = Buffer.from(lines[3] ?? '', 'base64')
  if (
    key.length !== 42 ||
    signed.length !== 74 ||
    global.length !== 64 ||
    !lines[2]?.startsWith('trusted comment: ') ||
    !key.subarray(2, 10).equals(signed.subarray(2, 10))
  ) {
    throw new Error('Invalid updater signature encoding or key ID')
  }
  const algorithm = signed.subarray(0, 2).toString('ascii')
  const data = algorithm === 'ED' ? createHash('blake2b512').update(payload).digest() : payload
  if (!['ED', 'Ed'].includes(algorithm)) throw new Error('Unsupported signature algorithm')
  const ed25519 = createPublicKey({
    key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), key.subarray(10)]),
    format: 'der',
    type: 'spki',
  })
  const detached = signed.subarray(10)
  if (
    !verify(null, data, ed25519, detached) ||
    !verify(null, Buffer.concat([detached, Buffer.from(lines[2].slice(17))]), ed25519, global)
  ) {
    throw new Error('Updater signature verification failed')
  }
}

async function main() {
  const [directory, version] = process.argv.slice(2)
  if (!directory || !version)
    throw new Error('Usage: node scripts/build-update-manifest.mjs <bundle-dir> <version>')
  const installerName = `OpenTypeless_${version}_x64-setup.exe`
  const installer = await readFile(path.join(directory, installerName))
  const signature = await readFile(path.join(directory, `${installerName}.sig`), 'utf8')
  const config = JSON.parse(
    await readFile(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8'),
  )
  verifyUpdateSignature(installer, signature, config.plugins.updater.pubkey)
  const manifest = JSON.stringify(updateManifest(version, signature), null, 2) + '\n'
  await writeFile(path.join(directory, 'latest.json'), manifest)
  const assets = [
    [installerName, installer],
    [`${installerName}.sig`, Buffer.from(signature)],
    ['latest.json', Buffer.from(manifest)],
  ]
  const checksums =
    assets
      .map(([name, bytes]) => `${createHash('sha256').update(bytes).digest('hex')}  ${name}`)
      .join('\n') + '\n'
  await writeFile(path.join(directory, 'SHA256SUMS.txt'), checksums)
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  await main()
}
