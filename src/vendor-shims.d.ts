// Minimal surface of the pinned foliate-js build. The vendored sources are
// plain ESM; these declarations keep TypeScript strict without checking them.

declare module '*/vendor/foliate-js/view.js' {
  export interface TocItem {
    label: string
    href: string
    subitems?: TocItem[] | null
  }

  export interface BookMetadata {
    title?: string
    author?: string | Array<string | { name?: string }>
    language?: string
  }

  export interface FoliateBook {
    metadata?: BookMetadata
    toc?: TocItem[] | null
    sections?: Array<{ id: string; href: string; cfi?: string }>
    rendition?: { layout?: string }
  }

  export class View extends HTMLElement {
    open(book: FoliateBook): Promise<void>
    close(): void
    goTo(target: string | number): Promise<void>
    next(distance?: number): Promise<void>
    prev(distance?: number): Promise<void>
    getCFI(index: number, range: Range): string
    renderer?: {
      getContents?: () => Array<{ doc: Document; index: number }>
    }
  }

  export function makeBook(file: Blob | File): Promise<FoliateBook>
}
