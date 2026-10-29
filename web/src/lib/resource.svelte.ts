// Data loaded from the server and refetched on live changes.

export class Resource<T> {
  data = $state<T | undefined>(undefined)
  error = $state<string | null>(null)
  loading = $state(true)
  private generation = 0

  constructor(private readonly load: () => Promise<T>) {}

  async refresh(): Promise<void> {
    const generation = ++this.generation
    try {
      const data = await this.load()
      // An older request finishing late must not overwrite a newer answer.
      if (generation !== this.generation) return
      this.data = data
      this.error = null
    } catch (e) {
      if (generation !== this.generation) return
      this.error = e instanceof Error ? e.message : String(e)
    } finally {
      if (generation === this.generation) this.loading = false
    }
  }
}
