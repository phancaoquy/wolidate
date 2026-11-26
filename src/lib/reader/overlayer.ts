export interface OverlayRect {
  x: number
  y: number
  width: number
  height: number
}

/**
 * Draws annotation rectangles in viewport coordinates. foliate-js also emits
 * `create-overlayer` for its own renderer; this class is kept separate so
 * highlights can be drawn above a PDF canvas or a fixed-layout page without
 * depending on that renderer.
 */
export class Overlayer {
  private readonly svg: SVGSVGElement
  private readonly group: SVGGElement

  constructor(host: HTMLElement) {
    const namespace = 'http://www.w3.org/2000/svg'
    this.svg = document.createElementNS(namespace, 'svg')
    this.group = document.createElementNS(namespace, 'g')
    this.svg.setAttribute('aria-hidden', 'true')
    this.svg.style.cssText =
      'position:absolute;inset:0;width:100%;height:100%;pointer-events:none;z-index:2'
    this.svg.append(this.group)
    host.append(this.svg)
  }

  set(rects: OverlayRect[], color = 'rgba(255, 214, 64, 0.45)'): void {
    this.group.replaceChildren()
    const namespace = 'http://www.w3.org/2000/svg'
    for (const rect of rects) {
      const element = document.createElementNS(namespace, 'rect')
      element.setAttribute('x', String(rect.x))
      element.setAttribute('y', String(rect.y))
      element.setAttribute('width', String(rect.width))
      element.setAttribute('height', String(rect.height))
      element.setAttribute('rx', '2')
      element.setAttribute('fill', color)
      this.group.append(element)
    }
  }

  clear(): void {
    this.group.replaceChildren()
  }

  destroy(): void {
    this.svg.remove()
  }
}
