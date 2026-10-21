// Local release signing. The private key is DPAPI-protected for the current Windows user.
// Never log unprotected key material. Artifacts contain only the public verification key.
import {generateKeyPairSync,createPrivateKey,createPublicKey,sign,createHash} from 'node:crypto';
import {readFile,writeFile,mkdir,stat} from 'node:fs/promises';
import {spawnSync} from 'node:child_process';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const appId='io.github.tatselkrik.hrisdesktop';
function protect(bytes,encrypt) {
  if(process.platform!=='win32') throw new Error('Local signing-key protection requires Windows.');
  const operation=encrypt?'Protect':'Unprotect';
  const script=`Add-Type -AssemblyName System.Security; $data=[Convert]::FromBase64String([Console]::In.ReadToEnd()); $out=[Security.Cryptography.ProtectedData]::${operation}($data,$null,[Security.Cryptography.DataProtectionScope]::CurrentUser); [Console]::Out.Write([Convert]::ToBase64String($out))`;
  const result=spawnSync(path.join(process.env.SystemRoot,'System32/WindowsPowerShell/v1.0/powershell.exe'),
    ['-NoProfile','-NonInteractive','-Command',script],{input:bytes.toString('base64'),encoding:'utf8',windowsHide:true});
  if(result.status!==0) throw new Error('Windows signing-key protection failed. No key material was printed.');
  return Buffer.from(result.stdout.trim(),'base64');
}
async function exists(file) {try {await stat(file);return true;} catch(error) {if(error.code==='ENOENT')return false;throw error;}}
function publicProfile(key) {
  const der=createPublicKey(key).export({format:'der',type:'spki'});
  if(der.length!==44 || der.subarray(0,12).toString('hex')!=='302a300506032b6570032100') throw new Error('Unsupported signing key.');
  return {public_key:der.subarray(12).toString('base64')};
}
export async function initialize(privateFolder) {
  await mkdir(privateFolder,{recursive:true});
  const file=path.join(privateFolder,'signing-key.dpapi');
  const profile=path.join(privateFolder,'hris-updates.json');
  if(!await exists(file)) {
    if(await exists(profile)) throw new Error('The signing key is missing. Preserve the current verification key and recover the signing key.');
    const pair=generateKeyPairSync('ed25519');
    const bytes=pair.privateKey.export({format:'der',type:'pkcs8'});
    try {await writeFile(file,protect(bytes,true),{flag:'wx'});} finally {bytes.fill(0);}
  }
  const bytes=protect(await readFile(file),false);
  try {
    const key=createPrivateKey({key:bytes,format:'der',type:'pkcs8'});
    const expected=publicProfile(key);
    if(await exists(profile)) {
      if(JSON.stringify(JSON.parse(await readFile(profile,'utf8')))!==JSON.stringify(expected)) throw new Error('The existing update verification key differs. No key was replaced.');
    } else {await writeFile(profile,JSON.stringify(expected,null,2)+'\n',{flag:'wx'});}
  } finally {bytes.fill(0);}
  return {public_profile:profile,protected_key:file};
}

export async function prepare(privateFolder,version,setupFile,exeFile,schema,notesFile) {
  if(!/^(0|[1-9][0-9]{0,5})\.(0|[1-9][0-9]{0,5})\.(0|[1-9][0-9]{0,5})$/.test(version) || !/^[0-9]{14}$/.test(schema)) throw new Error('Invalid release or database version.');
  await initialize(privateFolder);
  const setup=await readFile(setupFile),exe=await readFile(exeFile),notes=(await readFile(notesFile,'utf8')).trim();
  if(!setup.subarray(0,2).equals(Buffer.from('MZ')) || setup.length>64*1024*1024 || !setup.length || [...notes].length>1200) throw new Error('Unsupported installer or notes.');
  const release={application_id:appId,version,platform:'windows-x64',requires_schema:schema,
    artifact:`${version}/HRIS-Desktop-Update-${version}.exe`,sha256:hash(setup),executable_sha256:hash(exe),size:setup.length,notes};
  const bytes=protect(await readFile(path.join(privateFolder,'signing-key.dpapi')),false);
  let envelope;
  try {
    const key=createPrivateKey({key:bytes,format:'der',type:'pkcs8'});
    const payload=Buffer.from(JSON.stringify(release));
    envelope={payload:payload.toString('base64'),signature:sign(null,payload,key).toString('base64')};
  } finally {bytes.fill(0);}
  const manifest=path.join(privateFolder,`release-${version}.json`);
  if(await exists(manifest)) {
    if(JSON.stringify(JSON.parse(await readFile(manifest,'utf8')))!==JSON.stringify(envelope)) throw new Error('This release was already signed with different contents. Use a new version.');
  } else {await writeFile(manifest,JSON.stringify(envelope,null,2)+'\n',{flag:'wx'});}
  return {version,manifest,sha256:release.sha256,size:release.size};
}

if(process.argv[1] && path.resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  try {
    const [operation,...args]=process.argv.slice(2);
    const result=operation==='init'?await initialize(...args):operation==='prepare'?await prepare(...args):null;
    if(!result) throw new Error('Use init <private-folder> or prepare <private-folder> <version> <generic-setup> <exe> <schema-version> <notes-file>.');
    process.stdout.write(JSON.stringify(result)+'\n');
  } catch(error) {console.error(error.message);process.exitCode=1;}
}
