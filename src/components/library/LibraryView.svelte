<script lang="ts">
  import { onMount } from 'svelte'
  import BookCard from './BookCard.svelte'
  import { library } from '../../lib/stores/library.svelte'
  import { reader } from '../../lib/stores/reader.svelte'

  const CARD_MIN_WIDTH = 152
  const OVERSCAN_ROWS = 2

  let scroller: HTMLDivElement | undefined = $state()
  let scrollTop = $state(0)
  let viewportWidth = $state(800)
  let viewportHeight = $state(600)

  let columns = $derived(Math.max(2, Math.floor(viewportWidth / CARD_MIN_WIDTH)))
  let cellWidth = $derived(viewportWidth / columns)
  let rowHeight = $derived(cellWidth * 1.5 + 110)
  let rowCount = $derived(Math.ceil(library.filtered.length / columns))
  let totalHeight = $derived(Math.max(0, rowCount * rowHeight))
  let startRow = $derived(
    Math.max(0, Math.floor(scrollTop / rowHeight) - OVERSCAN_ROWS),
  )
  let endRow = $derived(
    Math.min(
      rowCount,
      Math.ceil((scrollTop + viewportHeight) / rowHeight) + OVERSCAN_ROWS,
    ),
  )

  let cells = $derived.by(() => {
    const result: Array<{
      book: (typeof library.books)[number]
      top: number
      left: number
      width: number
    }> = []
    for (let row = startRow; row < endRow; row++) {
      for (let column = 0; column < columns; column++) {
        const index = row * columns + column
        const book = library.filtered[index]
        if (!book) continue
        result.push({
          book,
          top: row * rowHeight,
          left: column * cellWidth,
          width: cellWidth,
        })
      }
    }
    return result
  })

  function measure(): void {
    if (!scroller) return
    const styles = getComputedStyle(scroller)
    viewportWidth =
      scroller.clientWidth -
      Number.parseFloat(styles.paddingLeft) -
      Number.parseFloat(styles.paddingRight)
    viewportHeight = scroller.clientHeight
  }

  function onScroll(event: Event): void {
    scrollTop = (event.currentTarget as HTMLDivElement).scrollTop
  }

  async function importNow(): Promise<void> {
    const book = await library.importBook()
    if (book) await reader.open(book)
  }

  function newCollection(): void {
    const name = window.prompt('New collection name')
    if (name?.trim()) void library.addCollection(name)
  }

  onMount(() => {
    if (!scroller) return
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(scroller)
    return () => observer.disconnect()
  })
</script>

<div class="library">
  <header class="library-header">
    <div>
      <h1 class="library-title">wolidate</h1>
      <p class="library-subtitle">
        {library.books.length}
        {library.books.length === 1 ? 'book' : 'books'}
        {#if library.activeCollection}
          in this collection
        {/if}
      </p>
    </div>

    <div class="library-header-actions">
      <span class="status-line">
        <span
          class="status-dot"
          class:online={library.syncStatus.connected}
        ></span>
        {library.syncStatus.connected ? 'Drive linked' : 'Local only'}
      </span>

      {#if library.syncStatus.connected}
        <button
          class="secondary-button"
          onclick={() => void library.sync()}
          disabled={library.syncing}
        >
          {library.syncing ? 'Syncing…' : 'Sync'}
        </button>
      {:else}
        <button
          class="secondary-button"
          onclick={() => void library.connectDrive()}
        >
          Connect Drive
        </button>
      {/if}

      <button
        class="primary-button"
        onclick={() => void importNow()}
        disabled={library.importing}
      >
        {library.importing ? 'Importing…' : '+ Add book'}
      </button>
    </div>
  </header>

  <div class="library-toolbar">
    <label class="search-box">
      <input
        type="search"
        placeholder="Search title or author"
        bind:value={library.search}
        aria-label="Search library"
      />
    </label>

    <div class="chip-row" aria-label="Collections">
      <button
        class="chip"
        class:active={library.activeCollection === null}
        aria-pressed={library.activeCollection === null}
        onclick={() => (library.activeCollection = null)}
      >
        All books
      </button>
      {#each library.collections as collection (collection.id)}
        <button
          class="chip"
          class:active={library.activeCollection === collection.id}
          aria-pressed={library.activeCollection === collection.id}
          onclick={() => (library.activeCollection = collection.id)}
        >
          {collection.name}
        </button>
      {/each}
      <button class="chip" onclick={newCollection}>+ Collection</button>
    </div>
  </div>

  {#if library.error}
    <div class="banner" role="alert">{library.error}</div>
  {/if}

  <div class="library-body">
    <div class="grid-scroll" bind:this={scroller} onscroll={onScroll}>
      {#if library.loading && library.books.length === 0}
        <div class="empty-state">
          <div class="spinner"></div>
          <span>Loading library…</span>
        </div>
      {:else if library.filtered.length === 0}
        <div class="empty-state">
          <strong>
            {library.books.length === 0 ? 'Your library is empty' : 'No books match'}
          </strong>
          <span>
            {library.books.length === 0
              ? 'Add an EPUB, MOBI, FB2, CBZ, or PDF to begin.'
              : 'Try a different search or collection.'}
          </span>
          {#if library.books.length === 0}
            <button class="primary-button" onclick={() => void importNow()}>
              + Add your first book
            </button>
          {/if}
        </div>
      {:else}
        <div class="grid-spacer" style={`height:${totalHeight}px`}>
          {#each cells as cell (cell.book.bookHash)}
            <div
              class="grid-cell"
              style={`top:${cell.top}px;left:${cell.left}px;width:${cell.width}px;height:${rowHeight - 16}px`}
            >
              <BookCard book={cell.book} />
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</div>
