// Publisher authentication and signature validation are independent. Never log keys or bodies.
const appId = 'io.github.tatselkrik.hrisdesktop';
const maxBytes = 64 * 1024 * 1024;
const b64 = value => Uint8Array.from(atob(value), c => c.charCodeAt(0));
const hex = bytes => [...new Uint8Array(bytes)].map(b=>b.toString(16).padStart(2,'0')).join('');
const response = (status,message,extra={}) => Response.json({message,...extra},{status,
  headers:{'Cache-Control':'no-store','X-Content-Type-Options':'nosniff'}});

async function manifest(header,publicKey,cryptoApi) {
  if (!header || header.length>16000) throw new Error();
  const envelope=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(b64(header)));
  if (!envelope || Object.keys(envelope).sort().join(',')!=='payload,signature' ||
      typeof envelope.payload!=='string' || envelope.payload.length>12000 ||
      typeof envelope.signature!=='string' || envelope.signature.length>128) throw new Error();
  const payload=b64(envelope.payload);
  const key=await cryptoApi.subtle.importKey('raw',b64(publicKey),{name:'Ed25519'},false,['verify']);
  if (!await cryptoApi.subtle.verify('Ed25519',key,b64(envelope.signature),payload)) throw new Error();
  const data=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(payload));
  if (data.application_id!==appId || data.platform!=='windows-x64' ||
      !/^(0|[1-9][0-9]{0,5})\.(0|[1-9][0-9]{0,5})\.(0|[1-9][0-9]{0,5})$/.test(data.version) ||
      data.artifact!==`${data.version}/HRIS-Desktop-Update-${data.version}.exe` ||
      !/^[0-9]{14}$/.test(data.requires_schema) || !/^[0-9a-f]{64}$/.test(data.sha256) ||
      !/^[0-9a-f]{64}$/.test(data.executable_sha256) || !Number.isSafeInteger(data.size) ||
      data.size<=0 || data.size>maxBytes || typeof data.notes!=='string' || [...data.notes].length>1200) throw new Error();
  return {envelope,data};
}

export function createReleaseHandler({url,serviceKey,anonKey,publicKey},fetcher=fetch,cryptoApi=crypto) {
  return async request => {
    if (request.method!=='POST') return response(405,'Use POST.');
    if (request.headers.has('Origin')) return response(403,'Only authorized deployment tools can publish updates.');
    if (!url?.startsWith('https://') || !serviceKey || !anonKey || !publicKey) return response(503,'Update publishing is not configured.');
    const authorization=request.headers.get('Authorization')||'';
    if (authorization.length>8192 || !/^Bearer [^\s]+$/.test(authorization)) return response(401,'Sign in before publishing.');
    if (request.headers.get('Content-Type')!=='application/octet-stream') return response(415,'Use a binary update package.');
    const call=(path,method='GET',body,admin=false,binary=false)=>fetcher(url.replace(/\/$/,'')+path,{
      method,redirect:'error',signal:AbortSignal.timeout(30000),headers:{
        apikey:admin?serviceKey:anonKey,Authorization:admin?'Bearer '+serviceKey:authorization,
        'Content-Type':binary?'application/octet-stream':'application/json','x-upsert':'false',
      },body:body===undefined?undefined:binary?body:JSON.stringify(body),
    });
    try {
      const identityResponse=await call('/auth/v1/user');
      if (!identityResponse.ok) return response(401,'The publisher session is invalid.');
      const identity=await identityResponse.json();
      if(typeof identity.id!=='string' || !/^[0-9a-f]{8}-[0-9a-f-]{27}$/i.test(identity.id)) return response(401,'The publisher identity is invalid.');
      const accessResponse=await call('/rest/v1/rpc/my_hris_access','POST',{});
      if (!accessResponse.ok) return response(403,'Administrator access is required.');
      const access=await accessResponse.json();
      if (access.user_id!==identity.id || access.role!=='administrator') return response(403,'Administrator access is required.');
      let signed;
      try { signed=await manifest(request.headers.get('X-HRIS-Manifest'),publicKey,cryptoApi); }
      catch { return response(400,'The update signature or manifest is invalid.'); }
      const {envelope,data}=signed;
      const feedResponse=await call('/rest/v1/rpc/desktop_update','POST',{});
      if (!feedResponse.ok) return response(403,'Current deployment access could not be verified.');
      const feed=await feedResponse.json();
      if (!/^[0-9]{14}$/.test(feed.schema_version) || data.requires_schema>feed.schema_version) return response(409,'Apply and verify the required database migration before publishing.');
      const reader=request.body?.getReader();
      if (!reader) return response(400,'An update package is required.');
      const chunks=[];let length=0;
      while (true) {
        const {value,done}=await reader.read();if(done) break;
        length+=value.byteLength;
        if (length>data.size || length>maxBytes) {await reader.cancel();return response(413,'The package exceeds its signed size.');}
        chunks.push(value);
      }
      if (length!==data.size) return response(400,'The package download is incomplete.');
      const bytes=new Uint8Array(length);let offset=0;
      for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.byteLength;}
      if (bytes[0]!==77 || bytes[1]!==90 || hex(await cryptoApi.subtle.digest('SHA-256',bytes))!==data.sha256) return response(400,'The package failed its integrity check.');
      const catalogPath='/rest/v1/desktop_releases?version=eq.'+data.version;
      const existing=await call(catalogPath+'&select=envelope,enabled','GET',undefined,true);
      if(!existing.ok) return response(502,'The release catalog is unavailable.');
      const rows=await existing.json();
      if(rows.length && (rows[0].envelope.payload!==envelope.payload || rows[0].envelope.signature!==envelope.signature)) return response(409,'This version already exists with different contents. Publish a new version.');
      if(rows.length && rows[0].enabled) return response(200,'This exact update is already published.',{version:data.version,already_published:true});
      if(!rows.length) {
        const added=await call('/rest/v1/desktop_releases','POST',{version:data.version,platform:data.platform,
          requires_schema:data.requires_schema,envelope,enabled:false},true);
        if(!added.ok) return response(409,'The version could not be reserved. Refresh before retrying.');
      }
      const object='/storage/v1/object/desktop-updates/'+data.artifact;
      const uploaded=await call(object,'POST',bytes,true,true);
      if(!uploaded.ok) {
        if(uploaded.status!==409) return response(502,'The package could not be stored. The release remains disabled.');
        const prior=await call(object,'GET',undefined,true);
        if(!prior.ok || hex(await cryptoApi.subtle.digest('SHA-256',await prior.arrayBuffer()))!==data.sha256) return response(409,'The version object already exists with different contents.');
      }
      const enabled=await call(catalogPath,'PATCH',{enabled:true},true);
      if(!enabled.ok) return response(502,'The package is stored; the release remains disabled. Retry publishing this exact package.');
      return response(201,'Signed update published.',{version:data.version,published:true});
    } catch { return response(502,'Update publication could not finish. Retry the exact package.'); }
  };
}
