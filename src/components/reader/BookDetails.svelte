<script lang="ts">
  import Icon from '../Icon.svelte'
  import { reader } from '../../lib/stores/reader.svelte'

  const book = $derived(reader.activeBook)
  let coverFailed = $state(false)
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
  const highlightCount = $derived(
    reader.annotations.filter((annotation) => annotation.kind === 'highlight').length,
  )
  const bookmarkCount = $derived(
    reader.annotations.filter((annotation) => annotation.kind === 'bookmark').length,
  )

  function formatSize(bytes: number): string {
    if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }

  function formatDate(ms: number): string {
    return new Date(ms).toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    })
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') reader.detailsOpen = false
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if book}
  <div class="modal-backdrop">
    <div class="book-details" role="dialog" aria-modal="true" aria-label="Book details">
      <button
        class="details-close icon-button"
        aria-label="Close book details"
        onclick={() => (reader.detailsOpen = false)}
      >
        <Icon name="close" size={17} />
      </button>

      <div class="details-cover">
        {#if coverFailed}
          <div class="details-cover-fallback">{initials}</div>
        {:else}
          <img
            src={coverUrl}
            alt=""
            onerror={() => (coverFailed = true)}
          />
        {/if}
      </div>

      <div class="details-body">
        <h1>{book.title}</h1>
        <p class="details-author">{book.author || 'Unknown author'}</p>

        <div class="details-grid">
          <div><span>Format</span><strong>{book.format.toUpperCase()}</strong></div>
          <div><span>File size</span><strong>{formatSize(book.fileSize)}</strong></div>
          <div><span>Added</span><strong>{formatDate(book.addedAtMs)}</strong></div>
          <div><span>Language</span><strong>{book.language || 'Unknown'}</strong></div>
          <div>
            <span>Progress</span>
            <strong>{Math.round(book.progressFraction * 100)}%</strong>
          </div>
          <div>
            <span>Highlights · Bookmarks</span>
            <strong>{highlightCount} · {bookmarkCount}</strong>
          </div>
        </div>

        <p class="details-note">
          This file stays on your device. Reading progress and annotations sync
          only when Google Drive is connected.
        </p>
      </div>
    </div>
  </div>
{/if}
