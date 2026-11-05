/* The assertions the checks use, on Bun's own comparison and formatting
 * rather than node:assert. Same names and the same meaning as Node's, so a
 * check reads as before:
 *
 *   import assert from './assert.ts';
 *   assert.equal(a, b, 'why');
 *
 * `equal` and `deepEqual` are strict (Object.is, Bun.deepEquals with strict
 * on), the way node:assert/strict has them. A failure throws an
 * AssertionError whose message names both sides. */

export class AssertionError extends Error {
  readonly actual: unknown;
  readonly expected: unknown;

  constructor(message: string, actual: unknown, expected: unknown) {
    super(message);
    this.name = 'AssertionError';
    this.actual = actual;
    this.expected = expected;
  }
}

const show = (v: unknown): string => Bun.inspect(v, { depth: 6 });

function fail(message: unknown, actual?: unknown, expected?: unknown, how?: string): never {
  if (message instanceof Error) throw message;
  throw new AssertionError(
    (message as string | undefined) ??
      (how ? `${how}\n  actual:   ${show(actual)}\n  expected: ${show(expected)}` : 'failed'),
    actual,
    expected,
  );
}

function ok(value: unknown, message?: string): void {
  if (!value) fail(message ?? `expected a truthy value, got ${show(value)}`, value, true);
}

/* What `throws` and `rejects` accept as the expected error: a RegExp over the
 * message, an Error class, an object whose keys must match, or a function
 * that returns true for the right error. */
type ExpectError = string | RegExp | object | ((error: unknown) => boolean);

interface ErrorConstructorLike {
  new (...args: never[]): unknown;
  prototype?: unknown;
}

function matches(error: unknown, expected?: ExpectError): boolean {
  if (expected === undefined) return true;
  if (expected instanceof RegExp) return expected.test(String((error as Error | undefined)?.message ?? error));
  if (typeof expected === 'function') {
    const ctor = expected as ErrorConstructorLike & ((error: unknown) => boolean);
    if (ctor.prototype !== undefined && error instanceof ctor) return true;
    if (Error.isPrototypeOf(ctor as unknown as object) || (ctor as unknown) === Error) return false;
    return expected(error) === true;
  }
  return Object.entries(expected as object).every(([k, v]) =>
    v instanceof RegExp ? v.test(String((error as Record<string, unknown> | undefined)?.[k]))
      : Bun.deepEquals((error as Record<string, unknown> | undefined)?.[k], v, true),
  );
}

function threw(error: unknown, expected?: ExpectError, message?: string): void {
  if (!matches(error, expected)) {
    fail(typeof expected === 'string' ? expected : message ?? `the wrong error: ${show(error)}`, error, expected);
  }
}

const assert = (value: unknown, message?: string): void => ok(value, message);

interface AssertMembers {
  AssertionError: typeof AssertionError;
  strict: Assert;
  strictEqual: Assert['equal'];
  notStrictEqual: Assert['notEqual'];
  deepStrictEqual: Assert['deepEqual'];
  ok(value: unknown, message?: string): void;
  fail(message?: string): never;
  equal(actual: unknown, expected: unknown, message?: string): void;
  notEqual(actual: unknown, expected: unknown, message?: string): void;
  deepEqual(actual: unknown, expected: unknown, message?: string): void;
  notDeepEqual(actual: unknown, expected: unknown, message?: string): void;
  match(string: unknown, pattern: RegExp, message?: string): void;
  doesNotMatch(string: unknown, pattern: RegExp, message?: string): void;
  throws(fn: () => unknown, expected?: ExpectError, message?: string): void;
  rejects(promiseOrFn: unknown, expected?: ExpectError, message?: string): Promise<void>;
}
type Assert = typeof assert & AssertMembers;

Object.assign<Assert, AssertMembers>(assert as Assert, {
  AssertionError,
  ok,
  fail: (message: string | undefined) => fail(message ?? 'failed', undefined, undefined),
  equal(actual: unknown, expected: unknown, message?: string): void {
    if (!Object.is(actual, expected)) fail(message, actual, expected, 'not equal');
  },
  notEqual(actual: unknown, expected: unknown, message?: string): void {
    if (Object.is(actual, expected)) fail(message, actual, expected, 'equal, and should not be');
  },
  deepEqual(actual: unknown, expected: unknown, message?: string): void {
    if (!Bun.deepEquals(actual, expected, true)) fail(message, actual, expected, 'not deeply equal');
  },
  notDeepEqual(actual: unknown, expected: unknown, message?: string): void {
    if (Bun.deepEquals(actual, expected, true)) fail(message, actual, expected, 'deeply equal, and should not be');
  },
  match(string: unknown, pattern: RegExp, message?: string): void {
    if (typeof string !== 'string' || !pattern.test(string)) fail(message, string, pattern, 'does not match');
  },
  doesNotMatch(string: unknown, pattern: RegExp, message?: string): void {
    if (typeof string === 'string' && pattern.test(string)) fail(message, string, pattern, 'matches, and should not');
  },
  throws(fn: () => unknown, expected?: ExpectError, message?: string): void {
    try {
      fn();
    } catch (error) {
      threw(error, expected, message);
      return;
    }
    fail(typeof expected === 'string' ? expected : message ?? 'expected it to throw');
  },
  async rejects(promiseOrFn: unknown, expected?: ExpectError, message?: string): Promise<void> {
    try {
      await (typeof promiseOrFn === 'function' ? (promiseOrFn as () => unknown)() : promiseOrFn);
    } catch (error) {
      threw(error, expected, message);
      return;
    }
    fail(typeof expected === 'string' ? expected : message ?? 'expected it to reject');
  },
  strict: null as never,
  strictEqual: null as never,
  notStrictEqual: null as never,
  deepStrictEqual: null as never,
});
const typed = assert as Assert;
typed.strict = typed;
typed.strictEqual = typed.equal;
typed.notStrictEqual = typed.notEqual;
typed.deepStrictEqual = typed.deepEqual;

export default typed;
