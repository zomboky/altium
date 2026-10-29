import { afterEach, describe, expect, it, vi } from 'vitest'

import { Live, type Source } from './live.svelte'

class FakeSource implements Source {
  listeners = new Map<string, ((event: MessageEvent) => void)[]>()
  onerror: ((event: Event) => void) | null = null
  closed = false

  addEventListener(type: string, listener: (event: MessageEvent) => void) {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener])
  }

  close() {
    this.closed = true
  }

  emit(type: string, data: unknown = {}) {
    for (const listener of this.listeners.get(type) ?? []) {
      listener(new MessageEvent(type, { data: JSON.stringify(data) }))
    }
  }

  fail() {
    this.onerror?.(new Event('error'))
  }
}

function setup() {
  const sources: FakeSource[] = []
  const live = new Live(() => {
    const source = new FakeSource()
    sources.push(source)
    return source
  }, 10)
  live.start()
  return { live, sources, last: () => sources[sources.length - 1] }
}

afterEach(() => {
  vi.useRealTimers()
})

describe('Live', () => {
  it('dispatches changes by project and scope', () => {
    const { live, last } = setup()
    const tasks = vi.fn()
    const memory = vi.fn()
    const overview = vi.fn()
    live.on('p1', ['tasks'], tasks)
    live.on('p1', ['memory'], memory)
    live.on(null, ['projects'], overview)
    last().emit('hello')
    last().emit('change', { project: 'p1', scope: 'tasks' })
    last().emit('change', { project: 'p2', scope: 'tasks' })
    last().emit('change', { project: null, scope: 'projects' })
    expect(tasks).toHaveBeenCalledTimes(1)
    expect(memory).not.toHaveBeenCalled()
    expect(overview).toHaveBeenCalledTimes(1)
  })

  it('refetches everything after a reconnection', () => {
    vi.useFakeTimers()
    const { live, sources, last } = setup()
    const tasks = vi.fn()
    const overview = vi.fn()
    live.on('p1', ['tasks'], tasks)
    live.on(null, ['projects'], overview)
    last().emit('hello')
    expect(live.connected).toBe(true)
    // The first connection refetches nothing: views load on their own.
    expect(tasks).not.toHaveBeenCalled()

    last().fail()
    expect(live.connected).toBe(false)
    expect(sources[0].closed).toBe(true)
    vi.advanceTimersByTime(10)
    expect(sources).toHaveLength(2)
    last().emit('hello')
    expect(live.connected).toBe(true)
    expect(tasks).toHaveBeenCalledTimes(1)
    expect(overview).toHaveBeenCalledTimes(1)
  })

  it('refetches everything when the server says changes were missed', () => {
    const { live, last } = setup()
    const tasks = vi.fn()
    live.on('p1', ['tasks'], tasks)
    last().emit('hello')
    last().emit('resync')
    expect(tasks).toHaveBeenCalledTimes(1)
  })

  it('stops calling a removed listener', () => {
    const { live, last } = setup()
    const tasks = vi.fn()
    const off = live.on('p1', ['tasks'], tasks)
    off()
    last().emit('change', { project: 'p1', scope: 'tasks' })
    expect(tasks).not.toHaveBeenCalled()
  })
})
