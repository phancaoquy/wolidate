<script lang="ts">
  import { onMount } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import LibraryView from './components/library/LibraryView.svelte'
  import ReaderView from './components/reader/ReaderView.svelte'
  import TitleBar from './components/TitleBar.svelte'
  import { library } from './lib/stores/library.svelte'
  import { reader } from './lib/stores/reader.svelte'
  import { isTauri, takePendingOpen } from './lib/ipc/commands'

  let toast = $state<string | null>(null)
  let toastTimer: number | undefined

  function showToast(message: string): void {
    toast = message
    if (toastTimer !== undefined) window.clearTimeout(toastTimer)
    toastTimer = window.setTimeout(() => {
      toast = null
      toastTimer = undefined
    }, 5000)
  }

  async function handleOpenPaths(paths: string[]): Promise<void> {
    for (const path of paths) {
      const book = await library.importPath(path)
      if (book) {
        await reader.open(book)
        return
      }
    }
  }

  onMount(() => {
    if (!isTauri()) {
      void library.initialize()
      return
    }

    let active = true
    let unlistenOauth: (() => void) | undefined
    let unlistenOpen: (() => void) | undefined

    void listen<{ ok: boolean; message: string }>('oauth-complete', (event) => {
      if (!active) return
      showToast(event.payload.message)
      if (event.payload.ok) {
        void library.refreshSyncStatus()
        void library.refresh()
      }
    }).then((stop) => {
      if (active) unlistenOauth = stop
      else stop()
    })

    void listen<string[]>('open-file', (event) => {
      if (!active) return
      void handleOpenPaths(event.payload)
    }).then((stop) => {
      if (active) unlistenOpen = stop
      else stop()
    })

    void (async () => {
      await library.initialize()
      if (!active) return
      try {
        const paths = await takePendingOpen()
        if (active && paths.length > 0) await handleOpenPaths(paths)
      } catch {
        // No pending file is normal.
      }
    })()

    return () => {
      active = false
      unlistenOauth?.()
      unlistenOpen?.()
      if (toastTimer !== undefined) window.clearTimeout(toastTimer)
    }
  })
</script>

<div class="app-shell">
  <TitleBar />
  <main class="app-main">
    {#if reader.activeBook}
      <ReaderView />
    {:else}
      <LibraryView />
    {/if}
  </main>
</div>

{#if toast}
  <div class="toast" role="status">{toast}</div>
{/if}
