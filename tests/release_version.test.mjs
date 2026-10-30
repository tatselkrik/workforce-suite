import test from 'node:test';
import assert from 'node:assert/strict';
import { validReleaseVersion } from '../tools/updates/version.mjs';

test('stable and beta artifact versions retain bounded canonical spelling', () => {
  for (const value of ['0.1.70', '1.0.0-beta.1', '1.0.0-beta.10', '1.0.0-rc.1', '1.0.0']) assert.equal(validReleaseVersion(value), true, value);
  for (const value of ['v1.0.0', '1.00.0', '1.0', '1000000.0.0', '1.0.0-beta.01', '1.0.0-beta..1', '1.0.0-beta/other', '1.0.0-beta%2fother', '1.0.0+unrouted', '1.0.0\n', '1.0.0\r\n', '1.0.0-'+'x'.repeat(80)]) assert.equal(validReleaseVersion(value), false, value);
  assert.equal(validReleaseVersion(null), false);
});
