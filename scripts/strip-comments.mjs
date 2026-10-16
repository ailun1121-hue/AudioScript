#!/usr/bin/env node
// 注释清理：白名单保留式——只留功能性指令与 TODO/SAFETY 标记，其余注释连行删除。
// 默认 dry-run；`--write` 落盘前整棵备份到 .cache/，仓库没有 git 提交时那是唯一后悔药。
import { cpSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs'
import { dirname, join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = fileURLToPath(new URL('..', import.meta.url))
const SCOPE = ['src', 'scripts', 'src-tauri/src', 'index.html', 'overlay.html']
const EXTS = new Set(['.ts', '.tsx', '.mjs', '.js', '.css', '.html', '.rs'])
const SKIP = new Set(['src/shared/types.ts', 'scripts/strip-comments.mjs']) // typeshare 生成物 / 本脚本自身
const SKIP_DIRS = new Set(['node_modules', 'dist', 'target', '.cache', '.git', 'gen'])

// 命中即保留：这些删了会坏功能（指令）或丢掉线索（标记）
const KEEP = [
  /(^|\s)(biome|eslint)[-:]/,
  /@ts-(ignore|expect-error|nocheck|check)\b/,
  /<reference\s/,
  /\bSAFETY:/,
  /\b(TODO|FIXME|HACK|XXX)\b/,
  /@__PURE__|@__NO_SIDE_EFFECTS__|@preserve|@license|@vite-ignore/,
]

const read = (p) => readFileSync(p, 'utf8')
const lineOf = (src, idx) => src.slice(0, idx).split('\n').length

// 正则字面量判定：前一个非空白字符能当「值」结尾时，/ 是除号而不是正则开头
const REGEX_OK = new Set(['(', ',', '=', ':', '[', '!', '&', '|', '?', '{', ';', '+', '-', '*', '%', '~', '^', '<', '>'])

// ---------- C 家族（ts/tsx/js/mjs/css）：// 与 /* */，字符串/模板/正则/JSX 里的不算 ----------
function stripC(src, { jsx = false, css = false, lineOffset = 0 } = {}) {
  const out = []
  const kept = []
  const hits = []
  let removed = 0
  const n = src.length

  const popBlank = () => {
    while (out.length && (out[out.length - 1] === ' ' || out[out.length - 1] === '\t')) out.pop()
  }
  const blankBefore = (from) => {
    for (let k = from - 1; k >= 0; k--) {
      const c = src[k]
      if (c === '\n') return true
      if (c !== ' ' && c !== '\t' && c !== '\r') return false
    }
    return true
  }
  const blankAfter = (to) => {
    for (let k = to; k < n; k++) {
      const c = src[k]
      if (c === '\n') return true
      if (c !== ' ' && c !== '\t' && c !== '\r') return false
    }
    return true
  }
  // 返回删除后的下一个位置；保留则原样留下
  const handle = (from, to, body, block) => {
    if (KEEP.some((re) => re.test(body)) || (block && body.trimStart().startsWith('!'))) {
      kept.push({ line: lineOf(src, from) + lineOffset, text: src.slice(from, to).replace(/\s+$/, '') })
      out.push(src.slice(from, to))
      return to
    }
    removed++
    hits.push({ line: lineOf(src, from) + lineOffset, text: src.slice(from, to).replace(/\s+$/, '') })
    if (blankBefore(from) && blankAfter(to)) {
      popBlank()
      let j = to
      while (j < n && src[j] !== '\n') j++
      return Math.min(j + 1, n)
    }
    if (!blankBefore(from)) popBlank() // 行尾注释：留下代码，去掉多出来的空白
    return to
  }
  const scanString = (start) => {
    const q = src[start]
    let k = start + 1
    while (k < n) {
      if (src[k] === '\\') {
        k += 2
        continue
      }
      if (src[k] === q) return k + 1
      if (src[k] === '\n') return k // 未闭合：停在行尾，别吞掉后面整片代码
      k++
    }
    return n
  }
  const lastChar = () => {
    for (let k = out.length - 1; k >= 0; k--) {
      const s = out[k]
      const c = s[s.length - 1]
      if (c === ' ' || c === '\t' || c === '\n' || c === '\r') continue
      return c
    }
    return undefined
  }
  const scanRegex = (start) => {
    let k = start + 1
    let inClass = false
    while (k < n) {
      const c = src[k]
      if (c === '\\') {
        k += 2
        continue
      }
      if (c === '\n') return -1 // 正则不跨行：说明这不是正则，当除号
      if (c === '[') inClass = true
      else if (c === ']') inClass = false
      else if (c === '/' && !inClass) {
        k++
        while (k < n && /[a-z]/i.test(src[k])) k++
        return k
      }
      k++
    }
    return -1
  }

  const code = (start, inExpr) => {
    let k = start
    let depth = 0
    while (k < n) {
      const c = src[k]
      // 必须排在 `{` 分支之前：JSX 注释 `{/* … */}` 独占一行就整块删；对象字面量 `{ /* … */ }` 不碰
      if (jsx && c === '{' && src[k + 1] === '/' && src[k + 2] === '*' && blankBefore(k)) {
        const close = src.indexOf('*/', k + 3)
        if (close >= 0) {
          let m = close + 2
          while (m < n && /\s/.test(src[m])) m++
          if (src[m] === '}' && blankAfter(m + 1)) {
            popBlank()
            let e = m + 1
            while (e < n && src[e] !== '\n') e++
            removed++
            hits.push({ line: lineOf(src, k) + lineOffset, text: src.slice(k, close + 2) })
            k = Math.min(e + 1, n)
            continue
          }
        }
      }
      if (c === '}') {
        if (inExpr && depth === 0) {
          out.push('}')
          return k + 1
        }
        if (inExpr) depth--
        out.push(c)
        k++
        continue
      }
      if (c === '{') {
        if (inExpr) depth++
        out.push(c)
        k++
        continue
      }
      if (c === "'" || c === '"') {
        const j = scanString(k)
        out.push(src.slice(k, j))
        k = j
        continue
      }
      if (c === '`') {
        out.push('`')
        k++
        while (k < n) {
          if (src[k] === '\\') {
            out.push(src.slice(k, k + 2))
            k += 2
            continue
          }
          if (src[k] === '`') {
            out.push('`')
            k++
            break
          }
          if (src[k] === '$' && src[k + 1] === '{') {
            out.push('${')
            k = code(k + 2, true) // 表达式内部照常处理字符串与注释
            continue
          }
          out.push(src[k])
          k++
        }
        continue
      }
      if (c === '/' && src[k + 1] === '/' && !css) {
        const from = k
        let j = k
        while (j < n && src[j] !== '\n') j++
        k = handle(from, j, src.slice(from + 2, j), false)
        continue
      }
      if (c === '/' && src[k + 1] === '*') {
        const from = k
        const close = src.indexOf('*/', k + 2)
        const to = close < 0 ? n : close + 2
        k = handle(from, to, src.slice(from + 2, to - 2), true)
        continue
      }
      // 正则字面量：上面两条排除了 `//` 与 `/*`，这里才轮到它
      if (c === '/' && !css) {
        const prev = lastChar()
        if (prev === undefined || REGEX_OK.has(prev)) {
          const j = scanRegex(k)
          if (j > 0) {
            out.push(src.slice(k, j))
            k = j
            continue
          }
        }
      }
      out.push(c)
      k++
    }
    return k
  }

  code(0, false)
  return { text: out.join(''), removed, kept, hits }
}

// ---------- HTML：<!-- --> 以及内联 <script>/<style> ----------
function stripHtml(src) {
  const kept = []
  const hits = []
  let removed = 0
  let out = ''
  let i = 0
  const n = src.length

  const soloComment = (from, to) => {
    const before = src.slice(src.lastIndexOf('\n', from - 1) + 1, from)
    const nl = src.indexOf('\n', to)
    const after = src.slice(to, nl < 0 ? n : nl)
    return before.trim() === '' && after.trim() === ''
  }

  while (i < n) {
    if (src.startsWith('<!--', i)) {
      const close = src.indexOf('-->', i + 4)
      const to = close < 0 ? n : close + 3
      const body = src.slice(i + 4, to - 3)
      if (KEEP.some((re) => re.test(body))) {
        kept.push({ line: lineOf(src, i), text: src.slice(i, to).replace(/\s+$/, '') })
      } else {
        removed++
        hits.push({ line: lineOf(src, i), text: src.slice(i, to).replace(/\s+$/, '') })
        if (soloComment(i, to)) {
          out = out.replace(/[ \t]+$/, '')
          let j = to
          while (j < n && src[j] !== '\n') j++
          i = Math.min(j + 1, n)
          continue
        }
      }
      out += src.slice(i, to)
      i = to
      continue
    }
    const m = /^<(script|style)\b[^>]*>/i.exec(src.slice(i))
    if (m) {
      const open = i + m[0].length
      const close = src.indexOf(`</${m[1]}`, open)
      const end = close < 0 ? n : close
      const inner = stripC(src.slice(open, end), { lineOffset: lineOf(src, open) - 1 })
      out += m[0] + inner.text
      removed += inner.removed
      kept.push(...inner.kept)
      hits.push(...inner.hits)
      i = end
      continue
    }
    out += src[i]
    i++
  }
  return { text: out, removed, kept, hits }
}

// ---------- Rust：// 与可嵌套的 /* */，raw string / char 字面量 / 生命周期都要让开 ----------
function stripRust(src) {
  const out = []
  const kept = []
  const hits = []
  let removed = 0
  const n = src.length

  const popBlank = () => {
    while (out.length && (out[out.length - 1] === ' ' || out[out.length - 1] === '\t')) out.pop()
  }
  const blankBefore = (from) => {
    for (let k = from - 1; k >= 0; k--) {
      const c = src[k]
      if (c === '\n') return true
      if (c !== ' ' && c !== '\t' && c !== '\r') return false
    }
    return true
  }
  const blankAfter = (to) => {
    for (let k = to; k < n; k++) {
      const c = src[k]
      if (c === '\n') return true
      if (c !== ' ' && c !== '\t' && c !== '\r') return false
    }
    return true
  }
  const handle = (from, to, body) => {
    if (KEEP.some((re) => re.test(body))) {
      kept.push({ line: lineOf(src, from), text: src.slice(from, to).replace(/\s+$/, '') })
      out.push(src.slice(from, to))
      return to
    }
    removed++
    hits.push({ line: lineOf(src, from), text: src.slice(from, to).replace(/\s+$/, '') })
    if (blankBefore(from) && blankAfter(to)) {
      popBlank()
      let j = to
      while (j < n && src[j] !== '\n') j++
      return Math.min(j + 1, n)
    }
    if (!blankBefore(from)) popBlank()
    return to
  }
  const blockEnd = (k) => {
    let depth = 1
    let j = k + 2
    while (j < n && depth > 0) {
      if (src[j] === '/' && src[j + 1] === '*') {
        depth++
        j += 2
      } else if (src[j] === '*' && src[j + 1] === '/') {
        depth--
        j += 2
      } else j++
    }
    return j
  }
  const rawEnd = (k) => {
    let h = 0
    while (src[k + h] === '#') h++
    const quote = k + h
    if (src[quote] !== '"') return -1 // r 后面不是原始字符串，当普通字符走
    const closing = `"${'#'.repeat(h)}`
    const close = src.indexOf(closing, quote + 1)
    return close < 0 ? n : close + closing.length
  }

  let k = 0
  while (k < n) {
    const c = src[k]
    if (c === '/' && src[k + 1] === '/') {
      const from = k
      let j = k
      while (j < n && src[j] !== '\n') j++
      k = handle(from, j, src.slice(from + 2, j))
      continue
    }
    if (c === '/' && src[k + 1] === '*') {
      const from = k
      const to = blockEnd(k)
      k = handle(from, to, src.slice(from + 2, Math.max(from + 2, to - 2)))
      continue
    }
    if (c === '"') {
      let j = k + 1
      while (j < n) {
        if (src[j] === '\\') {
          j += 2
          continue
        }
        if (src[j] === '"') {
          j++
          break
        }
        j++
      }
      out.push(src.slice(k, j))
      k = j
      continue
    }
    if ((c === 'r' || c === 'b') && (src[k + 1] === '"' || src[k + 1] === '#' || src[k + 1] === 'r')) {
      const raw = c === 'r' ? rawEnd(k + 1) : src[k + 1] === 'r' ? rawEnd(k + 2) : -1
      if (raw > 0) {
        out.push(src.slice(k, raw))
        k = raw
        continue
      }
    }
    if (c === "'") {
      const lit = /^'(?:\\.|[^\\'])'/.exec(src.slice(k))
      if (lit) {
        out.push(lit[0])
        k += lit[0].length
        continue
      }
    }
    out.push(c)
    k++
  }
  return { text: out.join(''), removed, kept, hits }
}

// ---------- 驱动 ----------
const collect = (p, acc) => {
  const st = statSync(p)
  if (st.isDirectory()) {
    for (const e of readdirSync(p, { withFileTypes: true })) {
      if (e.isDirectory()) {
        if (!SKIP_DIRS.has(e.name)) collect(join(p, e.name), acc)
      } else if (EXTS.has(e.name.slice(e.name.lastIndexOf('.')))) acc.push(join(p, e.name))
    }
  } else acc.push(p)
}

const args = process.argv.slice(2)
const write = args.includes('--write')
const verbose = args.includes('--verbose')
const extra = args.filter((a) => !a.startsWith('--'))

const targets = []
for (const p of extra.length ? extra : SCOPE.map((s) => join(ROOT, s))) {
  try {
    collect(p, targets)
  } catch {
    console.warn(`跳过不存在的路径：${p}`)
  }
}

const files = [...new Set(targets)]
  .map((p) => relative(ROOT, p))
  .filter((p) => !SKIP.has(p))
  .filter((p) => !p.split('/').some((seg) => SKIP_DIRS.has(seg)))
  .sort()

let totalRemoved = 0
const allKept = []
const perFile = []
const plan = []

for (const rel of files) {
  const path = join(ROOT, rel)
  const raw = read(path)
  const eol = raw.includes('\r\n') ? '\r\n' : '\n'
  const src = eol === '\n' ? raw : raw.replaceAll('\r\n', '\n')
  const ext = rel.slice(rel.lastIndexOf('.'))
  const res =
    ext === '.html'
      ? stripHtml(src)
      : ext === '.rs'
        ? stripRust(src)
        : ext === '.css'
          ? stripC(src, { css: true })
          : stripC(src, { jsx: ext === '.tsx' })
  if (!res) continue

  let text = res.text.replace(/^\n+/, '').replace(/\n[ \t]*\n(?:[ \t]*\n)+/g, '\n\n').replace(/\n+$/, '\n')
  if (eol !== '\n') text = text.replaceAll('\n', eol)

  allKept.push(...res.kept.map((c) => ({ rel, ...c })))
  if (text === src) continue
  if (text.trim() === '') {
    console.warn(`⚠ ${rel} 全部内容都是注释，已跳过（人工确认后再删整个文件）`)
    continue
  }

  totalRemoved += res.removed
  perFile.push({ rel, removed: res.removed, kept: res.kept.length })
  if (verbose) for (const h of res.hits) console.log(`-  ${rel}:${h.line}  ${h.text}`)
  plan.push({ rel, path, text })
}

// 先备份再写：写坏了还有原件可回滚
const backup = join(ROOT, '.cache', `comment-backup-${new Date().toISOString().replace(/[:.]/g, '-')}`)
if (write) {
  for (const p of plan) {
    const dest = join(backup, p.rel.replace(/^(\.\.\/)+/, '_/')) // 仓外路径别让 .. 逃出备份目录
    mkdirSync(dirname(dest), { recursive: true })
    cpSync(p.path, dest)
  }
  for (const p of plan) writeFileSync(p.path, p.text)
}

perFile.sort((a, b) => b.removed - a.removed)
console.log(`\n${write ? '已写入' : 'dry-run'}：${plan.length} 个文件，删除注释 ${totalRemoved} 处，保留 ${allKept.length} 处`)
if (allKept.length) {
  console.log('\n保留清单（白名单命中）：')
  for (const k of allKept) console.log(`  ${`${k.rel}:${k.line}`.padEnd(46)} ${k.text.replace(/\s*\n\s*/g, ' ').slice(0, 80)}`)
}
console.log('\n按文件（删除 / 保留）：')
for (const f of perFile) console.log(`  ${f.rel.padEnd(46)} ${String(f.removed).padStart(3)} / ${f.kept}`)

if (write) console.log(`\n备份（改动前）：${relative(ROOT, backup)}`)
else console.log('\n加 --write 落盘（会先备份到 .cache/）；加 --verbose 逐条列出被删的注释。')

const skipped = []
for (const p of SCOPE.map((s) => join(ROOT, s))) {
  try {
    if (statSync(p).isDirectory()) {
      for (const e of readdirSync(p)) {
        const ext = e.slice(e.lastIndexOf('.'))
        if (e.startsWith('.') || EXTS.has(ext)) continue
        if (statSync(join(p, e)).isFile()) skipped.push(relative(ROOT, join(p, e)))
      }
    }
  } catch {}
}
if (skipped.length) console.log(`未处理（语言不在支持列表）：${skipped.join('、')}`)
