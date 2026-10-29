import { mount } from 'svelte'

import './app.css'
import App from './App.svelte'
import { HttpApi, Live } from './lib/api'

const live = new Live()
live.start()

const app = mount(App, {
  target: document.getElementById('app')!,
  props: { api: new HttpApi(), live },
})

export default app
