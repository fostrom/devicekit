import { test } from 'node:test'
import assert from 'node:assert/strict'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import fs from 'node:fs'
import { execFileSync } from 'node:child_process'
import http from 'node:http'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
const PACKAGE_DIR = path.join(__dirname, '..')
const AGENT_PATH = path.join(PACKAGE_DIR, '.agent', 'fostrom-device-agent')
const SOCKET_PATH = '/tmp/fostrom/agent.sock'

const FLEET_ID = 'FOSTROM0'
const DEVICE_ID = 'SANDBOX001'
const DEVICE_SECRET = 'FOS-TESTFLIGHTCONNFOSTROM0SANDBOX001'

// Import after path setup to ensure local sources are used.
const { default: Fostrom } = await import('../index.js')

function ensureAgentBinary(t) {
  if (!fs.existsSync(AGENT_PATH)) {
    t.skip('Device Agent binary not found; run JS SDK build first.')
    return false
  }
  return true
}

function socketExists() {
  try {
    fs.accessSync(SOCKET_PATH, fs.constants.F_OK)
    return true
  } catch {
    return false
  }
}

async function waitForSocketAbsence(timeoutMs = 5000) {
  const deadline = Date.now() + timeoutMs
  while (socketExists() && Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
}

async function waitForAgentResponse(timeoutMs = 10000) {
  let lastError = null
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    try {
      const resp = await unixRequest('/')
      if (resp.statusCode === 200) {
        return resp
      }
    } catch (err) {
      lastError = err
    }
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  if (lastError) {
    throw new Error(`Device Agent did not become ready: ${lastError.message}`)
  }
  throw new Error('Device Agent did not respond with 200 OK in time')
}

function unixRequest(pathname, method = 'GET') {
  return new Promise((resolve, reject) => {
    const req = http.request(
      {
        socketPath: SOCKET_PATH,
        path: pathname,
        method,
      },
      (res) => {
        const chunks = []
        res.on('data', (chunk) => chunks.push(Buffer.from(chunk)))
        res.on('end', () => {
          const bodyBuf = Buffer.concat(chunks)
          let json = null
          const contentType = res.headers['content-type'] || ''
          if (contentType.toLowerCase().includes('application/json') && bodyBuf.length > 0) {
            try {
              json = JSON.parse(bodyBuf.toString('utf8'))
            } catch {
              json = null
            }
          }
          resolve({
            statusCode: res.statusCode || 0,
            headers: res.headers,
            bodyJson: json,
          })
        })
      }
    )

    req.on('error', reject)
    req.end()
  })
}

function readAgentVersion(t) {
  try {
    const raw = execFileSync(AGENT_PATH, ['version'], { encoding: 'utf8' })
    return raw.trim()
  } catch (err) {
    t.skip(`Unable to read device agent version: ${err.message}`)
    return null
  }
}

test('device agent not running initially', { concurrency: false }, async (t) => {
  if (!ensureAgentBinary(t)) return

  Fostrom.stopAgent()
  await waitForSocketAbsence()

  if (socketExists()) {
    t.skip('Device Agent socket still present; environment already running agent.')
    return
  }

  await assert.rejects(() => unixRequest('/'), /ENOENT|ECONNREFUSED/)
})

test('start Fostrom SDK and verify agent headers', { concurrency: false }, async (t) => {
  if (!ensureAgentBinary(t)) return

  Fostrom.stopAgent()
  await waitForSocketAbsence()

  const agentVersion = readAgentVersion(t)
  if (!agentVersion) return

  const previousLocalMode = process.env.FOSTROM_LOCAL_MODE
  process.env.FOSTROM_LOCAL_MODE = 'true'

  const app = new Fostrom({
    fleet_id: FLEET_ID,
    device_id: DEVICE_ID,
    device_secret: DEVICE_SECRET,
    stopAgentOnExit: true,
  })

  try {
    await app.start()
    const response = await waitForAgentResponse()

    assert.equal(response.statusCode, 200)
    const headers = response.headers

    assert.equal(headers['x-powered-by'], 'Fostrom')
    assert.equal(headers['x-protocol'], 'Moonlight')
    assert.equal(headers['x-protocol-version'], '1')
    assert.equal(headers['x-api-version'], '1')

    const trimmedVersion = agentVersion.replace(/^v/, '')
    assert.equal(headers['x-agent-version'], trimmedVersion)
    assert.equal(headers['server'], `Fostrom-Device-Agent/${agentVersion}`)
    assert.equal(headers['x-fleet-id'], FLEET_ID)
    assert.equal(headers['x-device-id'], DEVICE_ID)

    assert.ok(response.bodyJson && typeof response.bodyJson === 'object')
  } finally {
    await app.shutdown(true)
    if (previousLocalMode === undefined) delete process.env.FOSTROM_LOCAL_MODE
    else process.env.FOSTROM_LOCAL_MODE = previousLocalMode
    await waitForSocketAbsence()
  }
})
