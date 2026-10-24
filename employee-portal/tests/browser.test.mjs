import { readFile } from 'node:fs/promises';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import test from 'node:test';

test('downloaded CSV cells preserve text and neutralize spreadsheet formulas', async () => {
  const source = await readFile(new URL('../web/app.js', import.meta.url), 'utf8');
  const start = source.indexOf('function csvCell(');
  const end = source.indexOf('function exportSlip(');
  assert(start >= 0 && end > start);
  const context = vm.createContext({});
  vm.runInContext(source.slice(start, end), context);
  for (const value of ['=1+1', '+CMD', '-1+2', '@SUM(A1)', '  =1', '\n=1', '\uFEFF=1']) {
    assert(context.csvCell(value).startsWith('"\''));
  }
  assert.equal(context.csvCell('Peña, Jr.'), '"Peña, Jr."');
  assert.equal(context.csvCell('a"b'), '"a""b"');
  assert.equal(context.csvCell('2026-09-01'), '"2026-09-01"');
});

test('portal HTML preserves UTF-8 punctuation after Windows source edits',async()=>{
 const html=await readFile(new URL('../web/index.html',import.meta.url),'utf8');
 assert(html.includes('HRIS · Employee portal'));assert(html.includes('View my details →'));
 assert(!html.includes('\u00c2\u00b7'));assert(!html.includes('\u00e2\u2020'));
});
