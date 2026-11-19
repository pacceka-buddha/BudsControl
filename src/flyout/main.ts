import { mount } from 'svelte'
import '../lib/styles/base.css'
import Flyout from './Flyout.svelte'

// The flyout's own look (see tokens.css), set before the first paint.
document.documentElement.classList.add('glass')

const app = mount(Flyout, {
  target: document.getElementById('app')!,
})

export default app
