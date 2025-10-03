import { execSync } from "child_process"
import { fileURLToPath } from "url"
import path from "path"
import http from "node:http"
import { stringifyJSON, parseJSON } from "./json.js"

function agent_path() {
  const __dirname = path.dirname(fileURLToPath(import.meta.url))
  return path.join(__dirname, ".agent", "fostrom-device-agent")
}

class FostromError extends Error {
  constructor(atom, msg) {
    super(`${atom}: ${msg}`)
    this.error = atom
  }
}

export class Mail {
  id
  name
  payload = undefined
  mailbox_size
  #instance

  constructor(fostrom_instance, id, name, payload, mailbox_size) {
    this.#instance = fostrom_instance
    this.id = id
    this.name = name
    this.payload = payload
    this.mailbox_size = mailbox_size
  }

  async ack() { return await this.#instance.__mail_op("ack", this.id) }
  async reject() { return await this.#instance.__mail_op("reject", this.id) }
  async requeue() { return await this.#instance.__mail_op("requeue", this.id) }
}

export default class Fostrom {
  #log = true
  #creds = {}
  #sseBuffer = ""
  #sseReq = null
  #reconnectTimer = null
  #stopped = true
  #installedExitHandler = false
  #stopAgentOnExit = false
  #runtimeEnv = null

  static #SOCK = "/tmp/fostrom/agent.sock"

  onMail = async mail => {
    if (this.#log) {
      console.warn(`[Fostrom] Received Mail (Mailbox Size: ${mail.mailbox_size}): ${mail.name} -> ID ${mail.id}`)
      console.warn("              Auto-Acknowledging Mail. Define Mail Handler to handle incoming mail.\n              `fostrom.on_mail = async (mail) => { ...; await mail.ack(); }`\n")
    }
    await mail.ack()
  }

  connected = () => {
    if (this.#log) console.info("[Fostrom] Connected")
  }

  unauthorized = (reason, after) => {
    if (this.#log) {
      const after_s = Math.floor((after || 0) / 1000);
      console.error(`[Fostrom] Unauthorized: ${reason}. Reconnecting in ${after_s} seconds...`)
    }
  }

  reconnecting = (reason, after) => {
    if (this.#log) {
      const after_s = Math.floor((after || 0) / 1000);
      console.warn(`[Fostrom] Failed to connect: ${reason}. Reconnecting in ${after_s} seconds...`)
    }
  }

  constructor(config = {}) {
    if (!config.fleet_id) throw "[Fostrom] Fleet ID required."
    if (!config.device_id) throw "[Fostrom] Device ID required."
    if (!config.device_secret) throw "[Fostrom] Device Secret required."
    this.#creds.fleet_id = config.fleet_id
    this.#creds.device_id = config.device_id
    this.#creds.device_secret = config.device_secret
    if (config.log == false) this.#log = false
    this.#stopAgentOnExit = Boolean(config.stopAgentOnExit || false)
    this.#runtimeEnv = config.env || config.runtimeEnv || process.env.NODE_ENV || null
  }

  #installExitHandler() {
    if (this.#installedExitHandler) return
    process.on('exit', () => {
      try { this.shutdown() } catch { }
    })
    this.#installedExitHandler = true
  }

  #start_agent() {
    const { fleet_id, device_id, device_secret } = this.#creds

    const env = {
      ...process.env,
      FOSTROM_FLEET_ID: fleet_id,
      FOSTROM_DEVICE_ID: device_id,
      FOSTROM_DEVICE_SECRET: device_secret,
    }
    if (this.#runtimeEnv && String(this.#runtimeEnv).trim() !== "") {
      env["FOSTROM_RUNTIME_ENV"] = String(this.#runtimeEnv)
    }

    const args = ["start"]

    try {
      const output = execSync(`${agent_path()} ${args.join(" ")}`, { encoding: "utf8", env })
      const out = output.trim()
      if (out.startsWith("started:")) return
      if (out.startsWith("already_started:")) return
      return
    } catch (error) {
      const out = (error.stdout || "").toString().trim()
      if (out) {
        const [atom, rest] = out.split(":", 2)
        throw new FostromError(atom || "failed", (rest || "Failed to start Device Agent").trim())
      }
      throw new FostromError("failed", "Failed to start Device Agent")
    }
  }

  async #req(path = "/", method = "GET", payload = null) {
    if (!["GET", "PUT", "POST", "DELETE", "HEAD"].includes(method)) {
      throw new Error(`Unsupported ${method} Request for path ${path}`)
    }

    const { fleet_id, device_id } = this.#creds
    const headers = {
      "X-Fleet-ID": fleet_id,
      "X-Device-ID": device_id,
    }

    let bodyString = null
    if (method === "POST" || method === "PUT") {
      headers["Content-Type"] = "application/json; charset=utf-8"
      headers["Accept"] = "application/json"
      if (payload === undefined || payload === null) {
        bodyString = "null"
      } else {
        bodyString = stringifyJSON(payload)
      }
      headers["Content-Length"] = Buffer.byteLength(bodyString, 'utf8')
    } else {
      headers["Accept"] = "application/json"
    }

    const res = await Fostrom.#unix_request({ path, method, headers, body: bodyString })

    if (res.statusCode < 200 || res.statusCode >= 300) {
      if (res.bodyJson && res.bodyJson.error) {
        const msg = res.bodyJson.msg || "Request failed"
        throw new FostromError(res.bodyJson.error, msg)
      }
      throw new FostromError("request_failed", "Communicating with the Device Agent failed")
    }

    return res
  }

  static stopAgent() {
    try {
      execSync(`${agent_path()} stop`, { encoding: "utf8" })
    } catch (e) {
      console.error("[Fostrom] Failed to stop the Fostrom Device Agent")
    }
  }

  async start() {
    if (!this.#stopped) return
    this.#installExitHandler()
    this.#start_agent()
    this.#stopped = false
    this.#open_event_stream()
  }

  async shutdown(stopAgent = null) {
    this.#stopped = true
    try {
      if (this.#reconnectTimer) {
        clearTimeout(this.#reconnectTimer)
        this.#reconnectTimer = null
      }
      if (this.#sseReq && typeof this.#sseReq.destroy === 'function') {
        this.#sseReq.destroy()
      }
    } catch { }
    this.#sseReq = null
    const doStop = (stopAgent === null) ? this.#stopAgentOnExit : Boolean(stopAgent)
    if (doStop) Fostrom.stopAgent()
  }

  async sendDatapoint(name, payload) {
    Fostrom.#validate_pulse_name(name)
    const res = await this.#req(`/pulse/datapoint/${name}`, "POST", payload)
    return
  }

  async sendMsg(name, payload) {
    Fostrom.#validate_pulse_name(name)
    const res = await this.#req(`/pulse/msg/${name}`, "POST", payload)
    return
  }

  async mailboxStatus() {
    const res = await this.#req(`/mailbox/next`, "HEAD")
    const h = res.headers
    const empty = Fostrom.#parse_bool(h["x-mailbox-empty"]) === true
    if (empty) {
      return { mailbox_size: 0, next_mail_id: null, next_mail_name: null }
    }
    return {
      mailbox_size: Number(h["x-mailbox-size"]) || 0,
      next_mail_id: Number(h["x-mail-id"]) || null,
      next_mail_name: h["x-mail-name"] || null,
    }
  }

  async nextMail() {
    const res = await this.#req(`/mailbox/next`, "GET")
    const h = res.headers
    const empty = Fostrom.#parse_bool(h["x-mailbox-empty"]) === true
    if (empty) return null

    const mailbox_size = Number(h["x-mailbox-size"]) || 0
    const id = Number(h["x-mail-id"]) || null
    const name = h["x-mail-name"] || null
    const hasPayload = Fostrom.#parse_bool(h["x-mail-has-payload"]) === true
    const payload = hasPayload ? (res.bodyJson ?? null) : null
    return new Mail(this, id, name, payload, mailbox_size)
  }

  async __mail_op(op, mail_id) {
    if (op != "ack" && op != "reject" && op != "requeue") {
      throw new Error("Invalid Mailbox Operation")
    }

    const res = await this.#req(`/mailbox/${op}/${mail_id}`, "PUT")
    const more = Fostrom.#parse_bool(res.headers["x-mail-available"]) === true
    if (more) {
      const mail = await this.nextMail()
      if (mail) await this.#deliverMail(mail)
    }
    return true
  }

  async #event_handler(event_object) {
    const { event } = event_object
    const data = event_object.data

    switch (event) {
      case 'connected':
        this.connected()
        break
      case 'disconnected': {
        if (data && typeof data.error === 'string') {
          const err = data.error
          const after = Number(data.reconnecting_in_ms || 0)
          const reason = (typeof err === 'string') ? err.split(':', 2)[0].trim() : ''
          if (reason === "unauthorized") this.unauthorized(err, after)
          else this.reconnecting(err, after)
        }
        break
      }
      case 'new_mail': {
        const mail = await this.nextMail()
        if (mail) await this.#deliverMail(mail)
        break
      }
      default:
        break
    }
  }

  async #open_event_stream() {
    if (this.#stopped) return
    if (this.#sseReq) return
    const { fleet_id, device_id } = this.#creds
    const options = {
      socketPath: Fostrom.#SOCK,
      path: "/events",
      method: "GET",
      headers: {
        "Accept": "text/event-stream",
        "X-Fleet-ID": fleet_id,
        "X-Device-ID": device_id,
        "Connection": "keep-alive",
      }
    }

    const req = http.request(options, (res) => {
      res.setEncoding('utf8')
      res.on('data', (chunk) => {
        this.#sseBuffer = Fostrom.#parse_events(this.#sseBuffer, chunk, this.#event_handler.bind(this))
      })
      res.on('error', () => {
        this.#sseReq = null
        if (!this.#stopped) {
          this.#reconnectTimer = setTimeout(() => this.#open_event_stream(), 500)
        }
      })
      res.on('aborted', () => {
        this.#sseReq = null
        if (!this.#stopped) {
          this.#reconnectTimer = setTimeout(() => this.#open_event_stream(), 500)
        }
      })
      res.on('end', () => {
        this.#sseReq = null
        if (!this.#stopped) {
          this.#reconnectTimer = setTimeout(() => this.#open_event_stream(), 250)
        }
      })
    })

    req.on('error', (_err) => {
      this.#sseReq = null
      if (!this.#stopped) {
        this.#reconnectTimer = setTimeout(() => this.#open_event_stream(), 500)
      }
    })

    req.end()
    this.#sseReq = req
  }

  // --- Static helpers ---
  static #parse_bool(v) {
    if (typeof v !== 'string') return false
    const s = v.toLowerCase()
    return s === 'true' || s === '1' || s === 'yes'
  }

  static #validate_pulse_name(name) {
    if (typeof name !== 'string') name = String(name)
    if (name.length === 0 || name.length > 255) {
      throw new FostromError('invalid_name', 'Pulse name must be 1..255 characters')
    }
    if (!/^[A-Za-z0-9_-]+$/.test(name)) {
      throw new FostromError('invalid_name', 'Pulse name may contain only A-Za-z0-9_-')
    }
  }

  static async #unix_request({ path, method, headers = {}, body = null }) {
    return new Promise((resolve, reject) => {
      const opts = {
        socketPath: Fostrom.#SOCK,
        path,
        method,
        headers,
      }

      const req = http.request(opts, (res) => {
        const chunks = []
        res.on('data', (chunk) => chunks.push(Buffer.from(chunk)))
        res.on('end', () => {
          const bodyBuf = Buffer.concat(chunks)
          const bodyText = bodyBuf.toString('utf8')
          const ct = res.headers['content-type'] || ''
          let bodyJson = null
          if (ct.toLowerCase().includes('application/json') && bodyText.length > 0) {
            try { bodyJson = parseJSON(bodyText) } catch { bodyJson = null }
          }
          resolve({
            statusCode: res.statusCode || 0,
            headers: res.headers || {},
            bodyText,
            bodyJson,
          })
        })
      })

      req.on('error', (err) => {
        reject(new FostromError('req_failed', `Communicating with the Device Agent failed: ${err.message}`))
      })

      if (body !== null && body !== undefined && method !== 'HEAD') {
        req.write(body)
      }
      req.end()
    })
  }

  static #parse_events(buffer, chunk, event_handler) {
    buffer += chunk
    const lines = buffer.split('\n')
    buffer = lines.pop() || ''

    let event = {}
    for (const raw of lines) {
      const line = raw.replace(/\r$/, '')
      if (line === '') {
        if (event.data && event.data !== '') {
          try { event.data = parseJSON(event.data) } catch { /* ignore */ }
        }
        if (event.event) event_handler(event)
        event = {}
      } else if (line.startsWith('data: ')) {
        event.data = (event.data || '') + line.slice(6)
      } else if (line.startsWith('event: ')) {
        event.event = line.slice(7)
      } else if (line.startsWith('id: ')) {
        const ts = parseInt(line.slice(4))
        if (!Number.isNaN(ts)) event.timestamp = new Date(ts)
      }
    }

    return buffer
  }

  async #deliverMail(mail) {
    try {
      await this.onMail(mail)
    } catch (e) {
      if (this.#log) console.error(`[Fostrom] onMail handler threw; auto-rejecting mail ${mail.id} (${mail.name})`, e)
      try { await mail.reject() } catch { }
    }
  }
}
