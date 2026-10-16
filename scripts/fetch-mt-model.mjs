#!/usr/bin/env node

import { spawnSync } from 'node:child_process'
import { copyFile, existsSync } from 'node:fs'
import { mkdir, readdir, rm, stat } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const VENV = join(ROOT, '.cache', 'ct2venv')
const PY = join(VENV, 'bin', 'python')
const TOK_CACHE = join(ROOT, '.cache', 'mt')
const MT_DIR = join(ROOT, 'src-tauri', 'assets', 'models', 'mt')

const PAIRS = [
  {
    hf: 'Helsinki-NLP/opus-mt-en-zh',
    dir: 'opus-mt-en-zh-ct2',
    tokenizer: 'tok-en-zh',
    note: 'en→zh (Apache-2.0)',
  },
  {
    hf: 'Helsinki-NLP/opus-mt-zh-en',
    dir: 'opus-mt-zh-en-ct2',
    tokenizer: 'tok-zh-en',
    note: 'zh→en (CC-BY-4.0，需署名)',
  },
]

const force = process.argv.includes('--force')

const SPM_TO_JSON_SCRIPT = `
import sys, site, glob
sp = glob.glob(site.getsitepackages()[0] + "/sentencepiece") + \
     glob.glob(site.getusersitepackages() + "/sentencepiece")
if sp:
    sys.path.insert(0, sp[0])
from tokenizers import SentencePieceUnigramTokenizer
tok = SentencePieceUnigramTokenizer.from_spm(sys.argv[1])
tok.save(sys.argv[2])
`

async function exists(p) {
  try {
    await stat(p)
    return true
  } catch {
    return false
  }
}

async function dirSize(dir) {
  let total = 0
  for (const e of await readdir(dir, { withFileTypes: true })) {
    const p = join(dir, e.name)
    total += e.isDirectory() ? await dirSize(p) : (await stat(p)).size
  }
  return total
}

async function isReady(dir) {
  return (
    (await exists(join(dir, 'model.bin'))) &&
    (await exists(join(dir, 'source-tokenizer.json'))) &&
    (await exists(join(dir, 'target-tokenizer.json')))
  )
}

function run(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { stdio: 'inherit', ...opts })
  if (r.error) throw new Error(`无法执行 ${cmd}：${r.error.message}`)
  if (r.status !== 0) throw new Error(`${cmd} 退出码 ${r.status}`)
}

async function ensureVenv() {
  if (!(await exists(PY))) {
    process.stdout.write('🐍 创建 Python venv（一次性的，约 1-2 分钟）…\n')
    await mkdir(join(ROOT, '.cache'), { recursive: true })
    run('python3', ['-m', 'venv', VENV])
    run(PY, ['-m', 'pip', 'install', '--quiet', '--upgrade', 'pip'])
  }

  const probe = spawnSync(
    PY,
    ['-c', 'import torch, ctranslate2, transformers, sentencepiece, google.protobuf'],
    { stdio: 'ignore' },
  )
  if (probe.status !== 0) {
    process.stdout.write('📦 安装转换依赖（torch 较大，首次约 3-5 分钟）…\n')
    run(PY, [
      '-m',
      'pip',
      'install',
      '--quiet',
      'torch',
      'ctranslate2',
      'transformers',
      'sentencepiece',
      'protobuf',
    ])
  }
}

async function fetchVocab(pair) {
  const out = join(TOK_CACHE, pair.tokenizer)
  if (!force && (await exists(join(out, 'source.spm')))) return out

  await mkdir(out, { recursive: true })
  process.stdout.write(`🔤 下载词表：${pair.hf}\n`)
  run(PY, [
    '-c',
    `from transformers import AutoTokenizer; \
AutoTokenizer.from_pretrained(${JSON.stringify(pair.hf)}).save_pretrained(${JSON.stringify(out)})`,
  ])
  return out
}

async function convert(pair) {
  const out = join(MT_DIR, pair.dir)
  if (!force && (await isReady(out))) {
    const size = await dirSize(out)
    process.stdout.write(
      `✅ ${pair.note} 已就绪（${(size / 1024 / 1024).toFixed(0)} MB）\n`,
    )
    return
  }

  const tokDir = await fetchVocab(pair)

  await mkdir(MT_DIR, { recursive: true })
  await rm(out, { recursive: true, force: true })

  process.stdout.write(`⬇️  转换 ${pair.note}（下载 fp32 权重 + 转 int8）…\n`)
  run(join(VENV, 'bin', 'ct2-transformers-converter'), [
    '--model',
    pair.hf,
    '--output_dir',
    out,
    '--quantization',
    'int8',
  ])

  for (const [spm, outName] of [
    ['source.spm', 'source-tokenizer.json'],
    ['target.spm', 'target-tokenizer.json'],
  ]) {
    process.stdout.write(`🔤 生成词表：${outName}\n`)
    run(PY, ['-c', SPM_TO_JSON_SCRIPT, join(tokDir, spm), join(out, outName)])
  }

  if (!(await isReady(out))) {
    throw new Error(`转换后仍缺文件：${out}`)
  }
  const size = await dirSize(out)
  process.stdout.write(`✅ ${pair.note} 完成（${(size / 1024 / 1024).toFixed(0)} MB）\n`)
}

async function main() {
  process.stdout.write('机器翻译模型（opus-mt + CTranslate2 int8）\n\n')
  await ensureVenv()
  for (const pair of PAIRS) {
    await convert(pair)
  }

  const total = await dirSize(MT_DIR)
  process.stdout.write(`\n✅ 翻译模型全部就绪，合计 ${(total / 1024 / 1024).toFixed(0)} MB\n`)
}

main().catch((e) => {
  console.error(`\n❌ ${e.message}`)
  process.exit(1)
})
