// Isolated deterministic helper verification. No ledger, RPC or hosted writes.
import assert from'node:assert/strict';import{readFile}from'node:fs/promises';import{PGlite}from'@electric-sql/pglite';
const db=new PGlite();let checks=0;const eq=(a,b)=>{assert.deepEqual(a,b);checks++;};
const id=n=>`af490000-0000-4000-8000-${String(n).padStart(12,'0')}`;
const booking=(n,units,early,approved=false)=>({request_id:id(n),units,early_units:early,approved});
const anchor={id:id(100),employee_id:id(101),type_code:'annual',effective_on:'2027-02-01',carry_expires_on:'2027-03-31',base_earned_at_opening:30,target_available_units:16,remaining_carry_units:4,previously_consumed_carry_units:2,baseline:[booking(1,6,6)]};
const project=async(o=anchor,asof='2027-03-01',base=30,rows=o.baseline)=>(await db.query('select hris_private.opening_project($1::jsonb,$2::date,$3::int,$4::jsonb)r',[JSON.stringify(o),asof,base,JSON.stringify(rows)])).rows[0].r;
const no=async o=>{try{await project(o);}catch(e){assert.equal(e.code,'23514');checks++;return;}assert.fail('Invalid input accepted');};
try{
 await db.exec('create role anon nologin;create role authenticated nologin;create schema hris_private;');
 await db.exec(await readFile(new URL('../../supabase/migrations/20261025143356_leave_opening_arithmetic.sql',import.meta.url),'utf8'));
 let p=await project();eq(p.balance.available_units,16);eq(p.unused_base_units,12);eq(p.unspent_carry_units,4);eq((await project(anchor,'2027-04-01')).balance.available_units,12);
 p=await project(anchor,'2027-03-01',30,[]);eq(p.balance.available_units,22);eq(p.unused_base_units,16);eq(p.unspent_carry_units,6);eq((await project(anchor,'2027-04-01',30,[])).balance.available_units,16);
 p=await project(anchor,'2027-03-01',30,[booking(2,5,5)]);eq(p.balance.available_units,17);eq(p.unused_base_units,16);eq(p.unspent_carry_units,1);
 await no({...anchor,remaining_carry_units:2147483647});await no({...anchor,remaining_carry_units:null});await no({...anchor,target_available_units:-1});await no({...anchor,carry_expires_on:'2027-03-15'});await no({...anchor,unexpected:true});await no({...anchor,baseline:[booking(1,6,6),booking(1,6,6)]});await no({...anchor,baseline:[{...booking(1,6,6),approved:null}]});await no({...anchor,baseline:[1]});
 for(let carry=0;carry<=6;carry++)for(let imported=0;imported<=6;imported++)for(let early=0;early<=4;early++)for(const keep of[true,false]){
  const o={...anchor,baseline:[booking(1,4,early)],remaining_carry_units:imported,previously_consumed_carry_units:Math.min(carry,early)};const current=[booking(2,3,2,true),...(keep?o.baseline:[])];
  p=await project(o,'2027-03-01',32,current);eq(p.balance.available_units,16+2+4-(keep?4:0)-3);const after=await project(o,'2027-04-01',32,current);eq(after.balance.available_units,after.unused_base_units);eq(after.unspent_carry_units,0);
 }
 await db.exec('set role authenticated');try{await project();assert.fail('Private helper exposed');}catch(e){assert.equal(e.code,'42501');checks++;}
 console.log('Passed '+checks+' isolated PostgreSQL opening arithmetic assertions; no hosted deployment.');
}finally{await db.close();}
