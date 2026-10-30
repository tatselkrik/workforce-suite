import test from 'node:test';
import assert from 'node:assert/strict';
import { inspectFile } from '../tools/public-source-check.mjs';
const inspect = (name, text, denied = []) => inspectFile(name, Buffer.from(text), denied);
test('operator profiles, private archives and signed builds cannot be tracked', () => {
  for (const name of ['deployment-private/key', 'assets/hris-connection.json', 'app/google-services.json', 'backup/data.bundle', 'dist/app.apk', 'nested/KEY.P12', 'settings/.env.production']) assert.ok(inspect(name, '{}').includes('operator_file'), name);
});
test('literal configuration examples and source fixtures remain publishable', () => {
  assert.deepEqual(inspect('config-examples/hris-connection.example.json', '{"publishable_key":"sb_publishable_REPLACE_ME"}'), []);
  assert.deepEqual(inspect('config-examples/.env.example', 'EXAMPLE=value'), []);
  assert.deepEqual(inspect('tests/connection.rs', 'https://fixture:fixture_password@example.com'), []);
});
test('actual key material and bearer-shaped secrets are detected without returning values', () => {
  const values = ['sb_secret_' + 'x'.repeat(40), 'ghp_' + 'x'.repeat(40), '-----BEGIN PRIVATE KEY-----\n' + 'a'.repeat(120) + '\n-----END PRIVATE KEY-----'];
  for (const value of values) { const findings = inspect('src/source.rs', value); assert.ok(findings.length); assert.ok(!JSON.stringify(findings).includes(value)); }
  const jwt = Buffer.from('{"alg":"HS256"}').toString('base64url') + '.' + Buffer.from('{"role":"service_role","issuer":"fixture only"}').toString('base64url') + '.' + 'x'.repeat(40);
  assert.ok(inspect('src/source.rs', jwt).includes('private_bearer'));
});
test('credential URLs cannot be hidden in non-reserved hosts', () => {
  const fixture = 'https://someone:long_password@';
  assert.ok(inspect('docs/setup.md', fixture + 'actual-host.net').includes('credential_url'));
  assert.ok(inspect('docs/setup.md', fixture + 'example.com.attacker.net').includes('credential_url'));
});
test('operator identifiers can be checked locally without embedding them in public rules', () => {
  const privateValue = 'local-fixture-' + 'x'.repeat(20);
  assert.deepEqual(inspect('src/config.rs', privateValue, [privateValue]), ['operator_identifier']);
});
