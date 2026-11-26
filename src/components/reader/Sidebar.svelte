<script lang="ts">
  import Icon from '../Icon.svelte'
  import { reader } from '../../lib/stores/reader.svelte'
  import type { TocItem } from '../../lib/stores/reader.svelte'

  let query = $state('')
  let annotationFilter = $state('')
  let menuOpen = $state(false)
  let coverFailed = $state(false)

  const book = $derived(reader.activeBook)
  const coverUrl = $derived(
    book ? `http://book.localhost/${book.bookHash}/__cover` : '',
  )
  const initials = $derived(
    (book?.title ?? '?')
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((word) => word[0]?.toUpperCase() ?? '')
      .join(''),
  )

  type FlatTocItem = TocItem & {
    depth: number
    key: string
    hasChildren: boolean
  }

  let expandedToc = $state<Set<string>>(new Set())
  let lastBookHash = ''

  function tocKey(parent: string, index: number): string {
    return `${parent}/${index}`
  }

  const flatToc = $derived.by(() => {
    const result: FlatTocItem[] = []
    const walk = (items: TocItem[], depth: number, parent: string): void => {
      items.forEach((item, index) => {
        const key = tocKey(parent, index)
        result.push({
          ...item,
          depth,
          key,
          hasChildren: Boolean(item.subitems?.length),
        })
        if (item.subitems?.length) walk(item.subitems, depth + 1, key)
      })
    }
    walk(reader.toc, 0, 'root')
    return result
  })

  const visibleToc = $derived.by(() => {
    const result: FlatTocItem[] = []
    const walk = (items: TocItem[], depth: number, parent: string): void => {
      items.forEach((item, index) => {
        const key = tocKey(parent, index)
        result.push({
          ...item,
          depth,
          key,
          hasChildren: Boolean(item.subitems?.length),
        })
        if (item.subitems?.length && expandedToc.has(key)) {
          walk(item.subitems, depth + 1, key)
        }
      })
    }
    walk(reader.toc, 0, 'root')
    return result
  })

  function toggleToc(key: string): void {
    const next = new Set(expandedToc)
    if (next.has(key)) next.delete(key)
    else next.add(key)
    expandedToc = next
  }

  $effect(() => {
    const hash = reader.activeBook?.bookHash ?? ''
    if (hash !== lastBookHash) {
      lastBookHash = hash
      expandedToc = new Set()
    }
  })

  const visibleAnnotations = $derived.by(() => {
    const needle = annotationFilter.trim().toLowerCase()
    return reader.annotations.filter((annotation) => {
      if (reader.sidebarTab === 'bookmarks' && annotation.kind !== 'bookmark') {
        return false
      }
      if (reader.sidebarTab === 'notes' && annotation.kind !== 'highlight') {
        return false
      }
      if (!needle) return true
      return `${annotation.quoteText} ${annotation.noteText}`
        .toLowerCase()
        .includes(needle)
    })
  })

  const searchResults = $derived.by(() => {
    const needle = query.trim().toLowerCase()
    if (!needle) return []
    const toc = flatToc
      .filter((item) => item.label.toLowerCase().includes(needle))
      .slice(0, 40)
      .map((item) => ({
        key: `toc:${item.href}`,
        label: item.label,
        detail: 'Contents',
        target: item.href,
      }))
    const notes = reader.annotations
      .filter((annotation) =>
        `${annotation.quoteText} ${annotation.noteText}`
          .toLowerCase()
          .includes(needle),
      )
      .slice(0, 40)
      .map((annotation) => ({
        key: `note:${annotation.id}`,
        label: annotation.quoteText || annotation.noteText,
        detail: annotation.kind,
        target: annotation.cfi,
      }))
    return [...toc, ...notes]
  })

  function go(target: string): void {
    reader.requestNavigation(target)
    if (window.matchMedia('(max-width: 767px)').matches) {
      reader.sidebarOpen = false
    }
  }

  function setTab(tab: 'toc' | 'notes' | 'bookmarks'): void {
    reader.setSidebarTab(tab)
    annotationFilter = ''
  }

  function toggleMenu(): void {
    menuOpen = !menuOpen
  }

  function run(action: () => void): void {
    menuOpen = false
    action()
  }

  function togglePin(): void {
    reader.sidebarPinned = !reader.sidebarPinned
  }

  function dateLabel(ms: number): string {
    return new Date(ms).toLocaleDateString(undefined, {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    })
  }
</script>

<aside class="sidebar" aria-label="Book navigation">
  <header class="sidebar-headerbar">
    <button
      class="icon-button"
      aria-label="Library"
      title="Library"
      onclick={() => reader.close()}
    >
      <Icon name="arrow-left" size={18} />
    </button>

    <div class="sidebar-spacer"></div>

    <button
      class="icon-button"
      class:active={reader.sidebarTab === 'search'}
      aria-label="Find in book"
      title="Find in book"
      onclick={() => reader.setSidebarTab('search')}
    >
      <Icon name="search" size={18} />
    </button>
    <button
      class="icon-button"
      class:active={menuOpen}
      aria-label="Book menu"
      title="Book menu"
      aria-expanded={menuOpen}
      onclick={toggleMenu}
    >
      <Icon name="more" size={18} />
    </button>
    <button
      class="icon-button"
      class:active={reader.sidebarPinned}
      aria-label="Pin sidebar"
      title="Pin sidebar"
      onclick={togglePin}
    >
      <Icon name="pin" size={17} />
    </button>

    {#if menuOpen}
      <div class="sidebar-menu">
        <button
          onclick={() =>
            run(() => {
              reader.typographyOpen = true
              reader.sidebarOpen = false
            })}
        >
          <Icon name="type" size={15} />
          Appearance
        </button>
        <button onclick={() => run(() => reader.cycleTheme())}>
          <Icon name={reader.resolvedTheme === 'dark' ? 'sun' : 'moon'} size={15} />
          Change theme
        </button>
        <button
          onclick={() =>
            run(() =>
              window.dispatchEvent(new CustomEvent('wolidate-toggle-tts')),
            )}
        >
          <Icon name="headphones" size={15} />
          Read aloud
        </button>
        <button onclick={() => run(() => (reader.detailsOpen = true))}>
          <Icon name="info" size={15} />
          About this book
        </button>
        <button onclick={() => run(() => reader.close())}>
          <Icon name="arrow-left" size={15} />
          Library
        </button>
      </div>
    {/if}
  </header>

  {#if book}
    <div class="sidebar-book">
      <div class="sidebar-cover">
        {#if coverFailed}
          <span>{initials}</span>
        {:else}
          <img
            src={coverUrl}
            alt=""
            onerror={() => (coverFailed = true)}
          />
        {/if}
      </div>
      <div class="sidebar-book-text">
        <strong>{book.title}</strong>
        <span>{book.author || 'Unknown author'}</span>
      </div>
      <button
        class="icon-button info-button"
        aria-label="About this book"
        title="About this book"
        onclick={() => (reader.detailsOpen = true)}
      >
        <Icon name="info" size={17} />
      </button>
    </div>
  {/if}

  <div class="sidebar-scroll">
    {#if reader.sidebarTab === 'toc'}
      {#if flatToc.length === 0}
        <div class="sidebar-empty-state">
          <Icon name="list" size={24} />
          <strong>No Contents</strong>
          <span>This book does not provide a table of contents.</span>
        </div>
      {:else}
        {#each visibleToc as item (item.key)}
          <div class={`toc-row depth-${Math.min(item.depth, 2)}`}>
            {#if item.hasChildren}
              <button
                class="toc-toggle"
                aria-label={expandedToc.has(item.key)
                  ? `Collapse ${item.label}`
                  : `Expand ${item.label}`}
                aria-expanded={expandedToc.has(item.key)}
                onclick={() => toggleToc(item.key)}
              >
                <Icon
                  name={expandedToc.has(item.key) ? 'chevron-down' : 'chevron-right'}
                  size={13}
                />
              </button>
            {:else}
              <span class="toc-toggle-spacer"></span>
            {/if}
            <button class="toc-item" onclick={() => go(item.href)}>
              {item.label}
            </button>
          </div>
        {/each}
      {/if}
    {:else if reader.sidebarTab === 'notes' || reader.sidebarTab === 'bookmarks'}
      <div class="annotation-tools">
        <span class="annotation-count">
          {visibleAnnotations.length}
          {visibleAnnotations.length === 1 ? 'item' : 'items'}
        </span>
      </div>

      {#if visibleAnnotations.length === 0}
        <div class="sidebar-empty-state">
          <Icon
            name={reader.sidebarTab === 'bookmarks' ? 'bookmark' : 'type'}
            size={24}
          />
          <strong>
            {reader.sidebarTab === 'bookmarks' ? 'No Bookmarks' : 'No Annotations'}
          </strong>
          <span>
            {reader.sidebarTab === 'bookmarks'
              ? 'Add some bookmarks to see them here.'
              : 'Select some text to add annotations.'}
          </span>
        </div>
      {:else}
        {#each visibleAnnotations as annotation (annotation.id)}
          <article class="annotation-card">
            <button class="annotation-quote" onclick={() => go(annotation.cfi)}>
              <Icon
                name={annotation.kind === 'bookmark' ? 'bookmark' : 'type'}
                size={13}
              />
              <span>
                {annotation.quoteText ||
                  annotation.noteText ||
                  annotation.cfi}
              </span>
            </button>
            {#if annotation.noteText && annotation.quoteText}
              <p class="annotation-note">{annotation.noteText}</p>
            {/if}
            <footer class="annotation-meta">
              <span>{dateLabel(annotation.createdAtMs)}</span>
              <button
                aria-label="Delete annotation"
                title="Delete annotation"
                onclick={() => void reader.removeAnnotation(annotation.id)}
              >
                <Icon name="trash" size={14} />
              </button>
            </footer>
          </article>
        {/each}
      {/if}

      <div class="annotation-filter">
        <Icon name="filter" size={14} />
        <input
          type="search"
          placeholder="Filter annotations…"
          bind:value={annotationFilter}
          aria-label="Filter annotations"
        />
      </div>
    {:else}
      <div class="sidebar-search">
        <Icon name="search" size={15} />
        <input
          type="search"
          placeholder="Find in book…"
          bind:value={query}
          aria-label="Find in book"
        />
      </div>
      {#if !query.trim()}
        <div class="sidebar-empty-state">
          <Icon name="search" size={24} />
          <strong>Search this book</strong>
          <span>Find chapter titles and saved annotations.</span>
        </div>
      {:else if searchResults.length === 0}
        <div class="sidebar-empty-state">
          <Icon name="search" size={24} />
          <strong>No Results Found</strong>
          <span>Try a different search.</span>
        </div>
      {:else}
        {#each searchResults as result (result.key)}
          <button class="search-result" onclick={() => go(result.target)}>
            <span class="search-result-label">{result.label}</span>
            <span class="search-result-detail">{result.detail}</span>
          </button>
        {/each}
      {/if}
    {/if}
  </div>

  <nav class="sidebar-tabs" aria-label="Sidebar sections">
    <button
      class:active={reader.sidebarTab === 'toc'}
      onclick={() => setTab('toc')}
    >
      <Icon name="list" size={18} />
      <span>Contents</span>
    </button>
    <button
      class:active={reader.sidebarTab === 'notes'}
      onclick={() => setTab('notes')}
    >
      <Icon name="type" size={17} />
      <span>Annotations</span>
    </button>
    <button
      class:active={reader.sidebarTab === 'bookmarks'}
      onclick={() => setTab('bookmarks')}
    >
      <Icon name="bookmark" size={17} />
      <span>Bookmarks</span>
    </button>
  </nav>
</aside>
