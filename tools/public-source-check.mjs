import { execFileSync } from 'node:child_process';
import { readFileSync, lstatSync, existsSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

// Findings contain categories and paths, never matched credentials or source lines.
export function inspectFile(name, bytes, denyValues = []) {
  const normalized = name.replaceAll('\\', '/').toLowerCase();
  const findings = [];
  if (/(^|\/)(deployment-private|work|dist|target|node_modules|\.git)\//.test(normalized)
      || /(^|\/)(hris-connection\.json|hris-updates\.json|google-services\.json|company-logo\.png|memory\.md|\.env(?:\.(?!example$)[^/]*)?)$/.test(normalized)
      || /\.(dpapi|p12|jks|keystore|apk|exe|bundle)$/.test(normalized)) findings.push('operator_file');
  const text = bytes.toString('utf8');
  const patterns = {
    private_key: /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----\r?\n[A-Za-z0-9+/=\r\n]{100,}-----END (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/,
    server_key: /sb_secret_[A-Za-z0-9_-]{30,}/,
    github_token: /(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,})/,
    google_server_key: /"private_key"\s*:\s*"-----BEGIN [A-Z ]*PRIVATE KEY-----\\n[A-Za-z0-9+/=\\n]{100,}/,
  };
  for (const [category, pattern] of Object.entries(patterns)) if (pattern.test(text)) findings.push(category);
  for (const match of text.matchAll(/eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/g)) {
    try {
      const payload = JSON.parse(Buffer.from(match[0].split('.')[1], 'base64url'));
      if (['authenticated', 'service_role'].includes(payload.role)) findings.push('private_bearer');
    } catch { /* Non-token parser examples. */ }
  }
  for (const candidate of text.matchAll(/https:\/\/[^\s/"':@]{3,}:[^\s/"'@]{8,}@[^\s/"']+/g)) {
    try {
      const host = new URL(candidate[0]).hostname;
      if (!(host === 'example.com' || host.endsWith('.example.com') || host.endsWith('.invalid') || host.endsWith('.test'))) findings.push('credential_url');
    } catch { findings.push('credential_url'); }
  }
  if (denyValues.some(value => value && bytes.includes(Buffer.from(value)))) findings.push('operator_identifier');
  return [...new Set(findings)];
}

export function scan(root, history = false, denyValues = []) {
  const git = (...args) => execFileSync('git', args, { cwd: root, maxBuffer: 160 * 1024 * 1024 });
  const names = git('ls-files', '-z').toString('utf8').split('\0').filter(Boolean);
  const findings = [];
  for (const name of names) {
    const file = path.resolve(root, name);
    if (!existsSync(file)) { findings.push({ path: name, categories: ['uncommitted_deletion'] }); continue; }
    if (!file.startsWith(path.resolve(root) + path.sep) || lstatSync(file).isSymbolicLink()) {
      findings.push({ path: name, categories: ['unsafe_source_path'] }); continue;
    }
    const categories = inspectFile(name, readFileSync(file), denyValues);
    if (categories.length) findings.push({ path: name, categories });
  }
  let blobs = 0;
  if (history) {
    const rows = git('rev-list', '--objects', '--all').toString('utf8').trim().split('\n');
    const objects = new Map(rows.map(row => { const at = row.indexOf(' '); return at < 0 ? [row, ''] : [row.slice(0, at), row.slice(at + 1)]; }));
    const metadata = execFileSync('git', ['cat-file', '--batch-check=%(objectname) %(objecttype) %(objectsize)'], { cwd: root, input: [...objects.keys()].join('\n') + '\n' }).toString('utf8').trim().split('\n');
    const ids = metadata.filter(row => row.split(' ')[1] === 'blob').map(row => row.split(' ')[0]);
    const content = execFileSync('git', ['cat-file', '--batch'], { cwd: root, input: ids.join('\n') + '\n', maxBuffer: 160 * 1024 * 1024 });
    let offset = 0;
    for (const id of ids) {
      const end = content.indexOf(10, offset); const size = Number(content.subarray(offset, end).toString().split(' ')[2]);
      const bytes = content.subarray(end + 1, end + 1 + size); offset = end + 2 + size;
      const name = objects.get(id); const categories = inspectFile(name, bytes, denyValues);
      if (categories.length) findings.push({ path: name, blob: id, categories });
      blobs++;
    }
  }
  return { passed: findings.length === 0, trackedFiles: names.length, historicalBlobs: blobs, findings };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const result = scan(process.cwd(), process.argv.includes('--history'));
  console.log(JSON.stringify(result, null, 2));
  if (!result.passed) process.exitCode = 1;
}
