#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { copyFileSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const CANVAS = 1024
const INSET = 100
const TRAY_SIZE = 36
const TRAY_INSET = 1
const MARK_BOX = { x: 128, y: 166.4, w: 729.6, h: 691.2 }

const srcPath = fileURLToPath(new URL('../logo.svg', import.meta.url))
const iconsDir = fileURLToPath(new URL('../src-tauri/icons', import.meta.url))
const src = readFileSync(srcPath, 'utf8')

const openEnd = src.indexOf('>', src.indexOf('<svg'))
const close = src.lastIndexOf('</svg>')
if (openEnd < 0 || close < 0) throw new Error(`${srcPath}: 找不到 <svg>…</svg>`)

const inner = src.slice(openEnd + 1, close)

const svg = (w, h, body) =>
  `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">\n${body}\n</svg>\n`

const tmp = mkdtempSync(join(tmpdir(), 'audioscript-icons-'))

const appIcon = join(tmp, 'app-icon.svg')
writeFileSync(
  appIcon,
  svg(
    CANVAS,
    CANVAS,
    `  <g transform="translate(${INSET},${INSET}) scale(${(CANVAS - 2 * INSET) / CANVAS})">${inner}\n  </g>`,
  ),
)
execFileSync('pnpm', ['tauri', 'icon', appIcon], { stdio: 'inherit' })

const noPlate = inner.replace(/<rect\b[^>]*\/>/, '')
if (noPlate === inner) throw new Error(`${srcPath}: 没找到底板 <rect/>，logo.svg 的结构变了？`)
const blackMark = noPlate.replace('fill="url(#wave)"', 'fill="#000"')
if (blackMark === noPlate) throw new Error(`${srcPath}: 没找到 fill="url(#wave)"，logo.svg 的结构变了？`)

const scale = (TRAY_SIZE - 2 * TRAY_INSET) / Math.max(MARK_BOX.w, MARK_BOX.h)
const fit = [
  `translate(${TRAY_SIZE / 2},${TRAY_SIZE / 2})`,
  `scale(${scale.toFixed(6)})`,
  `translate(${-MARK_BOX.x - MARK_BOX.w / 2},${-MARK_BOX.y - MARK_BOX.h / 2})`,
].join(' ')

const traySvg = join(tmp, 'tray.svg')
writeFileSync(traySvg, svg(TRAY_SIZE, TRAY_SIZE, `  <g transform="${fit}">${blackMark}\n  </g>`))
const trayOut = join(tmp, 'tray')
execFileSync('pnpm', ['tauri', 'icon', '-p', String(TRAY_SIZE), '-o', trayOut, traySvg], {
  stdio: 'inherit',
})
copyFileSync(join(trayOut, `${TRAY_SIZE}x${TRAY_SIZE}.png`), join(iconsDir, 'tray-template.png'))
console.log(`         Copied ${TRAY_SIZE}x${TRAY_SIZE}.png -> src-tauri/icons/tray-template.png`)
