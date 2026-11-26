<script lang="ts">
  import { onMount, tick } from 'svelte'
  import type { FoliateBook, View } from '../../vendor/foliate-js/view.js'
  import { Overlayer } from '../../vendor/foliate-js/overlayer.js'
  import { createFoliateView, loadBook } from '../../lib/reader/book-loader'
  import { TtsController } from '../../lib/reader/tts'
  import { reader } from '../../lib/stores/reader.svelte'
  import type { PageLayout, TocItem } from '../../lib/stores/reader.svelte'
  import type { Book } from '../../lib/ipc/commands'
  import Icon from '../Icon.svelte'
  import Sidebar from './Sidebar.svelte'
  import BookDetails from './BookDetails.svelte'
  import PdfPages from './PdfPages.svelte'
  import SelectionPopover from './SelectionPopover.svelte'
  import TypographyMenu from './TypographyMenu.svelte'

  type PdfApi = {
    goToPage(index: number): void
    next(): void
    prev(): void
    current(): number
  }

  let host: HTMLDivElement | undefined = $state()
  let view: View | null = null
  let ready = $state(false)
  let loading = $state(true)
  let error = $state<string | null>(null)
  let openedHash: string | null = null
  let tts = new TtsController()
  let renderedAnnotationIds: string[] = []
  let fixedLayout = $state(false)
  let pdfMode = $state(false)
  let foliateBook = $state<FoliateBook | null>(null)
  let pdfInitialIndex = $state(0)
  let pdfApi = $state<PdfApi | null>(null)
  let layoutOpen = $state(false)
  let appliedLayout: PageLayout | null = null

  const book = $derived(reader.activeBook)
  const progressLabel = $derived(`${Math.round(reader.fraction * 100)}%`)
  const locationLabel = $derived(
    reader.location
      ? `Loc. ${reader.location.current + 1} of ${reader.location.total}`
      : progressLabel,
  )

  const layoutOptions = $derived.by(() => {
    const page = pdfMode
    return [
      {
        id: 'scroll' as PageLayout,
        label: 'Scroll',
        detail: page ? 'Single page' : 'Single column',
        icon: 'scroll' as const,
      },
      {
        id: 'single' as PageLayout,
        label: page ? '1 page' : '1 column',
        detail: 'Normal page turn',
        icon: 'column-one' as const,
      },
      {
        id: 'spread' as PageLayout,
        label: page ? '2 pages' : '2 columns',
        detail: 'Side by side',
        icon: 'column-two' as const,
      },
    ]
  })

  function message(value: unknown): string {
    return value instanceof Error ? value.message : String(value)
  }

  function applyReflowLayout(layout: PageLayout): void {
    if (!view || view.isFixedLayout) return
    const renderer = view.renderer as
      | (HTMLElement & { render?: () => void })
      | undefined
    if (!renderer) return
    const count = layout === 'spread' ? '2' : '1'
    renderer.setAttribute('max-column-count', count)
    renderer.setAttribute('flow', layout === 'scroll' ? 'scrolled' : 'paginated')
    renderer.render?.()
  }

  function onPdfLocation(index: number): void {
    if (!view || !foliateBook) return
    const total = foliateBook.sections?.length ?? 0
    if (total === 0) return
    const clamped = Math.max(0, Math.min(total - 1, index))
    let cfi = ''
    try {
      cfi = view.getCFI(clamped)
    } catch {
      cfi = ''
    }
    reader.onRelocate({
      cfi: cfi || undefined,
      fraction: total > 1 ? clamped / (total - 1) : 0,
      location: { current: clamped, total },
    })
  }

  function nextPage(): void {
    if (pdfMode) pdfApi?.next()
    else void view?.next()
  }

  function prevPage(): void {
    if (pdfMode) pdfApi?.prev()
    else void view?.prev()
  }

  function closeLayoutMenu(event: MouseEvent): void {
    if (!layoutOpen) return
    const target = event.target
    if (
      target instanceof Element &&
      target.closest('.layout-menu, .layout-button')
    ) {
      return
    }
    layoutOpen = false
  }

  function applyStyles(doc: Document): void {
    const computed = getComputedStyle(document.documentElement)
    const background = computed.getPropertyValue('--reader-bg').trim() || '#fbfaf7'
    const foreground = computed.getPropertyValue('--reader-text').trim() || '#22211f'
    const link = computed.getPropertyValue('--reader-link').trim() || '#1f6f5c'
    const families = {
      serif: "'Iowan Old Style', 'Palatino Linotype', Georgia, serif",
      sans: "'Segoe UI Variable Text', 'Segoe UI', system-ui, sans-serif",
      mono: "ui-monospace, 'Cascadia Code', Consolas, monospace",
    }
    const family = families[reader.fontFamily]
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
        font-family: ${family} !important;
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

  function annotationColor(color: string): string {
    const colors: Record<string, string> = {
      yellow: '#f4c542',
      green: '#5fb37a',
      blue: '#5b9bd5',
      pink: '#df7fa4',
      red: '#d95c5c',
    }
    return colors[color.toLowerCase()] ?? colors.yellow
  }

  async function renderAnnotations(): Promise<void> {
    const contents = view?.renderer?.getContents?.() ?? []
    for (const content of contents) {
      for (const id of renderedAnnotationIds) content.overlayer?.remove(id)
    }
    renderedAnnotationIds = []
    if (!view) return

    for (const annotation of reader.annotations) {
      if (annotation.kind !== 'highlight') continue
      try {
        const resolved = await view.resolveCFI(annotation.cfi)
        const content = contents.find(
          (item) => item.index === resolved.index && item.overlayer,
        )
        if (!content) continue
        const range = resolved.anchor(content.doc)
        content.overlayer?.add(
          annotation.id,
          range,
          Overlayer.highlight,
          { color: annotationColor(annotation.color) },
        )
        renderedAnnotationIds.push(annotation.id)
      } catch {
        // A CFI from another rendition cannot be anchored in this document.
      }
    }
  }

  function onLoad(event: Event): void {
    const detail = (event as CustomEvent<{ doc: Document; index: number }>).detail
    applyStyles(detail.doc)
    installSelection(detail.doc, detail.index)
    void renderAnnotations()
  }

  function onRelocate(event: Event): void {
    if (pdfMode) return
    const detail = (event as CustomEvent<{
      cfi?: string
      fraction?: number
    }>).detail
    reader.onRelocate(detail)
    void tick().then(renderAnnotations)
  }

  async function openBook(target: Book): Promise<void> {
    if (!view) return
    loading = true
    error = null
    reader.setSelection(null)
    pdfMode = target.format.toLowerCase() === 'pdf'
    const initialCfi = reader.cfi
    const initialFraction = reader.fraction
    try {
      const foliate: FoliateBook = await loadBook(target)
      await view.open(foliate)
      openedHash = target.bookHash
      fixedLayout = Boolean(view.isFixedLayout)
      reader.toc = (foliate.toc ?? []) as TocItem[]
      if (pdfMode) {
        const total = foliate.sections?.length ?? 0
        let index = 0
        const initialTarget = initialCfi ?? { fraction: initialFraction }
        try {
          const resolved = await view.resolveNavigation(initialTarget)
          if (resolved && typeof resolved === 'object' && 'index' in resolved) {
            index = resolved.index
          }
        } catch {
          index = 0
        }
        pdfInitialIndex = Math.max(0, Math.min(Math.max(0, total - 1), index))
      }
      foliateBook = foliate
      loading = false
      await tick()
      if (pdfMode) {
        await tick()
        onPdfLocation(pdfInitialIndex)
      } else {
        applyReflowLayout(reader.pageLayout)
        appliedLayout = reader.pageLayout
        try {
          await view.goTo(initialCfi ?? 0)
        } catch {
          // A stale CFI from another device should not block the first section.
          try {
            await view.goTo(0)
          } catch {
            // Leave the renderer's own error state visible.
          }
        }
        applyStylesToAll()
        await renderAnnotations()
      }
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

  function seek(event: Event): void {
    const fraction = Number((event.currentTarget as HTMLInputElement).value) / 100
    if (pdfMode) {
      const total = foliateBook?.sections?.length ?? 0
      if (total > 0) pdfApi?.goToPage(Math.round(fraction * (total - 1)))
      return
    }
    void view?.goToFraction(fraction)
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

  function addBookmark(): void {
    if (!reader.cfi) return
    const existing = reader.annotations.filter(
      (annotation) =>
        annotation.kind === 'bookmark' && annotation.cfi === reader.cfi,
    )
    if (existing.length > 0) {
      for (const annotation of existing) {
        void reader.removeAnnotation(annotation.id)
      }
      return
    }
    void reader.addAnnotation({
      kind: 'bookmark',
      cfi: reader.cfi,
      quoteText: locationLabel,
    })
  }

  function toggleFullscreen(): void {
    if (document.fullscreenElement) void document.exitFullscreen()
    else void document.documentElement.requestFullscreen()
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
      if (layoutOpen) layoutOpen = false
      else if (reader.selection) reader.setSelection(null)
      else if (reader.typographyOpen || reader.detailsOpen) {
        reader.typographyOpen = false
        reader.detailsOpen = false
      } else if (reader.sidebarOpen) reader.sidebarOpen = false
      else reader.close()
    } else if (event.key === 'ArrowLeft') {
      prevPage()
    } else if (event.key === 'ArrowRight' || event.key === ' ') {
      event.preventDefault()
      nextPage()
    }
  }

  onMount(() => {
    let disposed = false
    const goTo = (event: Event): void => {
      const target = (event as CustomEvent<string>).detail
      if (!view || !target) return
      if (pdfMode) {
        void (async () => {
          try {
            const resolved = await view.resolveNavigation(target)
            if (
              resolved &&
              typeof resolved === 'object' &&
              'index' in resolved
            ) {
              pdfApi?.goToPage(resolved.index)
            }
          } catch {
            // A stale TOC target should not break the current page.
          }
        })()
      } else {
        void view.goTo(target)
      }
    }
    const toggleTtsEvent = (): void => toggleTts()
    window.addEventListener('foliate-go-to', goTo)
    window.addEventListener('wolidate-toggle-tts', toggleTtsEvent)
    window.addEventListener('keydown', onKeydown)
    window.addEventListener('click', closeLayoutMenu)

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
      window.removeEventListener('wolidate-toggle-tts', toggleTtsEvent)
      window.removeEventListener('keydown', onKeydown)
      window.removeEventListener('click', closeLayoutMenu)
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
    reader.resolvedTheme
    reader.fontFamily
    reader.fontSize
    reader.lineHeight
    if (!ready) return
    void tick().then(() => {
      applyStylesToAll()
      void renderAnnotations()
    })
  })

  $effect(() => {
    reader.annotations.length
    if (!ready) return
    void renderAnnotations()
  })

  $effect(() => {
    const layout = reader.pageLayout
    if (!book || !openedHash || !ready || !view || fixedLayout) return
    if (appliedLayout === layout) return
    appliedLayout = layout
    void tick().then(() => {
      const fraction = reader.fraction
      applyReflowLayout(layout)
      void view?.goToFraction(fraction)
    })
  })
</script>

<div class="reader">
  {#if reader.sidebarOpen}
    <Sidebar />
  {/if}

  <div class="reader-main">
    <div class="reader-view">
      <div class="reader-topbar">
        <div class="topbar-group">
          <button
            class="icon-button"
            aria-label="Sidebar"
            title="Sidebar"
            onclick={() => reader.toggleSidebar('toc')}
          >
            <Icon name="sidebar" size={18} />
          </button>
          <button
            class="icon-button"
            aria-label="Library"
            title="Library"
            onclick={() => reader.close()}
          >
            <Icon name="arrow-left" size={18} />
          </button>
        </div>
        <div class="topbar-group">
          <button
            class="icon-button"
            aria-label="Add bookmark"
            title="Add bookmark"
            onclick={addBookmark}
          >
            <Icon name="bookmark" size={18} />
          </button>
          {#if !fixedLayout || pdfMode}
            <button
              class="icon-button layout-button"
              class:active={layoutOpen}
              aria-label="Page layout"
              title="Page layout"
              aria-haspopup="menu"
              aria-expanded={layoutOpen}
              onclick={() => (layoutOpen = !layoutOpen)}
            >
              <Icon name="layout" size={18} />
            </button>
          {/if}
          <button
            class="icon-button"
            class:active={reader.typographyOpen}
            aria-label="Appearance"
            title="Appearance"
            onclick={() => reader.toggleTypography()}
          >
            <Icon name="type" size={18} />
          </button>
          <button
            class="icon-button"
            aria-label="Fullscreen"
            title="Fullscreen"
            onclick={toggleFullscreen}
          >
            <Icon name="maximize" size={17} />
          </button>
        </div>
      </div>

      {#if layoutOpen}
        <div class="layout-menu" role="menu" aria-label="Page layout">
          <div class="layout-menu-heading">
            {fixedLayout ? 'PDF page layout' : 'Page layout'}
          </div>
          {#each layoutOptions as option (option.id)}
            <button
              role="menuitemradio"
              aria-checked={reader.pageLayout === option.id}
              class:active={reader.pageLayout === option.id}
              onclick={() => {
                reader.setPageLayout(option.id)
                layoutOpen = false
              }}
            >
              <span class="layout-menu-icon">
                <Icon name={option.icon} size={17} />
              </span>
              <span class="layout-menu-label">{option.label}</span>
              <span class="layout-menu-detail">{option.detail}</span>
            </button>
          {/each}
        </div>
      {/if}

      <div class="foliate-host" class:hidden={pdfMode} bind:this={host}></div>

      {#if pdfMode && foliateBook}
        {#key foliateBook}
          <PdfPages
            book={foliateBook}
            mode={reader.pageLayout}
            initialIndex={pdfInitialIndex}
            onLocation={onPdfLocation}
            onReady={(api) => (pdfApi = api)}
          />
        {/key}
      {/if}

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
        <div class="page-location">{locationLabel}</div>
      {/if}
    </div>

    {#if !loading && !error}
      <footer class="reader-footer">
        <button
          class="icon-button footer-arrow"
          aria-label="Previous page"
          title="Previous page"
          onclick={prevPage}
        >
          <Icon name="chevron-left" size={17} />
        </button>
        {#if !pdfMode}
          <button
            class="icon-button footer-arrow"
            aria-label="Back"
            title="Back"
            onclick={() => view?.history?.back()}
          >
            <Icon name="undo" size={15} />
          </button>
          <button
            class="icon-button footer-arrow"
            aria-label="Forward"
            title="Forward"
            onclick={() => view?.history?.forward()}
          >
            <Icon name="redo" size={15} />
          </button>
        {/if}

        <div class="progress-box">
          <input
            class="progress-slider"
            type="range"
            min="0"
            max="100"
            step="0.1"
            value={reader.fraction * 100}
            aria-label="Reading progress"
            oninput={seek}
          />
        </div>

        <button class="location-button" title="Location">{locationLabel}</button>

        {#if !pdfMode}
          <button
            class="icon-button footer-arrow"
            class:active={reader.ttsActive}
            aria-label="Read aloud"
            title="Read aloud"
            onclick={toggleTts}
          >
            <Icon name="headphones" size={16} />
          </button>
        {/if}
        <button
          class="icon-button footer-arrow"
          aria-label="Next page"
          title="Next page"
          onclick={nextPage}
        >
          <Icon name="chevron-right" size={17} />
        </button>
      </footer>
    {/if}
  </div>

  {#if reader.selection}
    <SelectionPopover />
  {/if}

  {#if reader.typographyOpen}
    <TypographyMenu />
  {/if}

  {#if reader.detailsOpen}
    <BookDetails />
  {/if}
</div>
