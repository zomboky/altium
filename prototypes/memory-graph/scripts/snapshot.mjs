// Copies a small sample of a project's .altium/memory into src/data/memory.json.
// Usage: node scripts/snapshot.mjs <project dir> [max nodes]
import { readdirSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs'
import { join, basename, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const project = process.argv[2] ?? '../../../meudon'
const max = Number(process.argv[3] ?? 12)
const dir = join(project, '.altium', 'memory')

function unquote(v) {
  v = v.trim()
  if ((v.startsWith("'") && v.endsWith("'")) || (v.startsWith('"') && v.endsWith('"'))) {
    return v.slice(1, -1).replaceAll("''", "'")
  }
  return v
}

// The frontmatter is a flat YAML subset: scalars, lists of scalars, lists of maps.
function parse(text) {
  const [, front, ...rest] = text.split(/^---\s*$/m)
  const meta = {}
  let key = null
  let item = null
  for (const line of front.split(/\r?\n/)) {
    if (!line.trim()) continue
    let m
    if ((m = line.match(/^([a-z_]+):\s*(.*)$/))) {
      key = m[1]
      meta[key] = m[2] ? unquote(m[2]) : []
      item = null
    } else if ((m = line.match(/^- ([a-z_]+):\s*(.*)$/))) {
      item = { [m[1]]: unquote(m[2]) }
      meta[key].push(item)
    } else if ((m = line.match(/^- (.*)$/))) {
      meta[key].push(unquote(m[1]))
    } else if ((m = line.match(/^\s+([a-z_]+):\s*(.*)$/)) && item) {
      item[m[1]] = unquote(m[2])
    }
  }
  return { meta, body: rest.join('---').trim() }
}

const nodes = readdirSync(dir)
  .filter((f) => f.endsWith('.md'))
  .map((f) => {
    const { meta, body } = parse(readFileSync(join(dir, f), 'utf8'))
    return {
      id: basename(f, '.md'),
      kind: meta.kind,
      title: meta.title,
      description: meta.desc ?? '',
      tags: Array.isArray(meta.tags) ? meta.tags : [],
      pinned: meta.pinned === 'true',
      links: (Array.isArray(meta.links) ? meta.links : []).map((l) => ({ to: l.to, type: l.type, note: l.note ?? null })),
      updated: meta.updated,
      body,
    }
  })

// Keep the best connected nodes: they make the most readable sample.
const degree = new Map(nodes.map((n) => [n.id, 0]))
for (const n of nodes) {
  for (const l of n.links) {
    degree.set(n.id, degree.get(n.id) + 1)
    if (degree.has(l.to)) degree.set(l.to, degree.get(l.to) + 1)
  }
}
const sample = nodes.sort((a, b) => degree.get(b.id) - degree.get(a.id)).slice(0, max)

const out = join(dirname(fileURLToPath(import.meta.url)), '..', 'src', 'data', 'memory.json')
mkdirSync(dirname(out), { recursive: true })
writeFileSync(out, JSON.stringify({ project: basename(project), nodes: sample }, null, 2))
console.log(`${sample.length} of ${nodes.length} nodes written to ${out}`)
