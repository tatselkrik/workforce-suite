export class SafeError extends Error { constructor(status=503,kind="unavailable"){super(kind);this.status=status;this.kind=kind;} }
export async function responseJson(response,max=1000000){
 const declared=Number(response.headers.get("content-length")||0);if(declared>max)throw new SafeError();
 const reader=response.body?.getReader();if(!reader)throw new SafeError();let size=0;const parts=[];
 try{for(;;){const item=await reader.read();if(item.done)break;size+=item.value.length;if(size>max){await reader.cancel();throw new SafeError()}parts.push(item.value)}}finally{reader.releaseLock()}
 const bytes=new Uint8Array(size);let at=0;for(const p of parts){bytes.set(p,at);at+=p.length}
 try{return JSON.parse(new TextDecoder().decode(bytes))}catch{throw new SafeError()}
}
export const uuid=v=>typeof v==="string"&&/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v)&&v!=="00000000-0000-0000-0000-000000000000";
export const deviceToken=v=>typeof v==="string"&&v.length>=40&&v.length<=4096&&/^[A-Za-z0-9_:.-]+$/.test(v);
