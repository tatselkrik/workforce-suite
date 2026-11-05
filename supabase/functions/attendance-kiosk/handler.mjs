const MAX_PHOTO=65536,MAX_BODY=100000;
const reply=(status,value)=>new Response(JSON.stringify(value),{status,headers:{'content-type':'application/json','cache-control':'no-store'}});
export function jpegDimensions(bytes){
 if(bytes.length<100||bytes.length>MAX_PHOTO||bytes[0]!==255||bytes[1]!==216||bytes.at(-2)!==255||bytes.at(-1)!==217)throw Error('Capture a fresh JPEG photo.');
 let p=2;
 while(p+4<bytes.length){
  if(bytes[p++]!==255)throw Error('Invalid camera image.');while(bytes[p]===255)p++;
  const marker=bytes[p++];if(marker===218||marker===217)break;
  if(marker===1||(marker>=208&&marker<=215))continue;
  const size=bytes[p]*256+bytes[p+1];if(size<2||p+size>bytes.length)throw Error('Invalid camera image.');
  if([192,193,194].includes(marker)){
   const height=bytes[p+3]*256+bytes[p+4],width=bytes[p+5]*256+bytes[p+6];
   if(size<8||bytes[p+2]!==8||width<80||height<80||width>640||height>640)throw Error('Use a camera image between 80 and 640 pixels per side.');
   return {width,height};
  }
  p+=size;
 }
 throw Error('Camera image dimensions are missing.');
}
async function boundedJson(request){
 if(Number(request.headers.get('content-length')||0)>MAX_BODY)throw Error('Camera request is too large.');
 const reader=request.body?.getReader();if(!reader)throw Error('Attendance request required.');
 let size=0;const chunks=[];
 try{for(;;){const {done,value}=await reader.read();if(done)break;size+=value.byteLength;if(size>MAX_BODY){await reader.cancel();throw Error('Camera request is too large.');}chunks.push(value);}}
 finally{reader.releaseLock();}
 const bytes=new Uint8Array(size);let at=0;for(const chunk of chunks){bytes.set(chunk,at);at+=chunk.length;}
 return JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes));
}
export function createHandler({rpc,upload}){
 return async request=>{
  if(request.method!=='POST'||request.headers.has('origin'))return reply(405,{error:'Use the registered native attendance station.'});
  let body,bytes,hash;
  try{
   body=await boundedJson(request);
   if(body?.operation==='close'){
    if(Object.keys(body).sort().join(',')!=='operation,token'||!/^[a-f0-9]{64}$/.test(body.token))throw Error('Invalid station close request.');
    try{await rpc('kiosk_station_close',{p_token:body.token});return reply(200,{closed:true});}
    catch{return reply(503,{error:'Station close could not be confirmed. Its grant will expire automatically.'});}
   }
   if(!body||typeof body!=='object'||Array.isArray(body)||Object.keys(body).sort().join(',')!=='employee_code,operation,photo,pin,request_id,token')throw Error('Use a complete station punch request.');
   if(!/^[a-f0-9]{64}$/.test(body.token)||!/^[0-9]{6,12}$/.test(body.pin)||typeof body.employee_code!=='string'||body.employee_code.length<1||body.employee_code.length>80
      ||!['check_in','check_out'].includes(body.operation)||!/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/i.test(body.request_id)||typeof body.photo!=='string'||body.photo.length>87384||!/^[A-Za-z0-9+/]*={0,2}$/.test(body.photo))throw Error('Check the employee code, PIN and attendance action.');
   bytes=Uint8Array.from(atob(body.photo),c=>c.charCodeAt(0));jpegDimensions(bytes);
   hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),b=>b.toString(16).padStart(2,'0')).join('');
  }catch(e){return reply(400,{error:e instanceof SyntaxError?'Invalid attendance request.':e.message||'Invalid attendance request.'});}
  try{
   const prepared=await rpc('kiosk_punch_prepare',{p_request:{token:body.token,employee_code:body.employee_code,pin:body.pin,request_id:body.request_id,operation:body.operation,photo_hash:hash,photo_bytes:bytes.length}});
   if(!prepared?.accepted)return reply(prepared?.error?.startsWith('Station is locked')?403:400,{error:prepared?.error||'Attendance was not accepted.'});
   if(prepared.result)return reply(200,prepared.result);
   if(typeof prepared.object_path!=='string'||!/^[a-f0-9-]{36}\/[a-f0-9-]{36}\.jpg$/i.test(prepared.object_path))throw Error('Invalid photo destination.');
   await upload(prepared.object_path,bytes);
   const saved=await rpc('kiosk_punch_complete',{p_token:body.token,p_id:body.request_id});
   if(saved?.saved!==true||saved.request_id!==body.request_id)throw Error('Invalid saved attendance receipt.');
   return reply(200,saved);
  }catch(e){
   if(e?.definitive===true&&[400,403,409].includes(e.status))return reply(e.status,{error:e.message});
   return reply(503,{error:'Attendance delivery is uncertain. Keep this capture and retry the same punch; do not enter a replacement.'});
  }
 };
}
