<script lang="ts">
  import { onMount, tick, untrack } from 'svelte'
  import type { FoliateBook } from '../../vendor/foliate-js/view.js'
  import type { PageLayout } from '../../lib/stores/reader.svelte'

  export interface PdfPagesApi {
    goToPage(index: number): void
    next(): void
    prev(): void
    current(): number
  }

  let {
    book,
    mode,
    initialIndex = 0,
    onLocation,
    onReady,
  }: {
    book: FoliateBook
    mode: PageLayout
    initialIndex?: number
    onLocation: (index: number) => void
    onReady?: (api: PdfPagesApi | null) => void
  } = $props()

  type PageSource = {
    src: string
    onZoom?: (options: { doc: Document; scale: number }) => void
  }

  type CachedFrame = {
    index: number
    iframe: HTMLIFrameElement
    width: number
    height: number
    onZoom?: (options: { doc: Document; scale: number }) => void
  }

  let scroller = $state<HTMLDivElement | undefined>()
  let current = $state(0)
  let destroyed = false
  let lastReported = -1
  let resizeTimer: number | undefined
  let scrollFrame = 0
  let resizeObserver: ResizeObserver | undefined

  const sourceCache = new Map<number, Promise<PageSource | null>>()
  const framePromises = new WeakMap<HTMLElement, Promise<CachedFrame | null>>()
  const nodeFrames = new WeakMap<HTMLElement, CachedFrame>()

  const pageCount = $derived(book.sections?.length ?? 0)
  const perView = $derived(mode === 'spread' ? 2 : 1)
  const groupStart = $derived(Math.floor(current / perView) * perView)
  const groupIndices = $derived.by(() => {
    if (mode === 'scroll' || pageCount === 0) return []
    return Array.from(
      { length: Math.min(perView, pageCount - groupStart) },
      (_, offset) => groupStart + offset,
    )
  })

  function clamp(index: number): number {
    return Math.max(0, Math.min(pageCount - 1, Math.round(index)))
  }

  function report(index: number): void {
    if (index === lastReported) return
    lastReported = index
    onLocation(index)
  }

  function viewportFromDoc(doc: Document): { width: number; height: number } {
    const content =
      doc.querySelector('meta[name="viewport"]')?.getAttribute('content') ?? ''
    const width = Number(content.match(/width=([\d.]+)/)?.[1] ?? 1000)
    const height = Number(content.match(/height=([\d.]+)/)?.[1] ?? 1414)
    return {
      width: Number.isFinite(width) && width > 0 ? width : 1000,
      height: Number.isFinite(height) && height > 0 ? height : 1414,
    }
  }

  async function loadSource(index: number): Promise<PageSource | null> {
    const existing = sourceCache.get(index)
    if (existing) return existing
    const promise = (async () => {
      const section = book.sections?.[index]
      if (!section?.load) return null
      const loaded = await section.load()
      return typeof loaded === 'string'
        ? { src: loaded }
        : { src: loaded.src, onZoom: loaded.onZoom }
    })()
    sourceCache.set(index, promise)
    return promise
  }

  async function ensureFrame(
    index: number,
    node: HTMLElement,
  ): Promise<CachedFrame | null> {
    const existing = nodeFrames.get(node)
    if (existing) return existing
    const pending = framePromises.get(node)
    if (pending) return pending

    const promise = (async () => {
      node.classList.add('loading')
      const source = await loadSource(index)
      if (!source || destroyed) return null
      const iframe = document.createElement('iframe')
      iframe.className = 'pdf-frame'
      iframe.setAttribute('sandbox', 'allow-same-origin allow-scripts')
      iframe.setAttribute('scrolling', 'no')
      iframe.style.visibility = 'hidden'
      node.append(iframe)
      await new Promise<void>((resolve, reject) => {
        iframe.addEventListener('load', () => resolve(), { once: true })
        iframe.addEventListener(
          'error',
          () => reject(new Error('PDF page failed')),
          { once: true },
        )
        iframe.src = source.src
      })
      if (destroyed) return null
      const doc = iframe.contentDocument
      if (!doc) return null
      const { width, height } = viewportFromDoc(doc)
      const frame: CachedFrame = {
        index,
        iframe,
        width,
        height,
        onZoom: source.onZoom,
      }
      nodeFrames.set(node, frame)
      return frame
    })()
    framePromises.set(node, promise)
    return promise
  }

  function scaleForFrame(frame: CachedFrame): number {
    const available = Math.min(
      Math.max(320, scroller?.clientWidth ?? 800) - 48,
      1100,
    )
    return Math.max(0.1, available / frame.width)
  }

  function applyScale(frame: CachedFrame, scale: number): void {
    const doc = frame.iframe.contentDocument
    if (!doc) return
    frame.onZoom?.({ doc, scale })
    frame.iframe.style.width = `${frame.width * scale}px`
    frame.iframe.style.height = `${frame.height * scale}px`
    frame.iframe.style.transform = 'none'
  }

  function applyGroupScale(frames: CachedFrame[]): void {
    if (!scroller || frames.length === 0) return
    const gap = 12
    const availableWidth = Math.max(320, scroller.clientWidth - 48)
    const availableHeight = Math.max(240, scroller.clientHeight - 104)
    const totalWidth = frames.reduce((sum, frame) => sum + frame.width, 0)
    const maxHeight = Math.max(...frames.map((frame) => frame.height))
    const widthScale = (availableWidth - gap * (frames.length - 1)) / totalWidth
    const scale = Math.max(
      0.05,
      Math.min(widthScale, availableHeight / maxHeight),
    )
    for (const frame of frames) applyScale(frame, scale)
  }

  function frameForNode(index: number): CachedFrame | undefined {
    const node = scroller?.querySelector<HTMLElement>(
      `.pdf-page[data-index="${index}"]`,
    )
    return node ? nodeFrames.get(node) : undefined
  }

  function currentGroupFrames(): CachedFrame[] {
    return groupIndices
      .map((index) => frameForNode(index))
      .filter((frame): frame is CachedFrame => Boolean(frame))
  }

  function revealFrame(node: HTMLElement, frame: CachedFrame): void {
    node.classList.remove('loading')
    node.classList.add('loaded')
    node.style.removeProperty('aspect-ratio')
    frame.iframe.style.visibility = 'visible'
  }

  async function attachFrame(index: number, node: HTMLElement): Promise<void> {
    const frame = await ensureFrame(index, node)
    if (!frame || destroyed) return
    revealFrame(node, frame)
    if (mode === 'scroll') applyScale(frame, scaleForFrame(frame))
    else applyGroupScale(currentGroupFrames())
  }

  async function layoutGroup(): Promise<void> {
    if (mode === 'scroll') return
    const nodes = groupIndices.map((index) =>
      scroller?.querySelector<HTMLElement>(`.pdf-page[data-index="${index}"]`),
    )
    const frames = await Promise.all(
      groupIndices.map((index, position) => {
        const node = nodes[position]
        return node ? ensureFrame(index, node) : null
      }),
    )
    if (destroyed) return
    for (let position = 0; position < frames.length; position++) {
      const frame = frames[position]
      const node = nodes[position]
      if (frame && node) revealFrame(node, frame)
    }
    applyGroupScale(
      frames.filter((frame): frame is CachedFrame => Boolean(frame)),
    )
  }

  function scrollToPage(index: number): void {
    if (!scroller) return
    const node = scroller.querySelector<HTMLElement>(
      `.pdf-page[data-index="${index}"]`,
    )
    if (!node) return
    const hostTop = scroller.getBoundingClientRect().top
    scroller.scrollTop += node.getBoundingClientRect().top - hostTop - 8
  }

  function goToPage(index: number): void {
    if (pageCount === 0) return
    const target = clamp(index)
    current = target
    report(target)
    if (mode === 'scroll') {
      void tick().then(() => {
        scrollToPage(target)
        pruneFrames(target)
      })
    }
  }

  function next(): void {
    if (mode === 'scroll') goToPage(current + 1)
    else goToPage(groupStart + perView)
  }

  function prev(): void {
    if (mode === 'scroll') goToPage(current - 1)
    else goToPage(groupStart - perView)
  }

  function pruneFrames(center: number): void {
    if (!scroller) return
    const pages = scroller.querySelectorAll<HTMLElement>('.pdf-page[data-index]')
    for (const page of pages) {
      const index = Number(page.dataset.index)
      if (Math.abs(index - center) <= 6) continue
      const frame = nodeFrames.get(page)
      if (!frame) continue
      frame.iframe.remove()
      nodeFrames.delete(page)
      framePromises.delete(page)
      page.classList.remove('loaded', 'loading')
      page.style.removeProperty('aspect-ratio')
    }
  }

  function onScroll(): void {
    if (mode !== 'scroll' || !scroller || scrollFrame) return
    scrollFrame = requestAnimationFrame(() => {
      scrollFrame = 0
      if (!scroller) return
      const pages = Array.from(
        scroller.querySelectorAll<HTMLElement>('.pdf-page[data-index]'),
      )
      if (pages.length === 0) return
      const hostTop = scroller.getBoundingClientRect().top
      const marker = hostTop + scroller.clientHeight * 0.35
      let visible = 0
      for (const page of pages) {
        if (page.getBoundingClientRect().top <= marker) {
          visible = Number(page.dataset.index)
        } else {
          break
        }
      }
      if (visible === lastReported) return
      current = visible
      report(visible)
      pruneFrames(visible)
    })
  }

  function onResize(): void {
    if (resizeTimer !== undefined) window.clearTimeout(resizeTimer)
    resizeTimer = window.setTimeout(() => {
      resizeTimer = undefined
      if (destroyed) return
      if (mode === 'scroll') {
        const pages = scroller?.querySelectorAll<HTMLElement>('.pdf-page') ?? []
        for (const page of pages) {
          const frame = nodeFrames.get(page)
          if (frame) applyScale(frame, scaleForFrame(frame))
        }
      } else {
        applyGroupScale(currentGroupFrames())
      }
    }, 120)
  }

  function lazyPage(node: HTMLElement, index: number) {
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) void attachFrame(index, node)
        }
      },
      { root: scroller ?? null, rootMargin: '150% 0px' },
    )
    observer.observe(node)
    return {
      destroy() {
        observer.disconnect()
      },
    }
  }

  function groupPage(node: HTMLElement, index: number) {
    void attachFrame(index, node)
  }

  onMount(() => {
    current = clamp(initialIndex)
    onReady?.({ goToPage, next, prev, current: () => current })
    if (scroller) {
      resizeObserver = new ResizeObserver(onResize)
      resizeObserver.observe(scroller)
    }
    void tick().then(() => {
      if (mode === 'scroll') {
        scrollToPage(current)
        report(current)
      } else {
        void layoutGroup().then(() => report(current))
      }
    })
    return () => {
      destroyed = true
      if (scrollFrame) cancelAnimationFrame(scrollFrame)
      if (resizeTimer !== undefined) window.clearTimeout(resizeTimer)
      resizeObserver?.disconnect()
      onReady?.(null)
    }
  })

  $effect(() => {
    const layout = mode
    if (!scroller) return
    if (layout === 'scroll') {
      const target = untrack(() => current)
      void tick().then(() => {
        scrollToPage(target)
        report(target)
        pruneFrames(target)
      })
    } else {
      const start = groupStart
      void start
      void tick().then(() => void layoutGroup())
    }
  })
</script>

<div class="pdf-pages" class:scrolling={mode === 'scroll'}>
  <div class="pdf-scroll-host" bind:this={scroller} onscroll={onScroll}>
    {#if mode === 'scroll'}
      {#each Array.from({ length: pageCount }, (_, index) => index) as index (index)}
        <div class="pdf-page" data-index={index} use:lazyPage={index}>
          <div class="pdf-page-placeholder">
            <span>Page {index + 1}</span>
          </div>
        </div>
      {/each}
    {:else}
      <div class="pdf-group">
        {#each groupIndices as index (index)}
          <div class="pdf-page" data-index={index} use:groupPage={index}>
            <div class="pdf-page-placeholder">
              <span>Page {index + 1}</span>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .pdf-pages {
    position: absolute;
    z-index: 2;
    inset: 0;
    display: flex;
    overflow: hidden;
    background: var(--reader-desk);
  }

  .pdf-scroll-host {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
    padding: 16px 24px 72px;
  }

  .pdf-pages:not(.scrolling) .pdf-scroll-host {
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    padding: 16px;
  }

  .pdf-group {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    max-width: 100%;
    max-height: 100%;
  }

  .pdf-page {
    position: relative;
    flex: 0 0 auto;
    background: #fff;
    box-shadow:
      0 1px 3px rgb(0 0 0 / 20%),
      0 16px 38px rgb(0 0 0 / 14%);
  }

  .pdf-pages.scrolling .pdf-page {
    width: min(100%, 1100px);
    margin: 0 auto 16px;
    aspect-ratio: 1 / 1.414;
  }

  .pdf-pages.scrolling :global(.pdf-page.loaded) {
    aspect-ratio: auto;
  }

  .pdf-pages:not(.scrolling) .pdf-page {
    min-width: 160px;
    min-height: 220px;
  }

  .pdf-pages:not(.scrolling) :global(.pdf-page.loaded) {
    min-width: 0;
    min-height: 0;
  }

  .pdf-page-placeholder {
    display: grid;
    width: 100%;
    height: 100%;
    min-height: 220px;
    place-items: center;
    color: #777;
    font: 12px/1.4 var(--font-ui);
    background: #f3f3f1;
  }

  :global(.pdf-page.loaded) .pdf-page-placeholder {
    display: none;
  }

  :global(.pdf-frame) {
    display: block;
    border: 0;
    background: #fff;
  }
</style>
