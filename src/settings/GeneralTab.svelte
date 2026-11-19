<script lang="ts">
  import { onMount } from 'svelte'
  import { api } from '../lib/api'
  import type { Settings } from '../lib/bindings/Settings'
  import type { Snapshot } from '../lib/bindings/Snapshot'
  import BatteryRing from '../lib/components/BatteryRing.svelte'
  import Button from '../lib/components/Button.svelte'
  import SettingRow from '../lib/components/SettingRow.svelte'
  import ToggleSwitch from '../lib/components/ToggleSwitch.svelte'
  import { subtitle } from '../lib/format'
  import { app } from '../lib/stores.svelte'

  interface Props {
    snapshot: Snapshot
    settings: Settings
  }
  let { snapshot, settings }: Props = $props()

  const connected = $derived(snapshot.link === 'connected')
  const status = $derived(
    connected
      ? `Connected${snapshot.productId ? ` · model ${snapshot.productId}` : ''}`
      : snapshot.link === 'connecting'
        ? 'Connecting…'
        : 'Not connected',
  )

  // Launch at login is the system's setting, not ours: read it whenever this page is shown or the
  // window regains focus (the user may have changed it in Windows' Startup apps).
  let launchAtLogin = $state(false)
  let launchError = $state('')
  let version = $state('')

  async function refreshAutostart() {
    launchAtLogin = await api.getAutostart().catch(() => false)
  }

  async function toggleLaunch(enabled: boolean) {
    launchError = ''
    try {
      launchAtLogin = await api.setAutostart(enabled)
    } catch (error) {
      launchError = `Could not change the startup setting: ${error}`
      await refreshAutostart()
    }
  }

  onMount(() => {
    void refreshAutostart()
    void api.appVersion().then((v) => (version = v))
    window.addEventListener('focus', refreshAutostart)
    return () => window.removeEventListener('focus', refreshAutostart)
  })
</script>

<div class="page">
  <section class="device">
    <BatteryRing left={connected ? snapshot.left : null} right={connected ? snapshot.right : null} size={52} />
    <div class="who">
      <div class="name">{snapshot.deviceName}</div>
      <div class="status">{status}{connected ? ` · ${subtitle(snapshot)}` : ''}</div>
    </div>
    <Button onclick={() => api.reconnect()}>Reconnect</Button>
  </section>

  <div class="group">
    <SettingRow title="Launch at login" description="Start Buds Control in the notification area when you sign in.">
      <ToggleSwitch label="Launch at login" checked={launchAtLogin} onchange={toggleLaunch} />
    </SettingRow>
    {#if launchError}<p class="error" role="alert">{launchError}</p>{/if}
    <SettingRow title="Reconnect buds to this {snapshot.hostLabel} automatically" description="When the buds come out of the case and only join your phone, bring them back here too.">
      <ToggleSwitch label="Reconnect automatically" checked={settings.autoConnect} onchange={(v) => api.updateSettings({ autoConnect: v })} />
    </SettingRow>
    <SettingRow title="Remember settings between connections" description="When the buds connect, put back the noise mode and low latency you last picked in Buds Control.">
      <ToggleSwitch label="Remember settings" checked={settings.rememberSettings} onchange={(v) => api.updateSettings({ rememberSettings: v })} />
    </SettingRow>
    <SettingRow title="Show an on-screen popup when a shortcut changes the mode">
      <ToggleSwitch label="Show popup" checked={settings.showHud} onchange={(v) => api.updateSettings({ showHud: v })} />
    </SettingRow>
  </div>

  <div class="group">
    <SettingRow title="Show developer options" description="Packet log, raw commands and undocumented modes.">
      <ToggleSwitch label="Show developer options" checked={settings.developerMode} onchange={(v) => api.updateSettings({ developerMode: v })} />
    </SettingRow>
  </div>

  {#if app.platform?.os === 'windows'}
    <p class="tip">
      Tip: to keep the icon always visible, drag it from the hidden-icons arrow (^) onto the taskbar.
    </p>
  {/if}
  <p class="about">Buds Control {version} · unofficial, not affiliated with OnePlus, OPPO or realme.</p>
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .device {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px;
    background: var(--card);
    border: 1px solid var(--stroke);
    border-radius: var(--radius-card);
  }
  .who {
    flex: 1;
    min-width: 0;
  }
  .name {
    font-size: 16px;
    font-weight: 600;
  }
  .status {
    font-size: 13px;
    color: var(--text-2);
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .error {
    margin: 2px 4px;
    font-size: 12px;
    color: var(--danger);
  }
  .tip,
  .about {
    margin: 0;
    font-size: 12px;
    color: var(--text-2);
  }
  .about {
    color: var(--text-3);
  }
</style>
