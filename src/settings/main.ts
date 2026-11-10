import { mount } from 'svelte'
import '../lib/styles/base.css'
import Settings from './Settings.svelte'

// The settings window has a normal title bar over a Mica backdrop (set by the Rust side), so the page
// background stays translucent instead of covering it.
document.documentElement.classList.add('mica')

const app = mount(Settings, {
  target: document.getElementById('app')!,
})

export default app
