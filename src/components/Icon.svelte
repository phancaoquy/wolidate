<script lang="ts">
  const paths = {
    'arrow-left': ['M19 12H5', 'm12 19-7-7 7-7'],
    'chevron-left': ['m15 18-6-6 6-6'],
    'chevron-right': ['m9 18 6-6-6-6'],
    list: [
      'M8 6h13',
      'M8 12h13',
      'M8 18h13',
      'M3 6h.01',
      'M3 12h.01',
      'M3 18h.01',
    ],
    bookmark: ['m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z'],
    search: ['m21 21-4.3-4.3', 'M19 11a8 8 0 1 1-16 0 8 8 0 0 1 16 0z'],
    speaker: [
      'M11 5 6 9H2v6h4l5 4z',
      'M15.5 8.5a5 5 0 0 1 0 7',
      'M18.5 5.5a9 9 0 0 1 0 13',
    ],
    type: ['M4 7V4h16v3', 'M9 20h6', 'M12 4v16'],
    sun: [
      'M12 12m-4 0a4 4 0 1 0 8 0a4 4 0 1 0-8 0',
      'M12 2v2',
      'M12 20v2',
      'm4.93 4.93 1.42 1.42',
      'm17.66 17.66 1.42 1.42',
      'M2 12h2',
      'M20 12h2',
      'm6.34 17.66-1.41 1.41',
      'm19.07 4.93-1.41 1.41',
    ],
    moon: ['M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z'],
    close: ['M18 6 6 18', 'm6 6 12 12'],
    plus: ['M12 5v14', 'M5 12h14'],
    trash: [
      'M3 6h18',
      'M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6',
      'M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2',
      'M10 11v6',
      'M14 11v6',
    ],
    sync: ['M21 12a9 9 0 1 1-3-6.7L21 8', 'M21 3v5h-5'],
    more: ['M5 12h.01', 'M12 12h.01', 'M19 12h.01'],
    book: [
      'M4 19.5A2.5 2.5 0 0 1 6.5 17H20',
      'M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z',
    ],
    collection: [
      'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z',
    ],
    grid: [
      'M3 3h7v7H3z',
      'M14 3h7v7h-7z',
      'M3 14h7v7H3z',
      'M14 14h7v7h-7z',
    ],
    maximize: [
      'M8 3H5a2 2 0 0 0-2 2v3',
      'M16 3h3a2 2 0 0 1 2 2v3',
      'M8 21H5a2 2 0 0 1-2-2v-3',
      'M16 21h3a2 2 0 0 0 2-2v-3',
    ],
    layout: ['M3 4h18v16H3z', 'M9 4v16', 'M15 4v16'],
    scroll: ['M6 4h12', 'M6 9h12', 'M6 14h12', 'M6 19h12'],
    'column-one': ['M6 4h12v16H6z'],
    'column-two': ['M4 4h7v16H4z', 'M13 4h7v16h-7z'],
    check: ['m5 12 5 5L20 7'],
    pin: [
      'M12 17v5',
      'M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z',
    ],
    info: ['M12 12m-9 0a9 9 0 1 0 18 0a9 9 0 1 0-18 0', 'M12 16v-4', 'M12 8h.01'],
    undo: ['M3 7v6h6', 'M21 17a9 9 0 0 0-15-6.7L3 13'],
    redo: ['M21 7v6h-6', 'M3 17a9 9 0 0 1 15-6.7L21 13'],
    headphones: [
      'M3 14h3a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a9 9 0 0 1 18 0v7a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3',
    ],
    sidebar: ['M3 3h18v18H3z', 'M9 3v18'],
    filter: ['M22 3H2l8 9.46V19l4 2v-8.54z'],
    'chevron-down': ['m6 9 6 6 6-6'],
    pencil: [
      'M17 3a2.85 2.85 0 0 1 4 4L7.5 20.5 2 22l1.5-5.5z',
      'm15 5 4 4',
    ],
    monitor: ['M3 4h18v12H3z', 'M8 20h8', 'M12 16v4'],
    globe: [
      'M12 12m-9 0a9 9 0 1 0 18 0a9 9 0 1 0-18 0',
      'M3 12h18',
      'M12 3a15 15 0 0 1 0 18',
      'M12 3a15 15 0 0 0 0 18',
    ],
  } as const

  let {
    name,
    size = 18,
    strokeWidth = 1.8,
    label,
  }: {
    name: keyof typeof paths
    size?: number
    strokeWidth?: number
    label?: string
  } = $props()
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden={label ? undefined : 'true'}
  role={label ? 'img' : undefined}
>
  {#if label}<title>{label}</title>{/if}
  {#each paths[name] as d}
    <path {d} />
  {/each}
</svg>
