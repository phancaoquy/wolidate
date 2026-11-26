import type { FoliateBook, View } from '../../vendor/foliate-js/view.js'
import type { Book } from '../ipc/commands'

export const BOOK_ORIGIN = 'http://book.localhost'

const EXTENSIONS: Record<string, string> = {
  epub: 'epub',
  mobi: 'mobi',
  azw3: 'azw3',
  fb2: 'fb2',
  cbz: 'cbz',
  pdf: 'pdf',
}

const MIME_TYPES: Record<string, string> = {
  epub: 'application/epub+zip',
  mobi: 'application/x-mobipocket-ebook',
  azw3: 'application/x-mobipocket-ebook',
  fb2: 'application/x-fictionbook+xml',
  cbz: 'application/vnd.comicbook+zip',
  pdf: 'application/pdf',
}

/**
 * Blob-like object backed by HTTP range requests. foliate-js and its vendored
 * zip.js/pdf.js only call `size`, `slice()` and `arrayBuffer()`, so no book
 * bytes are downloaded until a renderer asks for them.
 */
export class HttpBookBlob {
  readonly size: number
  readonly name: string
  readonly type: string

  constructor(url: string, size: number, name: string, type: string) {
    this.url = url
    this.size = size
    this.name = name
    this.type = type
  }

  private readonly url: string

  slice(start = 0, end = this.size, contentType = ''): HttpBookSlice {
    return new HttpBookSlice(this.url, start, end, contentType || this.type)
  }

  /**
   * foliate-js only calls this for non-ZIP formats (FB2). Fetch in bounded
   * chunks instead of asking the protocol for a whole-file 200 response.
   */
  async arrayBuffer(): Promise<ArrayBuffer> {
    const chunkSize = 4 * 1024 * 1024
    const total = this.size
    const output = new Uint8Array(total)
    let offset = 0
    while (offset < total) {
      const end = Math.min(total, offset + chunkSize)
      const chunk = new Uint8Array(await this.slice(offset, end).arrayBuffer())
      output.set(chunk, offset)
      offset = end
    }
    return output.buffer
  }

  async text(): Promise<string> {
    return new TextDecoder().decode(await this.arrayBuffer())
  }
}

export class HttpBookSlice {
  constructor(
    private readonly url: string,
    readonly start: number,
    readonly end: number,
    readonly type: string,
  ) {}

  get size(): number {
    return Math.max(0, this.end - this.start)
  }

  slice(start = 0, end = this.size): HttpBookSlice {
    const absoluteStart = this.start + start
    const absoluteEnd = Math.min(this.end, this.start + end)
    return new HttpBookSlice(this.url, absoluteStart, absoluteEnd, this.type)
  }

  async arrayBuffer(): Promise<ArrayBuffer> {
    if (this.end <= this.start) return new ArrayBuffer(0)
    const response = await fetch(this.url, {
      cache: 'no-store',
      headers: { Range: `bytes=${this.start}-${this.end - 1}` },
    })
    if (response.status === 416) {
      throw new Error(`Range ${this.start}-${this.end - 1} is outside the book`)
    }
    if (!response.ok && response.status !== 206) {
      throw new Error(`${response.status} ${response.statusText} while reading the book`)
    }
    const buffer = await response.arrayBuffer()
    if (response.status === 200 && buffer.byteLength > this.size) {
      return buffer.slice(this.start, this.end)
    }
    return buffer
  }
}

export function bookUrl(bookHash: string): string {
  return `${BOOK_ORIGIN}/${bookHash}`
}

export function bookFileName(book: Book): string {
  const extension = EXTENSIONS[book.format] ?? 'bin'
  const safeTitle = book.title.replace(/[\\/:*?"<>|]+/g, ' ').trim()
  return `${safeTitle || book.bookHash}.${extension}`
}

export function bookMimeType(format: string): string {
  return MIME_TYPES[format] ?? 'application/octet-stream'
}

let viewModule: Promise<typeof import('../../vendor/foliate-js/view.js')> | undefined

export function loadViewModule(): Promise<typeof import('../../vendor/foliate-js/view.js')> {
  viewModule ??= import('../../vendor/foliate-js/view.js')
  return viewModule
}

export async function createFoliateView(): Promise<View> {
  await loadViewModule()
  return document.createElement('foliate-view') as unknown as View
}

/**
 * Build a foliate-js Book from range-backed bytes.
 *
 * `makeBook` would normally fetch a URL into a whole `File`, which violates
 * the zero-full-file-buffering rule. Passing a Blob-like object instead keeps
 * every read range-sized.
 */
export async function loadBook(book: Book): Promise<FoliateBook> {
  const { makeBook } = await loadViewModule()
  const blob = new HttpBookBlob(
    bookUrl(book.bookHash),
    book.fileSize,
    bookFileName(book),
    bookMimeType(book.format),
  )
  return makeBook(blob as unknown as Blob)
}
