#!/usr/bin/env node
import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { basename } from 'node:path'

function fail(message) {
  process.stderr.write(`\n❌ ${message}\n`)
  process.exit(1)
}

const opts = { version: '', baseUrl: '', notes: '', out: 'latest.json', platforms: [] }
const argv = process.argv.slice(2)

for (let i = 0; i < argv.length; i += 2) {
  const flag = argv[i]
  const value = argv[i + 1]
  if (value === undefined) fail(`参数 ${flag} 缺少值`)

  switch (flag) {
    case '--version':
      opts.version = value.replace(/^v/, '')
      break
    case '--base-url':
      opts.baseUrl = value.replace(/\/+$/, '')
      break
    case '--notes':
      opts.notes = value
      break
    case '--out':
      opts.out = value
      break
    case '--platform': {
      const at = value.indexOf('=')
      if (at <= 0) fail(`--platform 需要 target=artifact 形式：${value}`)
      opts.platforms.push({ target: value.slice(0, at), artifact: value.slice(at + 1) })
      break
    }
    default:
      fail(`未知参数 ${flag}`)
  }
}

if (!opts.version) fail('缺少 --version')
if (!opts.baseUrl) fail('缺少 --base-url')
if (opts.platforms.length === 0) fail('至少需要一个 --platform')

const platforms = {}
for (const { target, artifact } of opts.platforms) {
  if (!existsSync(artifact)) fail(`更新包不存在：${artifact}`)

  const sig = `${artifact}.sig`
  if (!existsSync(sig)) {
    fail(`签名文件不存在：${sig}（构建时是否设置了 TAURI_SIGNING_PRIVATE_KEY？）`)
  }

  platforms[target] = {
    signature: readFileSync(sig, 'utf8').trim(),
    url: `${opts.baseUrl}/${encodeURIComponent(basename(artifact))}`,
  }
}

const manifest = {
  version: opts.version,
  notes: opts.notes,
  pub_date: new Date().toISOString(),
  platforms,
}

writeFileSync(opts.out, `${JSON.stringify(manifest, null, 2)}\n`)
process.stdout.write(`已生成 ${opts.out}：${Object.keys(platforms).join('、')}\n`)
