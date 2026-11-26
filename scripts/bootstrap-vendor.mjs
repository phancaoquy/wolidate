// Pins foliate-js to a known commit and copies only runtime files into
// src/vendor/. The directory is gitignored; run this after npm install or
// whenever the pin changes.
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

const PIN = '78914aef4466eb960965702401634c2cb348e9b1'
const ROOT = path.resolve(import.meta.dirname, '..')
const TARGET = path.join(ROOT, 'src', 'vendor', 'foliate-js')
const PUBLIC_PDFJS = path.join(ROOT, 'public', 'vendor', 'pdfjs')
const TMP = fs.mkdtempSync(path.join(os.tmpdir(), 'foliate-js-'))

function fail(message) {
  console.error(message)
  process.exit(1)
}

function alreadyPinned() {
  const versionFile = path.join(TARGET, 'VERSION')
  return (
    fs.existsSync(versionFile) &&
    fs.readFileSync(versionFile, 'utf8').trim() === PIN &&
    fs.existsSync(path.join(PUBLIC_PDFJS, 'cmaps'))
  )
}

async function main() {
  if (alreadyPinned()) {
    stripMapComments(TARGET)
    console.log(`foliate-js ${PIN} already vendored`)
    return
  }

  const url = `https://codeload.github.com/johnfactotum/foliate-js/tar.gz/${PIN}`
  console.log(`Downloading foliate-js ${PIN}`)
  const response = await fetch(url)
  if (!response.ok) fail(`Download failed: ${response.status} ${response.statusText}`)
  const archive = path.join(TMP, 'foliate.tar.gz')
  fs.writeFileSync(archive, Buffer.from(await response.arrayBuffer()))

  const tar = spawnSync('tar', ['-xzf', archive, '-C', TMP], { stdio: 'inherit' })
  if (tar.status !== 0) fail('Could not extract foliate-js tarball (tar is required)')

  const extracted = fs
    .readdirSync(TMP, { withFileTypes: true })
    .find((entry) => entry.isDirectory() && entry.name.startsWith('foliate-js-'))
  if (!extracted) fail('Unexpected tarball layout')
  const source = path.join(TMP, extracted.name)

  fs.rmSync(TARGET, { recursive: true, force: true })
  fs.mkdirSync(TARGET, { recursive: true })

  for (const entry of fs.readdirSync(source, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      if (entry.name === 'vendor') {
        fs.cpSync(path.join(source, entry.name), path.join(TARGET, entry.name), {
          recursive: true,
        })
      }
      continue
    }
    if (entry.name.endsWith('.js') || entry.name === 'LICENSE') {
      fs.copyFileSync(path.join(source, entry.name), path.join(TARGET, entry.name))
    }
  }

  // Source maps and the optional dictionary/OPDS modules are not needed by the
  // reader shell. Removing the maps keeps the vendor tree small.
  function stripMaps(directory) {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const full = path.join(directory, entry.name)
      if (entry.isDirectory()) stripMaps(full)
      else if (entry.name.endsWith('.map')) fs.rmSync(full)
    }
  }
  stripMaps(TARGET)
  stripMapComments(TARGET)

  // foliate-js fetches the PDF worker and text-layer CSS at runtime. Vite
  // rewrites those relative asset URLs, so the runtime fetch would look for
  // files that do not exist in dist. Replace the header with static Vite
  // assets: a copied worker URL and inlined CSS strings.
  const pdfPath = path.join(TARGET, 'pdf.js')
  const pdfSource = fs.readFileSync(pdfPath, 'utf8')
  const headerStart = pdfSource.indexOf('const pdfjsPath')
  const headerEndMarker =
    "const annotationLayerBuilderCSS = await fetchText(pdfjsPath('annotation_layer_builder.css'))"
  const headerEnd = pdfSource.indexOf(headerEndMarker)
  if (headerStart === -1 || headerEnd === -1) {
    fail('Unexpected foliate-js pdf.js layout; update the bootstrap patch')
  }
  const patchedHeader = `import './vendor/pdfjs/pdf.mjs'
import textLayerBuilderCSS from './vendor/pdfjs/text_layer_builder.css?raw'
import annotationLayerBuilderCSS from './vendor/pdfjs/annotation_layer_builder.css?raw'

const pdfjsPath = path => '/vendor/pdfjs/' + path

const pdfjsLib = globalThis.pdfjsLib
pdfjsLib.GlobalWorkerOptions.workerSrc =
  new URL('./vendor/pdfjs/pdf.worker.mjs', import.meta.url).toString()
`
  fs.writeFileSync(
    pdfPath,
    pdfSource.slice(0, headerStart) +
      patchedHeader +
      pdfSource.slice(headerEnd + headerEndMarker.length),
  )

  // PDF.js fetches CMaps and standard fonts by URL. Keep them in public/ so
  // the same absolute paths work in Vite dev and in the embedded Tauri build.
  fs.rmSync(PUBLIC_PDFJS, { recursive: true, force: true })
  fs.mkdirSync(PUBLIC_PDFJS, { recursive: true })
  for (const directory of ['cmaps', 'standard_fonts']) {
    fs.cpSync(
      path.join(source, 'vendor', 'pdfjs', directory),
      path.join(PUBLIC_PDFJS, directory),
      { recursive: true },
    )
  }

  fs.writeFileSync(path.join(TARGET, 'VERSION'), `${PIN}\n`)
  console.log(`foliate-js ready at ${path.relative(ROOT, TARGET)}`)
}

main()
  .catch((error) => fail(error.stack ?? String(error)))
  .finally(() => fs.rmSync(TMP, { recursive: true, force: true }))

function stripMapComments(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const full = path.join(directory, entry.name)
    if (entry.isDirectory()) {
      stripMapComments(full)
      continue
    }
    if (!/\.(m?js|css)$/.test(entry.name)) continue
    const source = fs.readFileSync(full, 'utf8')
    const cleaned = source
      .replace(/\/\/# sourceMappingURL=.*$/gm, '')
      .replace(/\/\*# sourceMappingURL=[\s\S]*?\*\//g, '')
    if (cleaned !== source) fs.writeFileSync(full, cleaned)
  }
}
