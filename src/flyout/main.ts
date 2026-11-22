import { mount } from 'svelte'
import '../lib/styles/base.css'
import { applyPlatform } from '../lib/theme'
import Flyout from './Flyout.svelte'

// The flyout's own look (see tokens.css), set before the first paint.
document.documentElement.classList.add('glass')
// The OS tag (`data-platform`) too, so the platform's own look is there from the first paint.
void applyPlatform()

const app = mount(Flyout, {
  target: document.getElementById('app')!,
})

export default app
