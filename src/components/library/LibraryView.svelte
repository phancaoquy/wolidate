<script lang="ts">
  import { onMount } from 'svelte'
  import BookCard from './BookCard.svelte'
  import Icon from '../Icon.svelte'
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

  type OnlineSource = { name: string; url: string }

  let sidebarOpen = $state(true)
  let collectionMenu = $state<string | null>(null)
  let collectionDialog = $state<{ mode: 'create' | 'rename'; id?: string } | null>(
    null,
  )
  let collectionName = $state('')
  let deleteTarget = $state<{ id: string; name: string } | null>(null)
  let sourceDialog = $state(false)
  let sourceName = $state('')
  let sourceUrl = $state('')
  let sources = $state<OnlineSource[]>([])

  let activeCollectionName = $derived(
    library.collections.find((item) => item.id === library.activeCollection)?.name ?? null,
  )

  function countForCollection(id: string): number {
    return library.memberships.filter((row) => row.collectionId === id).length
  }

  function selectCollection(id: string | null): void {
    library.activeCollection = id
    collectionMenu = null
    if (window.matchMedia('(max-width: 899px)').matches) sidebarOpen = false
  }

  function renameCollection(id: string, current: string): void {
    collectionMenu = null
    collectionName = current
    collectionDialog = { mode: 'rename', id }
  }

  function deleteCollection(id: string, name: string): void {
    collectionMenu = null
    deleteTarget = { id, name }
  }

  async function submitCollection(): Promise<void> {
    const name = collectionName.trim()
    if (!name || !collectionDialog) return
    if (collectionDialog.mode === 'create') {
      await library.addCollection(name)
    } else if (collectionDialog.id) {
      await library.renameCollection(collectionDialog.id, name)
    }
    collectionDialog = null
  }

  async function confirmDeleteCollection(): Promise<void> {
    if (!deleteTarget) return
    await library.removeCollection(deleteTarget.id)
    deleteTarget = null
  }

  function saveSources(): void {
    localStorage.setItem('wolidate.sources', JSON.stringify(sources))
  }

  function addSource(): void {
    const name = sourceName.trim()
    const url = sourceUrl.trim()
    if (!name || !url) return
    sources = [...sources, { name, url }]
    saveSources()
    sourceName = ''
    sourceUrl = ''
    sourceDialog = false
  }

  function removeSource(url: string): void {
    sources = sources.filter((source) => source.url !== url)
    saveSources()
  }

  function setTheme(theme: 'system' | 'light' | 'dark'): void {
    reader.setTheme(theme)
  }

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
    collectionName = ''
    collectionDialog = { mode: 'create' }
  }

  onMount(() => {
    const savedSources = localStorage.getItem('wolidate.sources')
    if (savedSources) {
      try {
        const parsed = JSON.parse(savedSources)
        if (Array.isArray(parsed)) sources = parsed
      } catch {
        // Ignore a malformed local cache.
      }
    }
    if (window.matchMedia('(max-width: 899px)').matches) sidebarOpen = false
    if (!scroller) return
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(scroller)
    return () => observer.disconnect()
  })
</script>

<div class="library">
  <button
    class="library-sidebar-scrim"
    class:visible={sidebarOpen}
    aria-label="Close navigation"
    onclick={() => (sidebarOpen = false)}
  ></button>

  <aside class="library-sidebar" class:open={sidebarOpen}>
    <div class="library-sidebar-top">
      <span class="brand-mark"><Icon name="book" size={18} /></span>
      <div class="library-sidebar-brand">
        <strong>wolidate</strong>
        <span>Library</span>
      </div>
      <button
        class="icon-button sidebar-close"
        aria-label="Close navigation"
        onclick={() => (sidebarOpen = false)}
      >
        <Icon name="close" size={16} />
      </button>
    </div>

    <div class="library-nav">
      <section class="nav-section">
        <div class="nav-heading">Library</div>
        <button
          class="nav-row"
          class:active={library.activeCollection === null}
          onclick={() => selectCollection(null)}
        >
          <Icon name="grid" size={16} />
          <span class="nav-label">All Books</span>
          <span class="nav-count">{library.books.length}</span>
        </button>
      </section>

      <section class="nav-section">
        <div class="nav-heading-row">
          <span class="nav-heading">Collections</span>
          <button class="nav-add" aria-label="New collection" title="New collection" onclick={newCollection}>
            <Icon name="plus" size={14} />
          </button>
        </div>
        {#each library.collections as collection (collection.id)}
          <div class="nav-row-wrap">
            <button
              class="nav-row"
              class:active={library.activeCollection === collection.id}
              onclick={() => selectCollection(collection.id)}
            >
              <Icon name="collection" size={16} />
              <span class="nav-label">{collection.name}</span>
              <span class="nav-count">{countForCollection(collection.id)}</span>
            </button>
            <button
              class="nav-more"
              aria-label={`Actions for ${collection.name}`}
              title="Collection actions"
              onclick={() => (collectionMenu = collectionMenu === collection.id ? null : collection.id)}
            >
              <Icon name="more" size={15} />
            </button>
            {#if collectionMenu === collection.id}
              <div class="nav-menu">
                <button onclick={() => renameCollection(collection.id, collection.name)}>
                  <Icon name="pencil" size={14} />
                  Rename
                </button>
                <button class="danger" onclick={() => deleteCollection(collection.id, collection.name)}>
                  <Icon name="trash" size={14} />
                  Delete
                </button>
              </div>
            {/if}
          </div>
        {/each}
        {#if library.collections.length === 0}
          <p class="nav-empty">Create collections to group your books.</p>
        {/if}
      </section>

      <section class="nav-section">
        <div class="nav-heading-row">
          <span class="nav-heading">Online Sources</span>
          <button class="nav-add" aria-label="Add online source" title="Add online source" onclick={() => (sourceDialog = true)}>
            <Icon name="plus" size={14} />
          </button>
        </div>
        {#each sources as source (source.url)}
          <div class="nav-row-wrap">
            <button
              class="nav-row"
              onclick={() => {
                library.error = `Online source "${source.name}" is saved. OPDS browsing is not enabled in this build.`
                if (window.matchMedia('(max-width: 899px)').matches) sidebarOpen = false
              }}
            >
              <Icon name="globe" size={16} />
              <span class="nav-label">{source.name}</span>
            </button>
            <button class="nav-more" aria-label={`Remove ${source.name}`} onclick={() => removeSource(source.url)}>
              <Icon name="trash" size={14} />
            </button>
          </div>
        {/each}
        <button class="nav-row nav-row-add" onclick={() => (sourceDialog = true)}>
          <Icon name="globe" size={16} />
          <span class="nav-label">Add Online Source…</span>
        </button>
      </section>
    </div>

    <div class="library-sidebar-footer">
      <span class="theme-label">Theme</span>
      <div class="theme-switcher" role="group" aria-label="Theme">
        <button
          class:active={reader.theme === 'system'}
          aria-label="System theme"
          title="System theme"
          onclick={() => setTheme('system')}
        >
          <Icon name="monitor" size={15} />
        </button>
        <button
          class:active={reader.theme === 'light'}
          aria-label="Light theme"
          title="Light theme"
          onclick={() => setTheme('light')}
        >
          <Icon name="sun" size={15} />
        </button>
        <button
          class:active={reader.theme === 'sepia'}
          aria-label="Sepia theme"
          title="Sepia theme"
          onclick={() => reader.setTheme('sepia')}
        >
          <Icon name="book" size={15} />
        </button>
        <button
          class:active={reader.theme === 'dark'}
          aria-label="Dark theme"
          title="Dark theme"
          onclick={() => setTheme('dark')}
        >
          <Icon name="moon" size={15} />
        </button>
      </div>
    </div>
  </aside>

  <div class="library-main">
    <header class="library-header">
      <button
        class="icon-button sidebar-toggle"
        aria-label="Navigation"
        title="Navigation"
        onclick={() => (sidebarOpen = !sidebarOpen)}
      >
        <Icon name="sidebar" size={18} />
      </button>
      <div class="library-heading">
        <h1 class="library-title">Library</h1>
        <p class="library-subtitle">
          {#if activeCollectionName}
            {library.filtered.length}
            {library.filtered.length === 1 ? 'book' : 'books'}
            in {activeCollectionName}
          {:else}
            {library.books.length}
            {library.books.length === 1 ? 'book' : 'books'}
          {/if}
        </p>
      </div>

      <div class="library-header-actions">
        {#if library.syncStatus.connected}
          <button
            class="icon-button"
            aria-label="Sync library"
            title="Sync library"
            onclick={() => void library.sync()}
            disabled={library.syncing}
          >
            <Icon name="sync" size={17} />
          </button>
        {/if}
        <button
          class="primary-button"
          onclick={() => void importNow()}
          disabled={library.importing}
        >
          <Icon name="plus" size={17} />
          {library.importing ? 'Importing…' : 'Add book'}
        </button>
      </div>
    </header>

    <div class="library-toolbar">
      <label class="search-box">
        <span class="search-icon"><Icon name="search" size={16} /></span>
        <input
          type="search"
          placeholder="Search library…"
          bind:value={library.search}
          aria-label="Search library"
        />
      </label>

      {#if activeCollectionName}
        <button class="filter-pill" onclick={() => selectCollection(null)}>
          {activeCollectionName}
          <Icon name="close" size={12} />
        </button>
      {/if}

      {#if !library.syncStatus.connected}
        <button
          class="secondary-button connect-button"
          onclick={() => void library.connectDrive()}
        >
          <Icon name="sync" size={15} />
          Connect Drive
        </button>
      {/if}
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
            <span class="empty-icon"><Icon name="book" size={32} /></span>
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
                <Icon name="plus" size={16} />
                Add your first book
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

  {#if sourceDialog}
    <div class="modal-backdrop">
      <div class="source-dialog" role="dialog" aria-modal="true" aria-label="Add online source">
        <header class="popover-header">
          <h2>Add Online Source</h2>
          <button class="icon-button" aria-label="Close" onclick={() => (sourceDialog = false)}>
            <Icon name="close" size={16} />
          </button>
        </header>
        <p class="source-help">
          Save an OPDS catalog URL. Browsing requires the OPDS layer, which is
          not enabled in this build.
        </p>
        <label class="source-field">
          <span>Name</span>
          <input bind:value={sourceName} placeholder="Standard Ebooks" />
        </label>
        <label class="source-field">
          <span>Catalog URL</span>
          <input bind:value={sourceUrl} placeholder="https://example.org/catalog.opds" />
        </label>
        <div class="source-actions">
          <button class="secondary-button" onclick={() => (sourceDialog = false)}>Cancel</button>
          <button class="primary-button" onclick={addSource} disabled={!sourceName.trim() || !sourceUrl.trim()}>
            Save source
          </button>
        </div>
      </div>
    </div>
  {/if}

  {#if collectionDialog}
    <div class="modal-backdrop">
      <div
        class="source-dialog collection-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={collectionDialog.mode === 'create'
          ? 'New collection'
          : 'Rename collection'}
      >
        <form
          onsubmit={(event) => {
            event.preventDefault()
            void submitCollection()
          }}
        >
          <header class="popover-header">
            <h2>
              {collectionDialog.mode === 'create' ? 'New Collection' : 'Rename Collection'}
            </h2>
            <button
              type="button"
              class="icon-button"
              aria-label="Close"
              onclick={() => (collectionDialog = null)}
            >
              <Icon name="close" size={16} />
            </button>
          </header>
          <label class="source-field">
            <span>Name</span>
            <input
              bind:value={collectionName}
              placeholder="Favorites"
              maxlength="80"
            />
          </label>
          <div class="source-actions">
            <button
              type="button"
              class="secondary-button"
              onclick={() => (collectionDialog = null)}
            >
              Cancel
            </button>
            <button
              type="submit"
              class="primary-button"
              disabled={!collectionName.trim()}
            >
              {collectionDialog.mode === 'create' ? 'Create' : 'Save'}
            </button>
          </div>
        </form>
      </div>
    </div>
  {/if}

  {#if deleteTarget}
    <div class="modal-backdrop">
      <div
        class="source-dialog collection-dialog"
        role="alertdialog"
        aria-modal="true"
        aria-label="Delete collection"
      >
        <header class="popover-header">
          <h2>Delete Collection</h2>
          <button
            class="icon-button"
            aria-label="Close"
            onclick={() => (deleteTarget = null)}
          >
            <Icon name="close" size={16} />
          </button>
        </header>
        <p class="source-help">
          Delete “{deleteTarget.name}”? The books stay in your library.
        </p>
        <div class="source-actions">
          <button class="secondary-button" onclick={() => (deleteTarget = null)}>
            Cancel
          </button>
          <button class="danger-button" onclick={() => void confirmDeleteCollection()}>
            Delete
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
