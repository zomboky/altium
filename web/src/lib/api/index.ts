import { getContext, setContext } from 'svelte'

import type { Api } from './http'
import type { Live } from './live.svelte'

export * from './http'
export * from './live.svelte'
export * from './types'

const KEY = Symbol('altium')

export interface Services {
  api: Api
  live: Live
}

export function provide(services: Services) {
  setContext(KEY, services)
}

export function services(): Services {
  return getContext<Services>(KEY)
}
