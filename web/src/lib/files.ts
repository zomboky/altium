// Project files the dashboard can show: task deliverables and the files
// memory links point to. The server serves them under /api/projects/{id}/files/.

import { labels } from './i18n.svelte'

export type FileKind = 'pdf' | 'video' | 'audio' | 'image' | 'page' | 'text' | 'other' | 'url'

const EXTENSIONS: Record<string, FileKind> = {
  pdf: 'pdf',
  mp4: 'video',
  webm: 'video',
  ogv: 'video',
  mov: 'video',
  mp3: 'audio',
  wav: 'audio',
  ogg: 'audio',
  m4a: 'audio',
  png: 'image',
  jpg: 'image',
  jpeg: 'image',
  gif: 'image',
  webp: 'image',
  avif: 'image',
  svg: 'image',
  html: 'page',
  htm: 'page',
  md: 'text',
  markdown: 'text',
  txt: 'text',
  csv: 'text',
  tsv: 'text',
  json: 'text',
  log: 'text',
  py: 'text',
  r: 'text',
  yaml: 'text',
  yml: 'text',
  toml: 'text',
}

export const FILE_ICONS: Record<FileKind, string> = {
  pdf: '📄',
  video: '🎬',
  audio: '🔊',
  image: '🖼️',
  page: '🌐',
  text: '📝',
  other: '📦',
  url: '🔗',
}

export const FILE_LABELS = labels<FileKind>({
  pdf: ['PDF', 'PDF'],
  video: ['Video', 'Vidéo'],
  audio: ['Audio', 'Audio'],
  image: ['Image', 'Image'],
  page: ['Interactive page', 'Page interactive'],
  text: ['Text', 'Texte'],
  other: ['File', 'Fichier'],
  url: ['Web link', 'Lien web'],
})

export function isUrl(target: string): boolean {
  return /^https?:\/\//.test(target)
}

export function fileKind(target: string): FileKind {
  if (isUrl(target)) return 'url'
  const extension = target.split('/').pop()?.split('.').pop()?.toLowerCase() ?? ''
  return EXTENSIONS[extension] ?? 'other'
}

/** Last part of a path or URL, for labels. */
export function fileName(target: string): string {
  if (isUrl(target)) return target.replace(/^https?:\/\//, '')
  return target.split('/').filter(Boolean).pop() ?? target
}

/** Where the browser opens a deliverable or a linked file. */
export function fileHref(project: string, target: string): string {
  if (isUrl(target)) return target
  const path = target.split('/').filter(Boolean).map(encodeURIComponent).join('/')
  return `/api/projects/${encodeURIComponent(project)}/files/${path}`
}
