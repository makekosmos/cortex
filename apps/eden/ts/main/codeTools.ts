import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { spawn } from 'node:child_process'
import type { OxlintConfig } from 'oxlint'

export type CodeLintSeverity = 'off' | 'warn' | 'error'

export type CodeToolsPreset = 'myagkiy' | 'balans' | 'strogiy'
export type CodeLintTrigger = 'on_save' | 'on_idle'

export interface CodeToolsSettings {
  formatOnSave: boolean;
  preset: CodeToolsPreset;
  lintTrigger: CodeLintTrigger;
}

export interface CodeLintDiagnostic {
  severity: 'error' | 'warning';
  message: string;
  line?: number;
  column?: number;
  ruleId?: string;
}

export interface CodeLintResult {
  ok: boolean;
  diagnostics: CodeLintDiagnostic[];
  unsupportedReason?: string;
}

export interface CodeFormatResult {
  ok: boolean;
  code: string;
  errors: string[];
  unsupportedReason?: string;
}

export const defaultCodeToolsSettings: CodeToolsSettings = {
  formatOnSave: true,
  preset: 'balans',
  lintTrigger: 'on_idle',
}

const lintProfileByPreset: Record<CodeToolsPreset, { correctness: CodeLintSeverity; suspicious: CodeLintSeverity; style: CodeLintSeverity }> = {
  myagkiy: {
    correctness: 'warn',
    suspicious: 'warn',
    style: 'off',
  },
  balans: {
    correctness: 'error',
    suspicious: 'warn',
    style: 'warn',
  },
  strogiy: {
    correctness: 'error',
    suspicious: 'error',
    style: 'error',
  },
}

const formatProfileByPreset: Record<CodeToolsPreset, { printWidth: number; tabWidth: number; semi: boolean; singleQuote: boolean; trailingComma: 'all' | 'es5' | 'none' }> = {
  myagkiy: {
    printWidth: 110,
    tabWidth: 2,
    semi: false,
    singleQuote: false,
    trailingComma: 'none',
  },
  balans: {
    printWidth: 100,
    tabWidth: 2,
    semi: true,
    singleQuote: true,
    trailingComma: 'es5',
  },
  strogiy: {
    printWidth: 90,
    tabWidth: 2,
    semi: true,
    singleQuote: true,
    trailingComma: 'all',
  },
}

const extensionByLanguage: Record<string, string> = {
  bash: 'sh',
  css: 'css',
  html: 'html',
  javascript: 'js',
  json: 'json',
  jsx: 'jsx',
  markdown: 'md',
  plaintext: 'txt',
  text: 'txt',
  typescript: 'ts',
  tsx: 'tsx',
}

const lintableLanguages = new Set(['javascript', 'jsx', 'typescript', 'tsx'])
const formattableLanguages = new Set(Object.keys(extensionByLanguage))

function normalizeLanguage(language: string) {
  const normalizedLanguage = language.trim().toLowerCase()
  return normalizedLanguage === 'text' ? 'plaintext' : normalizedLanguage
}

function getExtension(language: string) {
  return extensionByLanguage[normalizeLanguage(language)] ?? 'txt'
}

function getOxlintConfig(settings: CodeToolsSettings): OxlintConfig {
  const lintProfile = lintProfileByPreset[settings.preset]
  return {
    categories: {
      correctness: lintProfile.correctness,
      suspicious: lintProfile.suspicious,
      style: lintProfile.style,
    },
    plugins: ['typescript'],
  }
}

function getFormatOptions(settings: CodeToolsSettings) {
  const formatProfile = formatProfileByPreset[settings.preset]
  return {
    printWidth: formatProfile.printWidth,
    tabWidth: formatProfile.tabWidth,
    semi: formatProfile.semi,
    singleQuote: formatProfile.singleQuote,
    trailingComma: formatProfile.trailingComma,
    insertFinalNewline: true,
  }
}

async function withTempFile<T>(language: string, code: string, callback: (filePath: string, dirPath: string) => Promise<T>) {
  const dirPath = await fs.mkdtemp(path.join(os.tmpdir(), 'eden-oxc-'))
  const filePath = path.join(dirPath, `snippet.${getExtension(language)}`)
  await fs.writeFile(filePath, code, 'utf-8')

  try {
    return await callback(filePath, dirPath)
  } finally {
    await fs.rm(dirPath, { recursive: true, force: true })
  }
}

async function runNodeBin(scriptPath: string, args: string[], workdir: string) {
  return await new Promise<{ stdout: string; stderr: string; exitCode: number }>((resolve, reject) => {
    const child = spawn(process.execPath, [scriptPath, ...args], {
      cwd: workdir,
      stdio: ['ignore', 'pipe', 'pipe'],
    })

    let stdout = ''
    let stderr = ''

    child.stdout.on('data', chunk => {
      stdout += chunk.toString()
    })

    child.stderr.on('data', chunk => {
      stderr += chunk.toString()
    })

    child.on('error', reject)
    child.on('close', exitCode => {
      resolve({ stdout, stderr, exitCode: exitCode ?? 1 })
    })
  })
}

function parseOxlintOutput(stdout: string): CodeLintDiagnostic[] {
  const parsed = JSON.parse(stdout) as Array<{
    severity?: string;
    message?: string;
    ruleId?: string;
    labels?: Array<{ span?: { offset?: number; length?: number }; line?: number; column?: number }>;
  }>

  return parsed.map(item => {
    const firstLabel = item.labels?.[0]
    return {
      severity: item.severity === 'warning' ? 'warning' : 'error',
      message: item.message ?? 'Unknown oxlint diagnostic',
      line: firstLabel?.line,
      column: firstLabel?.column,
      ruleId: item.ruleId,
    }
  })
}

export async function lintCode(language: string, code: string, settings: CodeToolsSettings): Promise<CodeLintResult> {
  const normalizedLanguage = normalizeLanguage(language)

  if (!lintableLanguages.has(normalizedLanguage)) {
    return {
      ok: true,
      diagnostics: [],
      unsupportedReason: 'Lint is currently available for JavaScript and TypeScript code blocks.',
    }
  }

  return await withTempFile(normalizedLanguage, code, async (filePath, dirPath) => {
    const configPath = path.join(dirPath, '.oxlintrc.json')
    await fs.writeFile(configPath, JSON.stringify(getOxlintConfig(settings), null, 2), 'utf-8')

    const scriptPath = path.join(process.env.APP_ROOT ?? process.cwd(), 'node_modules', 'oxlint', 'bin', 'oxlint')
    const result = await runNodeBin(scriptPath, ['--format', 'json', '-c', configPath, filePath], dirPath)

    if (!result.stdout.trim()) {
      return {
        ok: result.exitCode === 0,
        diagnostics: [],
      }
    }

    return {
      ok: result.exitCode === 0,
      diagnostics: parseOxlintOutput(result.stdout),
    }
  })
}

export async function formatCode(language: string, code: string, settings: CodeToolsSettings): Promise<CodeFormatResult> {
  const normalizedLanguage = normalizeLanguage(language)

  if (!formattableLanguages.has(normalizedLanguage)) {
    return {
      ok: true,
      code,
      errors: [],
      unsupportedReason: 'Formatting is currently available for text, Markdown, HTML, CSS, JSON, JavaScript, and TypeScript code blocks.',
    }
  }

  return await withTempFile(normalizedLanguage, code, async (filePath, dirPath) => {
    const configPath = path.join(dirPath, '.oxfmtrc.json')
    await fs.writeFile(configPath, JSON.stringify(getFormatOptions(settings), null, 2), 'utf-8')

    const scriptPath = path.join(process.env.APP_ROOT ?? process.cwd(), 'node_modules', 'oxfmt', 'bin', 'oxfmt')
    const result = await runNodeBin(scriptPath, ['--config', configPath, filePath], dirPath)
    const formattedCode = await fs.readFile(filePath, 'utf-8')

    return {
      ok: result.exitCode === 0,
      code: formattedCode,
      errors: result.stderr.trim() ? [result.stderr.trim()] : [],
    }
  })
}
