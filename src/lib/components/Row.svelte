<script lang="ts">
  // An icon, a two-line label and something on the right (a switch, a button...).
  import type { Snippet } from 'svelte'
  import Icon, { type IconName } from './Icon.svelte'

  interface Props {
    icon: IconName
    title: string
    subtitle: string
    accessory: Snippet
  }
  let { icon, title, subtitle, accessory }: Props = $props()
</script>

<div class="row">
  <span class="icon"><Icon name={icon} size={16} /></span>
  <div class="text">
    <div class="title">{title}</div>
    <div class="sub">{subtitle}</div>
  </div>
  <div class="accessory">{@render accessory()}</div>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .icon {
    flex: none;
    width: 20px;
    display: grid;
    place-items: center;
    color: var(--text-2);
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-size: 12.5px;
    font-weight: 600;
  }
  .sub {
    font-size: 11px;
    color: var(--text-2);
  }
  .accessory {
    flex: none;
  }
</style>
