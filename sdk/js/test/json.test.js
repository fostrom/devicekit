import assert from 'node:assert/strict'
import { test } from 'node:test'
import { stringifyJSON, parseJSON } from '../json.js'

test('parse upgrades only unsafe integers to BigInt', () => {
  const result = parseJSON('{"id":9007199254740993,"nested":{"value":-9007199254740993}}')
  assert.strictEqual(typeof result.id, 'bigint')
  assert.strictEqual(result.id, 9007199254740993n)
  assert.strictEqual(typeof result.nested.value, 'bigint')
  assert.strictEqual(result.nested.value, -9007199254740993n)
})

test('parse keeps safe integers, decimals, and scientific notation as numbers', () => {
  const result = parseJSON('{"safe":9007199254740991,"decimal":1.5,"exp":1e5}')
  assert.strictEqual(typeof result.safe, 'number')
  assert.strictEqual(result.safe, 9007199254740991)
  assert.strictEqual(typeof result.decimal, 'number')
  assert.strictEqual(result.decimal, 1.5)
  assert.strictEqual(typeof result.exp, 'number')
  assert.strictEqual(result.exp, 100000)
})

test('parse preserves array ordering and mixes numbers with bigints', () => {
  const result = parseJSON('[1,9007199254740995,1.1,-9007199254740995]')
  assert.strictEqual(result.length, 4)
  assert.strictEqual(typeof result[0], 'number')
  assert.strictEqual(typeof result[1], 'bigint')
  assert.strictEqual(result[1], 9007199254740995n)
  assert.strictEqual(typeof result[2], 'number')
  assert.strictEqual(typeof result[3], 'bigint')
  assert.strictEqual(result[3], -9007199254740995n)
})

test('stringify serializes bigints as bare numbers', () => {
  const json = stringifyJSON({ value: 9007199254740995n })
  assert.strictEqual(json, '{"value":9007199254740995}')
})

test('stringify keeps JSON semantics for arrays and undefined', () => {
  const source = { a: undefined, b: [1, undefined, 2n] }
  const json = stringifyJSON(source)
  assert.strictEqual(json, '{"b":[1,null,2]}')
})

test('stringify respects toJSON if present', () => {
  const date = new Date('2025-01-02T03:04:05.000Z')
  const json = stringifyJSON({ when: date })
  assert.strictEqual(json, '{"when":"2025-01-02T03:04:05.000Z"}')
})

test('stringify throws on circular structures', () => {
  const obj = {}
  obj.self = obj
  assert.throws(() => stringifyJSON(obj), /circular structure|cyclic structure/)
})

test('parse preserves negative zero', () => {
  const result = parseJSON('-0')
  assert.strictEqual(1 / result, -Infinity)
})

test('parse leaves non-integer large literals as numbers', () => {
  const result = parseJSON('[1e20]')
  assert.strictEqual(typeof result[0], 'number')
})

test('top-level bigint stringifies correctly', () => {
  const json = stringifyJSON(42n)
  assert.strictEqual(json, '42')
})
