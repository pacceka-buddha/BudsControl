<script lang="ts">
  import { api } from '../lib/api'
  import BatteryRing from '../lib/components/BatteryRing.svelte'
  import Icon from '../lib/components/Icon.svelte'
  import { subtitle } from '../lib/format'
  import { popupMenu } from '../lib/menu'
  import type { Snapshot } from '../lib/bindings/Snapshot'

  interface Props {
    snapshot: Snapshot
    /** The More panel is open. */
    expanded: boolean
    ontoggle: () => void
  }
  let { snapshot, expanded, ontoggle }: Props = $props()

  const connected = $derived(snapshot.link === 'connected')
  let menuButton: HTMLButtonElement | undefined = $state()

  async function openMenu() {
    if (!menuButton) return
    const choice = await popupMenu(
      [
        { id: 'settings', text: 'Settings…' },
        { id: 'reconnect', text: 'Reconnect' },
        'separator',
        { id: 'quit', text: 'Quit Buds Control' },
      ],
      menuButton,
    )
    if (choice === 'settings') await api.openSettings()
    else if (choice === 'reconnect') await api.reconnect()
    else if (choice === 'quit') await api.quit()
  }
</script>

<header>
  <BatteryRing left={connected ? snapshot.left : null} right={connected ? snapshot.right : null} size={34} />
  <div class="text">
    <div class="name">{snapshot.deviceName}</div>
    <div class="sub">{subtitle(snapshot)}</div>
  </div>
  <div class="buttons">
    {#if connected && snapshot.hasMoreControls}
      <button
        type="button"
        class="round"
        class:flipped={expanded}
        aria-label={expanded ? 'Show fewer controls' : 'Show more controls'}
        aria-expanded={expanded}
        aria-controls="more-controls"
        title={expanded ? 'Fewer controls' : 'Equalizer, low latency, find earbuds and gestures'}
        onclick={ontoggle}
      >
        <Icon name="chevronDown" size={12} />
      </button>
    {/if}
    <button type="button" class="round" bind:this={menuButton} aria-label="More options" aria-haspopup="menu" title="Settings, reconnect, quit" onclick={openMenu}>
      <Icon name="more" size={16} />
    </button>
  </div>
</header>

<style>
  header {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .name {
    font-size: 13.5px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 11.5px;
    color: var(--text-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .buttons {
    display: flex;
    gap: 6px;
  }
  .round {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--fill);
    box-shadow:
      inset 0 0.5px 0 var(--sel-hi),
      0 1px 2px rgba(0, 0, 0, 0.12);
    color: var(--text-2);
    transition: filter 120ms;
  }
  .round:hover {
    filter: brightness(1.15);
  }
  .round:active {
    filter: brightness(0.92);
  }
  .round :global(svg) {
    transition: transform var(--dur) var(--ease-out);
  }
  .flipped :global(svg) {
    transform: rotate(180deg);
  }
</style>
