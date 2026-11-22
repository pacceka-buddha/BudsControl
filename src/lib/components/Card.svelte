<script lang="ts">
  // A softly filled rounded panel with an optional small grey caption title.
  import type { Snippet } from 'svelte'
  import Icon, { type IconName } from './Icon.svelte'

  interface Props {
    title?: string
    icon?: IconName
    children: Snippet
  }
  let { title, icon, children }: Props = $props()
</script>

<section class="card">
  {#if title}
    <h2>
      {#if icon}<Icon name={icon} size={12} />{/if}
      {title}
    </h2>
  {/if}
  {@render children()}
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px;
    background: var(--card);
    box-shadow: inset 0 0.5px 0 var(--hi, transparent);
    border-radius: var(--radius-card);
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 5px;
    margin: 0;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-2);
  }
</style>
