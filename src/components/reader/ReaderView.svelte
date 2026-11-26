<script lang="ts">
  import { onMount, tick } from 'svelte'
  import type { FoliateBook, View } from '../../vendor/foliate-js/view.js'
  import { createFoliateView, loadBook } from '../../lib/reader/book-loader'
  import { TtsController } from '../../lib/reader/tts'
  import { reader } from '../../lib/stores/reader.svelte'
  import type { TocItem } from '../../lib/stores/reader.svelte'
  import type { Book } from '../../lib/ipc/commands'
  import Sidebar from './Sidebar.svelte'
  import SelectionPopover from './SelectionPopover.svelte'
  import TypographyMenu from './TypographyMenu.svelte'

  let host: HTMLDivElement | undefined = $state()
  let view: View | null = null
  let ready = $state(false)
  let loading = $state(true)
  let error = $state<string | null>(null)
  let openedHash: string | null = null
  let tts = new TtsController()

  const book = $derived(reader.activeBook)
  const progressLabel = $derived(`${Math.round(reader.fraction * 100)}%`)

  function message(value: unknown): string {
    return value instanceof Error ? value.message : String(value)
  }

  function applyStyles(doc: Document): void {
    const computed = getComputedStyle(document.documentElement)
    const background = computed.getPropertyValue('--reader-bg').trim() || '#fbfaf7'
    const foreground = computed.getPropertyValue('--reader-text').trim() || '#22211f'
    const link = computed.getPropertyValue('--reader-link').trim() || '#1f6f5c'
    let style = doc.getElementById('foliate-user-style') as HTMLStyleElement | null
    if (!style) {
      style = doc.createElement('style')
      style.id = 'foliate-user-style'
      doc.head.append(style)
    }
    style.textContent = `
      html { background: ${background} !important; color: ${foreground} !important; }
      body {
        background: ${background} !important;
        color: ${foreground} !important;
        font-family: 'Iowan Old Style', 'Palatino Linotype', Georgia, serif !important;
        font-size: ${reader.fontSize}em !important;
        line-height: ${reader.lineHeight} !important;
      }
      a { color: ${link} !important; }
      img, svg { max-width: 100% !important; }
    `
  }

  function applyStylesToAll(): void {
    const contents = view?.renderer?.getContents?.() ?? []
    for (const { doc } of contents) applyStyles(doc)
  }

  function installSelection(doc: Document, index: number): void {
    doc.addEventListener('selectionchange', () => {
      const selection = doc.getSelection()
      if (!view || !selection || selection.isCollapsed || selection.rangeCount === 0) {
        reader.setSelection(null)
        return
      }
      const range = selection.getRangeAt(0)
      const frame = doc.defaultView?.frameElement as HTMLElement | null
      const frameRect = frame?.getBoundingClientRect()
      const rect = range.getBoundingClientRect()
      const text = selection.toString().trim()
      if (!frameRect || !text || rect.width === 0) {
        reader.setSelection(null)
        return
      }
      let cfi = ''
      try {
        cfi = view.getCFI(index, range)
      } catch {
        cfi = ''
      }
      if (!cfi) {
        reader.setSelection(null)
        return
      }
      reader.setSelection({
        text,
        cfi,
        x: frameRect.left + rect.left + rect.width / 2,
        y: frameRect.top + rect.top,
      })
    })
  }

  function onLoad(event: Event): void {
    const detail = (event as CustomEvent<{ doc: Document; index: number }>).detail
    applyStyles(detail.doc)
    installSelection(detail.doc, detail.index)
  }

  function onRelocate(event: Event): void {
    const detail = (event as CustomEvent<{
      cfi?: string
      fraction?: number
    }>).detail
    reader.onRelocate(detail)
  }

  async function openBook(target: Book): Promise<void> {
    if (!view) return
    loading = true
    error = null
    reader.setSelection(null)
    try {
      const foliate: FoliateBook = await loadBook(target)
      await view.open(foliate)
      openedHash = target.bookHash
      reader.toc = (foliate.toc ?? []) as TocItem[]
      loading = false
      await tick()
      if (reader.cfi) {
        try {
          await view.goTo(reader.cfi)
        } catch {
          // A stale CFI from another device should not block the book.
        }
      }
      applyStylesToAll()
    } catch (cause) {
      error = message(cause)
      loading = false
    }
  }

  function retry(): void {
    if (!book) return
    openedHash = null
    void openBook(book)
  }

  function toggleTts(): void {
    if (reader.ttsActive) {
      tts.stop()
      reader.ttsActive = false
      return
    }
    const contents = view?.renderer?.getContents?.() ?? []
    const text = contents.map(({ doc }) => doc.body?.innerText ?? '').join('\n\n')
    if (!text.trim()) return
    reader.ttsActive = true
    tts.speak(text, {
      onEnd: () => {
        reader.ttsActive = false
      },
      onError: (speechError) => {
        reader.ttsActive = false
        console.warn('speech synthesis failed:', speechError)
      },
    })
  }

  function onKeydown(event: KeyboardEvent): void {
    const target = event.target
    if (
      target instanceof HTMLElement &&
      target.closest('button, input, textarea, select, [contenteditable="true"]')
    ) {
      return
    }
    if (event.key === 'Escape') {
      if (reader.selection) reader.setSelection(null)
      else if (reader.typographyOpen || reader.sidebarTab) {
        reader.typographyOpen = false
        reader.sidebarTab = null
      } else reader.close()
    } else if (event.key === 'ArrowLeft') {
      void view?.prev()
    } else if (event.key === 'ArrowRight' || event.key === ' ') {
      event.preventDefault()
      void view?.next()
    }
  }

  onMount(() => {
    let disposed = false
    const goTo = (event: Event): void => {
      const target = (event as CustomEvent<string>).detail
      if (view && target) void view.goTo(target)
    }
    window.addEventListener('foliate-go-to', goTo)
    window.addEventListener('keydown', onKeydown)

    void (async () => {
      try {
        const element = await createFoliateView()
        if (disposed) return
        view = element
        element.style.cssText = 'display:block;width:100%;height:100%'
        element.addEventListener('load', onLoad as EventListener)
        element.addEventListener('relocate', onRelocate as EventListener)
        host?.append(element)
        ready = true
        await tick()
      } catch (cause) {
        error = message(cause)
        loading = false
      }
    })()

    return () => {
      disposed = true
      window.removeEventListener('foliate-go-to', goTo)
      window.removeEventListener('keydown', onKeydown)
      tts.stop()
      view?.close()
      view = null
      host?.replaceChildren()
    }
  })

  $effect(() => {
    if (!book || !ready || !view) return
    if (openedHash === book.bookHash) return
    void openBook(book)
  })

  $effect(() => {
    reader.theme
    reader.fontSize
    reader.lineHeight
    if (!ready) return
    void tick().then(applyStylesToAll)
  })
</script>

<div class="reader">
  <header class="reader-bar">
    <button
      class="icon-button"
      aria-label="Back to library"
      title="Back to library"
      onclick={() => reader.close()}
    >
      ←
    </button>

    <div class="book-heading">
      <strong>{book?.title ?? ''}</strong>
      <span>{book?.author || 'Unknown author'}</span>
    </div>

    <div class="reader-actions">
      <button
        class="icon-button"
        class:active={reader.sidebarTab === 'toc'}
        aria-label="Contents"
        title="Contents"
        onclick={() => reader.toggleSidebar('toc')}
      >
        ☰
      </button>
      <button
        class="icon-button"
        class:active={reader.sidebarTab === 'notes'}
        aria-label="Notes and highlights"
        title="Notes and highlights"
        onclick={() => reader.toggleSidebar('notes')}
      >
        ✎
      </button>
      <button
        class="icon-button"
        class:active={reader.sidebarTab === 'search'}
        aria-label="Search this book"
        title="Search this book"
        onclick={() => reader.toggleSidebar('search')}
      >
        ⌕
      </button>
      <button
        class="icon-button"
        class:active={reader.ttsActive}
        aria-label="Read aloud"
        title="Read aloud"
        onclick={toggleTts}
      >
        {reader.ttsActive ? '■' : '▶'}
      </button>
      <button
        class="icon-button"
        class:active={reader.typographyOpen}
        aria-label="Typography"
        title="Typography"
        onclick={() => reader.toggleTypography()}
      >
        Aa
      </button>
      <button
        class="icon-button"
        aria-label="Change theme"
        title="Change theme"
        onclick={() => reader.cycleTheme()}
      >
        {reader.theme === 'dark' ? '☀' : '☾'}
      </button>
    </div>
  </header>

  <div class="reader-body">
    <div class="reader-view">
      <div class="foliate-host" bind:this={host}></div>

      {#if loading || reader.loading}
        <div class="reader-loading">
          <div class="spinner"></div>
          <span>Opening {book?.title ?? 'book'}…</span>
        </div>
      {/if}

      {#if error}
        <div class="reader-error" role="alert">
          <strong>Could not open this book</strong>
          <span>{error}</span>
          <div class="reader-actions">
            <button class="secondary-button" onclick={retry}>Retry</button>
            <button class="secondary-button" onclick={() => reader.close()}>
              Back to library
            </button>
          </div>
        </div>
      {/if}

      {#if !loading && !error}
        <div class="reader-progress">{progressLabel}</div>
      {/if}
    </div>

    {#if reader.sidebarTab}
      <Sidebar />
    {/if}
  </div>

  {#if reader.selection}
    <SelectionPopover />
  {/if}

  {#if reader.typographyOpen}
    <TypographyMenu />
  {/if}
</div>
