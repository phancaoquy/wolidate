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

  export interface FoliateSection {
    id: string
    href: string
    cfi?: string
    size?: number
    pageSpread?: string
    load?: () => Promise<
      | string
      | {
          src: string
          onZoom?: (options: { doc: Document; scale: number }) => void
        }
    >
  }

  export interface FoliateBook {
    metadata?: BookMetadata
    toc?: TocItem[] | null
    sections?: FoliateSection[]
    rendition?: { layout?: string; spread?: string }
    dir?: string
    resolveHref?: (href: string) => Promise<{ index: number } | undefined>
  }

  export interface OverlayerLike {
    add(key: string, range: Range | (() => Range), draw: unknown, options?: unknown): void
    remove(key: string): void
  }

  export class View extends HTMLElement {
    open(book: FoliateBook): Promise<void>
    close(): void
    goTo(target: string | number): Promise<void>
    goToFraction(fraction: number): Promise<void>
    next(distance?: number): Promise<void>
    prev(distance?: number): Promise<void>
    getCFI(index: number, range?: Range | null): string
    resolveNavigation(
      target: string | number | { fraction: number },
    ):
      | { index: number; anchor?: unknown }
      | Promise<{ index: number; anchor?: unknown }>
    book?: FoliateBook
    isFixedLayout?: boolean
    history?: {
      back(): void
      forward(): void
    }
    resolveCFI(
      cfi: string,
    ): { index: number; anchor: (doc: Document) => Range } | Promise<{ index: number; anchor: (doc: Document) => Range }>
    renderer?: HTMLElement & {
      getContents?: () => Array<{
        doc: Document
        index: number
        overlayer?: OverlayerLike
      }>
    }
  }

  export function makeBook(file: Blob | File): Promise<FoliateBook>
}

declare module '*/vendor/foliate-js/overlayer.js' {
  export interface OverlayRect {
    left: number
    top: number
    width: number
    height: number
  }

  export class Overlayer {
    static highlight(
      rects: OverlayRect[],
      options?: { color?: string },
    ): SVGElement
  }
}
