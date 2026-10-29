// A clock ticking every second, for timers and relative dates.

class Clock {
  now = $state(new Date())

  constructor() {
    setInterval(() => (this.now = new Date()), 1000)
  }
}

export const clock = new Clock()
