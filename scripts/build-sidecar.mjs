// Builds `budsctl` in release mode and copies it where Tauri expects a sidecar:
// src-tauri/binaries/budsctl-<target-triple>[.exe]. Run by `beforeBuildCommand`.
import { execFileSync } from 'node:child_process'
import { copyFileSync, mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const host = execFileSync('rustc', ['-vV'], { encoding: 'utf8' })
  .split('\n')
  .find((line) => line.startsWith('host:'))
  ?.slice(5)
  .trim()
if (!host) throw new Error('cannot determine the Rust target triple from `rustc -vV`')

execFileSync('cargo', ['build', '--release', '-p', 'budsctl'], { cwd: root, stdio: 'inherit' })

const ext = host.includes('windows') ? '.exe' : ''
const from = join(root, 'target', 'release', `budsctl${ext}`)
const dir = join(root, 'src-tauri', 'binaries')
mkdirSync(dir, { recursive: true })
const to = join(dir, `budsctl-${host}${ext}`)
copyFileSync(from, to)
console.log(`sidecar: ${to}`)
