import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import {
  build_sdk_manifest,
  detect_host_app,
  detect_runtime,
  read_sdk_version,
} from '../manifest.js'

function withCwd(dir, fn) {
  const previous = process.cwd()
  process.chdir(dir)
  try {
    return fn()
  } finally {
    process.chdir(previous)
  }
}

function mktmp(prefix) {
  return fs.mkdtempSync(path.join(os.tmpdir(), prefix))
}

// -------------------------
// build_sdk_manifest
// -------------------------

test('build_sdk_manifest returns valid JSON with sdk === "js"', () => {
  const payload = JSON.parse(build_sdk_manifest('prod'))
  assert.equal(payload.sdk, 'js')
  assert.equal(typeof payload.sdk_manifest, 'object')
  assert.ok(payload.sdk_manifest !== null)
})

test('build_sdk_manifest includes sdk_version and runtime fields', () => {
  const { sdk_manifest } = JSON.parse(build_sdk_manifest('prod'))
  assert.equal(sdk_manifest.sdk_version, read_sdk_version())
  assert.ok(sdk_manifest.runtime, 'runtime field should be present')
  assert.equal(sdk_manifest.runtime_env, 'prod')
})

test('build_sdk_manifest omits runtime_env when blank or undefined', () => {
  for (const blank of [undefined, null, '', '   ']) {
    const { sdk_manifest } = JSON.parse(build_sdk_manifest(blank))
    assert.ok(
      !('runtime_env' in sdk_manifest),
      `runtime_env should be omitted for ${JSON.stringify(blank)}`,
    )
  }
})

test('build_sdk_manifest does not contain null/undefined values', () => {
  const { sdk_manifest } = JSON.parse(build_sdk_manifest('prod'))
  for (const [k, v] of Object.entries(sdk_manifest)) {
    assert.ok(v !== null && v !== undefined, `${k} should not be null/undefined`)
  }
})

// -------------------------
// detect_runtime
// -------------------------

test('detect_runtime under Node returns runtime: "node" with versions', () => {
  // The test runner is Node (or Bun, but we don't run tests under Bun here).
  const info = detect_runtime()
  if (process.versions.bun) {
    assert.equal(info.runtime, 'bun')
    assert.equal(info.bun_version, process.versions.bun)
  } else if (typeof globalThis.Deno !== 'undefined') {
    assert.equal(info.runtime, 'deno')
  } else {
    assert.equal(info.runtime, 'node')
    assert.equal(info.node_version, process.versions.node)
    assert.equal(info.v8_version, process.versions.v8)
  }
})

// -------------------------
// detect_host_app
// -------------------------

test('detect_host_app finds nearest package.json walking up from cwd', () => {
  const root = mktmp('fostrom-host-')
  const nested = path.join(root, 'src', 'app')
  fs.mkdirSync(nested, { recursive: true })

  fs.writeFileSync(
    path.join(root, 'package.json'),
    JSON.stringify({ name: 'my-app', version: '0.4.0' }),
  )

  withCwd(nested, () => {
    const { app_name, app_version } = detect_host_app()
    assert.equal(app_name, 'my-app')
    assert.equal(app_version, '0.4.0')
  })

  fs.rmSync(root, { recursive: true, force: true })
})

test('detect_host_app skips a package.json named "fostrom"', () => {
  const root = mktmp('fostrom-host-')
  fs.writeFileSync(
    path.join(root, 'package.json'),
    JSON.stringify({ name: 'fostrom', version: '0.1.0' }),
  )

  withCwd(root, () => {
    const { app_name, app_version } = detect_host_app()
    assert.equal(app_name, null)
    assert.equal(app_version, null)
  })

  fs.rmSync(root, { recursive: true, force: true })
})

test('detect_host_app stops at node_modules ancestors', () => {
  const root = mktmp('fostrom-host-')
  const inside = path.join(root, 'node_modules', 'some_pkg')
  fs.mkdirSync(inside, { recursive: true })

  // Place an outer package.json that would be picked up if the walk did not stop.
  fs.writeFileSync(
    path.join(root, 'package.json'),
    JSON.stringify({ name: 'outer-app', version: '9.9.9' }),
  )

  withCwd(inside, () => {
    const { app_name, app_version } = detect_host_app()
    assert.equal(app_name, null)
    assert.equal(app_version, null)
  })

  fs.rmSync(root, { recursive: true, force: true })
})

test('detect_host_app handles missing or unreadable package.json gracefully', () => {
  const root = mktmp('fostrom-host-')
  withCwd(root, () => {
    const result = detect_host_app()
    // We can't guarantee no ancestor package.json exists, so just assert the shape.
    assert.equal(typeof result, 'object')
    assert.ok('app_name' in result)
    assert.ok('app_version' in result)
  })
  fs.rmSync(root, { recursive: true, force: true })
})

// -------------------------
// read_sdk_version
// -------------------------

test('read_sdk_version matches the SDK package.json version', () => {
  const pkg = JSON.parse(fs.readFileSync(path.join(import.meta.dirname, '..', 'package.json'), 'utf8'))
  assert.equal(read_sdk_version(), pkg.version)
})
