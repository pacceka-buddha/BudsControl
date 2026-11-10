<script lang="ts">
  // For diagnosing and exploring: device facts, every noise-mode bit, raw commands, the packet log.
  import { api } from '../lib/api'
  import type { Snapshot } from '../lib/bindings/Snapshot'
  import Button from '../lib/components/Button.svelte'
  import Icon from '../lib/components/Icon.svelte'
  import SettingRow from '../lib/components/SettingRow.svelte'
  import ToggleSwitch from '../lib/components/ToggleSwitch.svelte'
  import { parseCommand } from '../lib/format'
  import PacketLog from './PacketLog.svelte'

  interface Props {
    snapshot: Snapshot
  }
  let { snapshot }: Props = $props()

  let commandText = $state('0106')
  let payloadText = $state('')
  const command = $derived(parseCommand(commandText))

  const facts = $derived([
    ['Name', snapshot.deviceName],
    ['Address', snapshot.address ?? '—'],
    ['RFCOMM channel', snapshot.channel?.toString() ?? '—'],
    ['Product ID', snapshot.productId ?? '—'],
    ['Active noise bit', snapshot.noiseBit == null ? '—' : `bit ${snapshot.noiseBit}`],
  ])

  function send() {
    if (command !== null) void api.sendRaw(command, payloadText)
  }
</script>

<div class="page">
  <section>
    <h2>Device</h2>
    <dl class="facts">
      {#each facts as [key, value] (key)}
        <dt>{key}</dt>
        <dd>{value}</dd>
      {/each}
    </dl>
  </section>

  <section>
    <h2>Noise modes</h2>
    <p class="hint">Every mode bit HeyMelody lists for this model, including ones the flyout doesn't show.</p>
    <div class="group">
      {#each snapshot.layout as item (item.bit)}
        <div class="bit">
          <span class="num">bit {item.bit}</span>
          <span class="mode">{item.name}</span>
          {#if snapshot.noiseBit === item.bit}
            <span class="active" aria-label="Active"><Icon name="check" size={16} /></span>
          {:else}
            <Button onclick={() => api.setNoiseBit(item.bit)}>Apply</Button>
          {/if}
        </div>
      {:else}
        <p class="hint">Connect the buds to see their layout.</p>
      {/each}
    </div>
  </section>

  <section>
    <h2>Experimental</h2>
    <SettingRow title="Game mode (low latency)" description={snapshot.lowLatency == null ? "The buds don't report this until it changes." : 'Reported by the buds.'}>
      <ToggleSwitch label="Game mode" checked={snapshot.lowLatency ?? false} onchange={(on) => api.setLowLatency(on)} />
    </SettingRow>
  </section>

  {#if snapshot.hosts.length > 0}
    <section>
      <h2>Paired devices (multipoint)</h2>
      <div class="group">
        {#each snapshot.hosts as host (host.address)}
          <SettingRow title={host.name + (host.isThisHost ? ` (this ${snapshot.hostLabel})` : '')} description={host.connected ? 'Connected' : 'Not connected'}>
            <code>{host.address}</code>
          </SettingRow>
        {/each}
      </div>
    </section>
  {/if}

  <section>
    <h2>Raw command</h2>
    <div class="raw">
      <input class="cmd" aria-label="Command (hex)" placeholder="0106" bind:value={commandText} spellcheck="false" />
      <input class="payload" aria-label="Payload (hex)" placeholder="payload hex, e.g. 01 01" bind:value={payloadText} spellcheck="false" />
      <Button variant="accent" disabled={command === null} onclick={send}>Send</Button>
    </div>
    <div class="quick">
      <Button onclick={() => api.sendRaw(0x0103, '')}>Product ID</Button>
      <Button onclick={() => api.sendRaw(0x0106, '')}>Battery</Button>
      <Button onclick={() => api.sendRaw(0x010c, '01 01')}>Noise mode</Button>
      <span class="spacer"></span>
      <Button onclick={() => api.scanQueries()}>Scan 0x0101–0x0120</Button>
    </div>
    <p class="hint">Frames are wrapped automatically (AA, length, sequence). Commands outside 0x01xx can change settings on the buds.</p>
  </section>

  <section>
    <h2>Packet log</h2>
    <PacketLog />
  </section>
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 600;
  }
  .hint {
    margin: 0 0 8px;
    font-size: 12px;
    color: var(--text-2);
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .facts {
    display: grid;
    grid-template-columns: 160px 1fr;
    gap: 6px 12px;
    margin: 0;
    padding: 12px 14px;
    background: var(--card);
    border: 1px solid var(--stroke);
    border-radius: var(--radius-card);
    font-size: 13px;
  }
  dt {
    color: var(--text-2);
  }
  dd {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    user-select: text;
  }
  .bit {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 40px;
    padding: 6px 14px;
    background: var(--card);
    border: 1px solid var(--stroke);
    border-radius: var(--radius-card);
    font-size: 13px;
  }
  .num {
    width: 56px;
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
  }
  .mode {
    flex: 1;
  }
  .active {
    color: var(--accent);
    display: grid;
    width: 56px;
    justify-items: center;
  }
  code {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-2);
    user-select: text;
  }
  .raw {
    display: flex;
    gap: 8px;
  }
  input {
    min-height: 28px;
    padding: 4px 8px;
    border: 1px solid var(--stroke-strong);
    border-radius: var(--radius-control);
    background: var(--fill);
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 12px;
    user-select: text;
  }
  input:focus {
    outline: 2px solid var(--accent);
    outline-offset: 0;
  }
  .cmd {
    width: 90px;
  }
  .payload {
    flex: 1;
    min-width: 0;
  }
  .quick {
    display: flex;
    gap: 8px;
    margin: 8px 0;
  }
  .spacer {
    flex: 1;
  }
</style>
