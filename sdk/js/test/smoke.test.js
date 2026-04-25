import { test } from 'node:test'
import assert from 'node:assert/strict'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import fs from 'node:fs'

import Fostrom, { Mail } from '../index.js'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const PACKAGE_DIR = path.join(__dirname, '..')
const AGENT_PATH = path.join(PACKAGE_DIR, '.agent', 'fostrom-device-agent')
const pkgJson = JSON.parse(fs.readFileSync(path.join(PACKAGE_DIR, 'package.json'), 'utf8'))

test('module exports are available', () => {
  assert.ok(Fostrom, 'Fostrom default export should be defined')
  assert.equal(typeof Fostrom, 'function', 'Fostrom should be a constructor')
  assert.ok(Mail, 'Mail export should be defined')
})

test('constructor validates required credentials', () => {
  assert.throws(() => new Fostrom(), /Fleet ID required/)
  assert.throws(() => new Fostrom({ fleet_id: 'F', device_secret: 'S' }), /Device ID required/)
  assert.throws(() => new Fostrom({ fleet_id: 'F', device_id: 'D' }), /Device Secret required/)
})

test('package metadata is consistent', () => {
  assert.equal(pkgJson.name, 'fostrom')
  assert.ok(pkgJson.version && typeof pkgJson.version === 'string' && pkgJson.version.length > 0)
})

test('device agent binary exists', () => {
  assert.ok(fs.existsSync(AGENT_PATH), 'Bundled device agent should exist')
  const stats = fs.statSync(AGENT_PATH)
  assert.ok(stats.isFile() || stats.isSymbolicLink(), 'Device agent path should be a file or symlink')
})

test('constructor accepts collect_telemetry across all valid forms', () => {
  const base = { fleet_id: 'F', device_id: 'D', device_secret: 'S' }
  assert.doesNotThrow(() => new Fostrom({ ...base }))
  assert.doesNotThrow(() => new Fostrom({ ...base, collect_telemetry: true }))
  assert.doesNotThrow(() => new Fostrom({ ...base, collect_telemetry: false }))
  assert.doesNotThrow(() => new Fostrom({ ...base, collect_telemetry: 90 }))
  // Numbers < 15 are accepted; #start_agent will silently omit the env var,
  // letting the agent fall back to its default interval.
  assert.doesNotThrow(() => new Fostrom({ ...base, collect_telemetry: 10 }))
  assert.doesNotThrow(() => new Fostrom({ ...base, collect_telemetry: 0 }))
})
