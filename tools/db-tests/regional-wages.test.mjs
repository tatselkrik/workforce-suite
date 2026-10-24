import{readFile,writeFile}from'node:fs/promises';
export async function runWageEngineTests({db,eq,deny,session,owner}){
 await db.exec('reset role');const file=new URL('../../tests/fixtures/regional-wage-vectors.json',import.meta.url);const vectors=JSON.parse(await readFile(file,'utf8'));
 eq((await db.query('select hris_private.regional_wage_catalog()c')).rows[0].c,JSON.parse(await readFile(new URL('../../src/data/ph-regional-wages-2026-10-24.json',import.meta.url),'utf8')),'Database/catalog exact source parity');
 for(const v of vectors){
  const c=(await db.query('select hris_private.regional_wage_compute($1)c',[v.source])).rows[0].c;
  for(const[k,val]of Object.entries(v.expected))eq(k==='statuses'?c.days.map(d=>d.status):c[k],val,v.label+' '+k);
  v.result=c;
 }
 for(const p of[{basis:null,rate_cents:55000,annual_divisor:365},{basis:'monthly',rate_cents:0,annual_divisor:365},{basis:'monthly',rate_cents:55000,annual_divisor:313}])await deny('select hris_private.regional_wage_compute($1)',[{...vectors[0].source,pay:p}],['23514']);
 await deny('select hris_private.regional_wage_compute($1)',[{...vectors[0].source,starts_on:'2027-01-01',ends_on:'2027-01-01'}],['23514']);
 const foreign=structuredClone(vectors[0].source);foreign.classifications[0].company_id='00000000-0000-4000-8000-000000399999';await deny('select hris_private.regional_wage_compute($1)',[foreign],['23514']);
 await session(owner);await deny('select hris_private.regional_wage_catalog()',[],['42501']);await deny('select hris_private.regional_wage_compute($1)',[vectors[0].source],['42501']);
 await db.exec('reset role');await writeFile(file,JSON.stringify(vectors,null,2)+'\n');
}
