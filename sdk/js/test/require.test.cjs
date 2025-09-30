const test = require('node:test')
const assert = require('node:assert/strict')

const fostromModule = require('../index.js')

// When requiring an ES module via module-sync, Node returns
// the module namespace. Ensure the default export and named
// exports are reachable.
test('require() exposes default export', () => {
  assert.ok(fostromModule.default, 'default export should exist')
  assert.equal(typeof fostromModule.default, 'function', 'default export should be constructible')
})

test('require() exposes Mail named export', () => {
  assert.ok(fostromModule.Mail, 'Mail named export should exist')
})
