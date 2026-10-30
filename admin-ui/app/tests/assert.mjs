/* The assertions the checks use, on Bun's own comparison and formatting
 * rather than node:assert. Same names and the same meaning as Node's, so a
 * check reads as before:
 *
 *   import assert from './assert.mjs';
 *   assert.equal(a, b, 'why');
 *
 * `equal` and `deepEqual` are strict (Object.is, Bun.deepEquals with strict
 * on), the way node:assert/strict has them. A failure throws an
 * AssertionError whose message names both sides. */

class AssertionError extends Error {
  constructor(message, actual, expected) {
    super(message);
    this.name = 'AssertionError';
    this.actual = actual;
    this.expected = expected;
  }
}

const show = (v) => Bun.inspect(v, { depth: 6 });

function fail(message, actual, expected, how) {
  if (message instanceof Error) throw message;
  throw new AssertionError(
    message ?? (how ? `${how}\n  actual:   ${show(actual)}\n  expected: ${show(expected)}` : 'failed'),
    actual,
    expected,
  );
}

function ok(value, message) {
  if (!value) fail(message ?? `expected a truthy value, got ${show(value)}`, value, true);
}

/* What `throws` and `rejects` accept as the expected error: a RegExp over the
 * message, an Error class, an object whose keys must match, or a function
 * that returns true for the right error. */
function matches(error, expected) {
  if (expected === undefined) return true;
  if (expected instanceof RegExp) return expected.test(String(error?.message ?? error));
  if (typeof expected === 'function') {
    if (expected.prototype !== undefined && error instanceof expected) return true;
    if (Error.isPrototypeOf(expected) || expected === Error) return false;
    return expected(error) === true;
  }
  return Object.entries(expected).every(([k, v]) =>
    v instanceof RegExp ? v.test(String(error?.[k])) : Bun.deepEquals(error?.[k], v, true),
  );
}

function threw(error, expected, message) {
  if (!matches(error, expected)) {
    fail(typeof expected === 'string' ? expected : message ?? `the wrong error: ${show(error)}`, error, expected);
  }
}

const assert = (value, message) => ok(value, message);

Object.assign(assert, {
  AssertionError,
  ok,
  fail: (message) => fail(message ?? 'failed'),
  equal(actual, expected, message) {
    if (!Object.is(actual, expected)) fail(message, actual, expected, 'not equal');
  },
  notEqual(actual, expected, message) {
    if (Object.is(actual, expected)) fail(message, actual, expected, 'equal, and should not be');
  },
  deepEqual(actual, expected, message) {
    if (!Bun.deepEquals(actual, expected, true)) fail(message, actual, expected, 'not deeply equal');
  },
  notDeepEqual(actual, expected, message) {
    if (Bun.deepEquals(actual, expected, true)) fail(message, actual, expected, 'deeply equal, and should not be');
  },
  match(string, pattern, message) {
    if (typeof string !== 'string' || !pattern.test(string)) fail(message, string, pattern, 'does not match');
  },
  doesNotMatch(string, pattern, message) {
    if (typeof string !== 'string' || pattern.test(string)) fail(message, string, pattern, 'matches, and should not');
  },
  throws(fn, expected, message) {
    try {
      fn();
    } catch (error) {
      threw(error, expected, message);
      return;
    }
    fail(typeof expected === 'string' ? expected : message ?? 'expected it to throw');
  },
  async rejects(promiseOrFn, expected, message) {
    try {
      await (typeof promiseOrFn === 'function' ? promiseOrFn() : promiseOrFn);
    } catch (error) {
      threw(error, expected, message);
      return;
    }
    fail(typeof expected === 'string' ? expected : message ?? 'expected it to reject');
  },
});
assert.strictEqual = assert.equal;
assert.notStrictEqual = assert.notEqual;
assert.deepStrictEqual = assert.deepEqual;
assert.strict = assert;

export default assert;
export { AssertionError };
