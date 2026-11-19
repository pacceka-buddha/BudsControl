<script lang="ts">
  import { api } from '../lib/api'
  import Button from '../lib/components/Button.svelte'
  import Spinner from '../lib/components/Spinner.svelte'
  import type { Snapshot } from '../lib/bindings/Snapshot'

  interface Props {
    snapshot: Snapshot
  }
  let { snapshot }: Props = $props()
  const connecting = $derived(snapshot.link === 'connecting')
</script>

<div class="panel" role="status" aria-live="polite">
  {#if connecting}<Spinner size={16} />{/if}
  <span class="message">{connecting ? 'Connecting…' : `Buds not connected to this ${snapshot.hostLabel}`}</span>
  {#if !connecting}
    <Button onclick={() => api.reconnect()}>Retry</Button>
  {/if}
</div>

<style>
  .panel {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    background: var(--card);
    box-shadow: inset 0 0.5px 0 var(--hi);
    border-radius: var(--radius-card);
  }
  .message {
    flex: 1;
    font-size: 12px;
    color: var(--text-2);
  }
</style>
