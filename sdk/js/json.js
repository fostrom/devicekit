// JSON functions to correctly handle BigInts

function stringifyJSON(value) {
  if (typeof JSON.rawJSON !== 'function') {
    throw new Error('JSON.rawJSON not available; requires Node.js 22+/Bun 1.1+')
  }
  return JSON.stringify(value, (_key, v) => {
    if (typeof v === 'bigint') {
      return JSON.rawJSON(v.toString())
    }
    return v
  })
}

function parseJSON(text) {
  if (typeof JSON.rawJSON !== 'function') {
    throw new Error('JSON.rawJSON not available; requires Node.js 22+/Bun 1.1+')
  }
  return JSON.parse(text, (_key, v, ctx) => {
    if (typeof v === 'number' && Number.isInteger(v) && !Number.isSafeInteger(v) && ctx?.source) {
      try {
        return BigInt(ctx.source)
      } catch {
        return v
      }
    }
    return v
  })
}

export { stringifyJSON, parseJSON }
