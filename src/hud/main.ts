import { mount } from 'svelte'
import '../lib/styles/base.css'
import Hud from './Hud.svelte'

// The HUD window is transparent: nothing but the pill may paint.
document.documentElement.classList.add('mica')

const app = mount(Hud, {
  target: document.getElementById('app')!,
})

export default app
