// Internal: not part of the public API. The package's `exports` map intentionally
// does not surface this file, so end users cannot `import` it. Tests reach in via
// a relative path within the package.

import { fileURLToPath } from "url"
import path from "path"
import fs from "node:fs"

export function sdk_dir() {
  return path.dirname(fileURLToPath(import.meta.url))
}

export function read_sdk_version() {
  try {
    const pkgPath = path.join(sdk_dir(), "package.json")
    const raw = fs.readFileSync(pkgPath, "utf8")
    const pkg = JSON.parse(raw)
    if (pkg && typeof pkg.version === "string") return pkg.version
  } catch { }
  return null
}

export function detect_runtime() {
  if (typeof process !== "undefined" && process.versions) {
    if (process.versions.bun) {
      return {
        runtime: "bun",
        bun_version: process.versions.bun,
        node_version: process.versions.node || null,
      }
    }
    if (typeof globalThis.Deno !== "undefined") {
      return {
        runtime: "deno",
        deno_version: (globalThis.Deno.version && globalThis.Deno.version.deno) || null,
        v8_version: process.versions.v8 || null,
      }
    }
    return {
      runtime: "node",
      node_version: process.versions.node || null,
      v8_version: process.versions.v8 || null,
    }
  }
  return { runtime: "unknown" }
}

export function detect_host_app() {
  let dir
  try {
    dir = path.resolve(process.cwd())
  } catch {
    return { app_name: null, app_version: null }
  }

  const sdkDirResolved = path.resolve(sdk_dir())

  while (true) {
    // Don't claim app metadata from inside node_modules or the SDK package itself.
    if (dir.split(path.sep).includes("node_modules")) break
    if (dir === sdkDirResolved) break

    const pkgPath = path.join(dir, "package.json")
    try {
      const stat = fs.statSync(pkgPath)
      if (stat.isFile()) {
        const raw = fs.readFileSync(pkgPath, "utf8")
        const pkg = JSON.parse(raw)
        const name = (pkg && typeof pkg.name === "string") ? pkg.name : null
        const version = (pkg && typeof pkg.version === "string") ? pkg.version : null
        if (name && name.toLowerCase() === "fostrom") {
          return { app_name: null, app_version: null }
        }
        return { app_name: name, app_version: version }
      }
    } catch { }

    const parent = path.dirname(dir)
    if (parent === dir) break
    dir = parent
  }

  return { app_name: null, app_version: null }
}

export function build_sdk_manifest(runtimeEnv) {
  const runtimeInfo = detect_runtime()
  const { app_name, app_version } = detect_host_app()

  const sdk_manifest = {
    sdk_version: read_sdk_version(),
    ...runtimeInfo,
  }

  if (runtimeEnv && String(runtimeEnv).trim() !== "") {
    sdk_manifest.runtime_env = String(runtimeEnv)
  }

  if (app_name) sdk_manifest.app_name = app_name
  if (app_version) sdk_manifest.app_version = app_version

  for (const k of Object.keys(sdk_manifest)) {
    if (sdk_manifest[k] === null || sdk_manifest[k] === undefined) {
      delete sdk_manifest[k]
    }
  }

  return JSON.stringify({ sdk: "js", sdk_manifest })
}
