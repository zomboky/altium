// Drives the real `altium` binary for the end-to-end tests, in a throwaway
// environment: its own Altium config, its own Claude config, no real claude.

import { type ChildProcess, execFileSync, spawn } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const repo = resolve(here, '../..')
const exe = process.platform === 'win32' ? 'altium.exe' : 'altium'
export const BIN = process.env.ALTIUM_BIN ?? join(repo, 'target', 'debug', exe)
export const PORT = Number(process.env.ALTIUM_E2E_PORT ?? 7499)
export const ROOT = join(tmpdir(), 'altium-e2e')
export const PROJECT = join(ROOT, 'demo')
const FIXTURES = join(repo, 'crates/altium-cli/tests/fixtures/hooks/windows-2.1.310')

export function env(): NodeJS.ProcessEnv {
  return {
    ...process.env,
    ALTIUM_CONFIG_DIR: join(ROOT, 'config'),
    CLAUDE_CONFIG_DIR: join(ROOT, 'claude'),
    // `altium setup` writes the Claude config itself instead of calling claude.
    ALTIUM_CLAUDE: join(ROOT, 'no-claude'),
    HOME: join(ROOT, 'home'),
    USERPROFILE: join(ROOT, 'home'),
  }
}

export function altium(args: string[], options: { cwd?: string; input?: string } = {}): string {
  return execFileSync(BIN, args, { cwd: options.cwd ?? ROOT, env: env(), input: options.input, encoding: 'utf8' })
}

/** Replays the recorded hooks of a real session, up to `last` included. */
export function replay(last = Infinity) {
  const files = readdirSync(FIXTURES)
    .filter((f) => f.endsWith('.json'))
    .sort()
  for (const file of files) {
    const index = Number(file.slice(0, 3))
    if (index > last) break
    const event = file.slice(4, -5)
    const project = JSON.stringify(PROJECT).slice(1, -1)
    const payload = readFileSync(join(FIXTURES, file), 'utf8').replaceAll('{{PROJECT}}', project)
    altium(['hook', event], { cwd: PROJECT, input: payload })
  }
}

/** Calls MCP tools as Claude would, from the project directory. */
export function mcp(calls: { name: string; arguments: Record<string, unknown> }[]): unknown[] {
  const lines = [
    { jsonrpc: '2.0', id: 0, method: 'initialize', params: { protocolVersion: '2025-06-18', capabilities: {} } },
    { jsonrpc: '2.0', method: 'notifications/initialized' },
    ...calls.map((call, i) => ({ jsonrpc: '2.0', id: i + 1, method: 'tools/call', params: call })),
  ]
  const output = altium(['mcp'], { cwd: PROJECT, input: lines.map((l) => JSON.stringify(l)).join('\n') + '\n' })
  return output
    .split('\n')
    .filter(Boolean)
    .map((l) => JSON.parse(l))
    .filter((r) => r.id > 0)
    .map((r) => {
      if (r.error || r.result?.isError) throw new Error(JSON.stringify(r))
      return r.result
    })
}

/** A fresh environment with one project, `Démo`, initialized and set up. */
export function prepare() {
  rmSync(ROOT, { recursive: true, force: true })
  for (const dir of ['config', 'claude', 'home', 'demo']) mkdirSync(join(ROOT, dir), { recursive: true })
  writeFileSync(join(PROJECT, 'README.md'), '# Démo\n')
  try {
    execFileSync('git', ['init', '-q'], { cwd: PROJECT })
  } catch {
    // Without git, the dashboard just shows no repository.
  }
  altium(['setup'])
  altium(['init', '--name', 'Démo'], { cwd: PROJECT })
}

export function serve(): ChildProcess {
  if (!existsSync(BIN)) throw new Error(`${BIN} introuvable : lance d’abord cargo build`)
  const server = spawn(BIN, ['serve', '--port', String(PORT)], { cwd: ROOT, env: env(), stdio: 'ignore' })
  return server
}

export async function waitForServer(timeoutMs = 10_000) {
  const start = Date.now()
  while (Date.now() - start < timeoutMs) {
    try {
      const response = await fetch(`http://127.0.0.1:${PORT}/api/health`)
      if (response.ok) return
    } catch {
      // Not listening yet.
    }
    await new Promise((r) => setTimeout(r, 100))
  }
  throw new Error(`altium serve ne répond pas sur le port ${PORT}`)
}
