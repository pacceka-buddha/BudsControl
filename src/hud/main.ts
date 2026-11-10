import { mount } from 'svelte'
import '../lib/styles/base.css'
import Hud from './Hud.svelte'

const app = mount(Hud, {
  target: document.getElementById('app')!,
})

export default app
