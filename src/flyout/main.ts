import { mount } from 'svelte'
import '../lib/styles/base.css'
import Flyout from './Flyout.svelte'

const app = mount(Flyout, {
  target: document.getElementById('app')!,
})

export default app
