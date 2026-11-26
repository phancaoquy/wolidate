<script lang="ts">
  import { onMount } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import Icon from './Icon.svelte'
  import { isTauri } from '../lib/ipc/commands'

  const isAndroid =
    typeof navigator !== 'undefined' && /Android/i.test(navigator.userAgent)
  const visible = isTauri() && !isAndroid

  let maximized = $state(false)
  const appWindow = isTauri() ? getCurrentWindow() : null

  const directions = [
    'North',
    'South',
    'East',
    'West',
    'NorthEast',
    'NorthWest',
    'SouthEast',
    'SouthWest',
  ] as const

  async function refreshMaximized(): Promise<void> {
    if (appWindow) maximized = await appWindow.isMaximized()
  }

  function minimize(): void {
    void appWindow?.minimize()
  }

  function toggleMaximize(): void {
    void appWindow?.toggleMaximize()
  }

  function close(): void {
    void appWindow?.close()
  }

  function resize(direction: (typeof directions)[number], event: PointerEvent): void {
    event.preventDefault()
    void appWindow?.startResizeDragging(direction)
  }

  function onDoubleClick(event: MouseEvent): void {
    const target = event.target
    if (!(target instanceof Element)) return
    if (!target.closest('.titlebar') || target.closest('.window-controls')) return
    toggleMaximize()
  }

  onMount(() => {
    if (!appWindow) return
    void refreshMaximized()
    window.addEventListener('dblclick', onDoubleClick)
    let unlisten: (() => void) | undefined
    let active = true
    void appWindow.onResized(() => void refreshMaximized()).then((stop) => {
      if (active) unlisten = stop
      else stop()
    })
    return () => {
      active = false
      unlisten?.()
      window.removeEventListener('dblclick', onDoubleClick)
    }
  })
</script>

{#if visible}
  <header class="titlebar" data-tauri-drag-region>
    <div class="titlebar-brand" data-tauri-drag-region>
      <span class="titlebar-mark"><Icon name="book" size={14} /></span>
      <span>wolidate</span>
    </div>
    <div class="titlebar-title" data-tauri-drag-region></div>
    <div class="window-controls">
      <button aria-label="Minimize" title="Minimize" onclick={minimize}>
        <span class="control-minimize"></span>
      </button>
      <button
        aria-label={maximized ? 'Restore' : 'Maximize'}
        title={maximized ? 'Restore' : 'Maximize'}
        onclick={toggleMaximize}
      >
        <span class={maximized ? 'control-restore' : 'control-maximize'}></span>
      </button>
      <button class="control-close" aria-label="Close" title="Close" onclick={close}>
        <span class="control-close-icon"></span>
      </button>
    </div>
  </header>

  {#each directions as direction (direction)}
    <button
      class={`resize-handle resize-${direction.toLowerCase()}`}
      aria-label="Resize window"
      tabindex="-1"
      onpointerdown={(event) => resize(direction, event)}
    ></button>
  {/each}
{/if}
