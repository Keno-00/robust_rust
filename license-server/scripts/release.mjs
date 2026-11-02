import {createHash,createPrivateKey,createPublicKey,generateKeyPairSync,sign,verify,createHmac} from 'node:crypto';
import {readFile,writeFile,mkdir,copyFile,stat,realpath} from 'node:fs/promises';
import {createReadStream} from 'node:fs';
import {resolve,dirname,join,extname} from 'node:path';
import {spawnSync} from 'node:child_process';
import {assertCanonicalWorkspace} from './canonical-workspace.mjs';

const LIMIT=2*1024*1024*1024;
const prefix=Buffer.concat([Buffer.from('VERUX-RELEASE-CATALOG-V1'),Buffer.from([10])]);
const sha=b=>createHash('sha256').update(b).digest('hex');
const fail=m=>{throw new Error(m)};
const hash=s=>typeof s==='string' && /^[a-f0-9]{64}$/.test(s);
const id=s=>typeof s==='string' && /^[a-z0-9][a-z0-9.-]{0,63}$/.test(s);
const text=(s,n)=>typeof s==='string' && s.length>0 && Buffer.byteLength(s)<=n && !/[\x00-\x1f\x7f]/.test(s);
const guid=s=>typeof s==='string' && /^\{[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}\}$/i.test(s);
function relative(s){
 return typeof s==='string' && s.length>0 && s.length<180 && s.split('/').every(p=>
 /^[A-Za-z0-9_.-]+$/.test(p) && p!=='.' && p!=='..' && !p.endsWith('.') &&
 !/^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])$/i.test(p.split('.')[0]));
}
function version(s){
 if(typeof s!=='string'||!/^\d{1,5}\.\d{1,5}\.\d{1,5}$/.test(s))fail('Invalid MSI version');
 const v=s.split('.').map(Number);if(v.join('.')!==s)fail('Use canonical version components without leading zeroes');
 if(v[0]>255||v[1]>255||v[2]>65535)fail('MSI version exceeds limits');
 return v;
}
function fields(o,names){
 if(!o||typeof o!=='object'||Array.isArray(o)||Object.keys(o).some(k=>!names.includes(k))||names.some(k=>!(k in o)))
 fail('Invalid or unknown fields: expected '+names.join(', '));
}
function asset(a,max){fields(a,['namespace','sha256','size']);if(!(id(a.namespace)||a.namespace==='_shell')||!hash(a.sha256)||!Number.isSafeInteger(a.size)||a.size<1||a.size>max)fail('Invalid artifact');}
export function validate(c){
 fields(c,['schema','revision','issued_at','expires_at','banner','products','launcher']);
 const now=Math.floor(Date.now()/1000);
 if(c.schema!==1||!Number.isSafeInteger(c.revision)||c.revision<1||!Number.isSafeInteger(c.issued_at)||
 !Number.isSafeInteger(c.expires_at)||c.issued_at>now+300||c.expires_at<=now||c.expires_at<=c.issued_at||
 c.expires_at-c.issued_at>604800||!Array.isArray(c.products)||c.products.length>64)fail('Invalid or expired catalog');
 asset(c.banner,8*1024*1024);if(c.banner.namespace!=='_shell')fail('Invalid brand namespace');
 const ids=new Set(),upgrades=new Set(),dirs=[],services=new Set();
 for(const p of [...c.products,...(c.launcher?[c.launcher]:[])]){
  fields(p,['id','title','requisites','thumbnail','banner','release',...('replacement_service' in p?['replacement_service']:[]),...('service_file' in p?['service_file']:[])]);
  if('service_file' in p&&(!p.replacement_service||!relative(p.service_file)||!p.release.files.some(f=>f.path===p.service_file)))fail('Invalid service file');
  if('replacement_service' in p&&(typeof p.replacement_service!=='string'||!/^[A-Za-z0-9_.-]{1,64}$/.test(p.replacement_service)))fail('Invalid replacement service');
  if(p.replacement_service){const service=p.replacement_service.toLowerCase();if(services.has(service))fail('Products cannot claim the same driver service');services.add(service);}
  if(!id(p.id)||ids.has(p.id)||!text(p.title,80)||!Array.isArray(p.requisites)||p.requisites.length>16||!p.requisites.every(s=>text(s,160)))fail('Invalid product');
  ids.add(p.id);asset(p.thumbnail,8*1024*1024);asset(p.banner,8*1024*1024);
  const r=p.release;
  fields(r,['version','package','product_code','upgrade_code','directory','executable','executable_sha256','files','min_windows_build','architecture']);
  version(r.version);asset(r.package,LIMIT);if([p.thumbnail,p.banner,r.package].some(a=>a.namespace!==p.id))fail('Cross-product artifact rejected');
  const directory=typeof r.directory==='string'?r.directory.toLowerCase():'';
  if(!guid(r.product_code)||!guid(r.upgrade_code)||upgrades.has(r.upgrade_code.toUpperCase())||!relative(r.directory)||
  dirs.some(d=>d===directory||d.startsWith(directory+'/')||directory.startsWith(d+'/'))||
  !relative(r.executable)||!r.executable.toLowerCase().endsWith('.exe')||!hash(r.executable_sha256)||
  !Number.isInteger(r.min_windows_build)||r.min_windows_build<17763||r.architecture!=='x64'||
  !Array.isArray(r.files)||r.files.length<1||r.files.length>512)fail('Invalid release contract');
  upgrades.add(r.upgrade_code.toUpperCase());dirs.push(directory);
  const paths=new Set();
  for(const f of r.files){
   fields(f,['path','sha256']);
   if(!relative(f.path)||!hash(f.sha256)||paths.has(f.path.toLowerCase()))fail('Invalid installed file');
   paths.add(f.path.toLowerCase());
  }
  if(!r.files.some(f=>f.path===r.executable&&f.sha256===r.executable_sha256))fail('Executable absent from file manifest');
 }
}
async function fileHash(path){const h=createHash('sha256');for await(const part of createReadStream(path))h.update(part);return h.digest('hex');}
function publicBytes(key){return Buffer.from(key.export({format:'jwk'}).x,'base64url');}
function privatePath(){return process.env.VERUX_RELEASE_PRIVATE_KEY_FILE||fail('Set VERUX_RELEASE_PRIVATE_KEY_FILE to your private key path');}
async function readPrivate(){
 const key=createPrivateKey(await readFile(privatePath()));
 if(key.asymmetricKeyType!=='ed25519')fail('Release key must be Ed25519');
 return key;
}
async function init(path){
 const absolute=resolve(path||fail('init requires a new private key path outside the repositories'));
 const {privateKey,publicKey}=generateKeyPairSync('ed25519');
 await mkdir(dirname(absolute),{recursive:true});
 await writeFile(absolute,privateKey.export({format:'pem',type:'pkcs8'}),{flag:'wx',mode:0o600});
 await writeFile(absolute+'.public',publicBytes(publicKey).toString('base64')+'\n',{flag:'wx'});
 console.log('Created private signing key and public key file. Restrict the private file ACL to the release operator; back it up securely.');
}
async function prepare(sourcePath,outPath){
 const source=JSON.parse((await readFile(sourcePath,'utf8')).trimStart());
 const base=dirname(resolve(sourcePath)),out=resolve(outPath);
 await mkdir(out,{recursive:false});
 await mkdir(join(out,'objects'));
 const baseReal=await realpath(base);
 async function sourceFile(file){
  if(!relative(file))fail('Use a relative source path inside the catalog folder');
  const path=await realpath(resolve(base,file));
  if(!path.toLowerCase().startsWith((baseReal+String.fromCharCode(92)).toLowerCase()))fail('Source path escapes catalog folder');
  return path;
 }
 async function add(file,kind,role,namespace){
  const path=await sourceFile(file),info=await stat(path);
  if(!info.isFile()||info.size<1||info.size>(kind==='png'?8*1024*1024:LIMIT))fail('Artifact size rejected: '+file);
  if(kind==='png'){
   const b=await readFile(path);
   if(b.length<33||b.subarray(0,8).toString('hex')!=='89504e470d0a1a0a')fail('Artwork must be PNG');
   const w=b.readUInt32BE(16),h=b.readUInt32BE(20);
   if(!w||!h||w>4096||h>4096||w*h>8000000)fail('Artwork dimensions rejected');
   if(role==='thumbnail'&&Math.abs(w/h-16/9)>0.01)fail('Thumbnails must be 16:9');
   if(role==='banner'&&(w/h<16/9||w/h>8))fail('Product banners must be 16:9 or wider');
   if(role==='brand'&&Math.abs(w/h-2560/500)>0.01)fail('Launcher banner must match the existing 2560:500 proportions');
  }
  const digest=await fileHash(path),key=namespace+'/'+digest+'.'+kind,target=join(out,'objects',key);
  await mkdir(dirname(target),{recursive:true});
  try{await copyFile(path,target,1);}catch(e){if(e.code!=='EEXIST')throw e;}
  return {namespace,sha256:digest,size:info.size};
 }
 const catalog={schema:1,revision:source.revision,issued_at:Math.floor(Date.now()/1000),
 expires_at:Math.floor(Date.now()/1000)+(source.ttl_seconds??86400),banner:await add(source.banner,'png','brand','_shell'),products:[],launcher:null};
 async function product(p,launcher=false){
  if(!id(p.id))fail('Invalid product identity');
  const r={...p.release};
  r.package=await add(r.package,'msi','package',p.id);
  if(!Array.isArray(r.files))fail('Provide the installed file list with local source paths');
  r.files=await Promise.all(r.files.map(async f=>({path:f.path,sha256:await fileHash(await sourceFile(f.source))})));
  const exe=r.files.find(f=>f.path===r.executable)||fail('Executable missing from files');
  r.executable_sha256=exe.sha256;
  return {...p,thumbnail:await add(p.thumbnail,'png',launcher?'launcher':'thumbnail',p.id),banner:await add(p.banner,'png','banner',p.id),release:r};
 }
 for(const p of source.products)catalog.products.push(await product(p));
 if(source.launcher)catalog.launcher=await product(source.launcher,true);
 validate(catalog);
 for(const p of [...catalog.products,...(catalog.launcher?[catalog.launcher]:[])]) {
  const inspector=process.env.VERUX_LAUNCHER_INSPECTOR||fail('Set VERUX_LAUNCHER_INSPECTOR to the built launcher executable');
  const expected=join(out,p.id+'.json');
  await writeFile(expected,JSON.stringify(p),{flag:'wx'});
  const result=spawnSync(inspector,['--inspect-msi',join(out,'objects',p.id,p.release.package.sha256+'.msi'),expected],{windowsHide:true,stdio:'inherit'});
  if(result.error||result.status!==0)fail('MSI identity inspection failed for '+p.id+'; inspect launcher.log');
 }
 const key=await readPrivate();
 const payload=Buffer.from(JSON.stringify(catalog));
 const envelope=Buffer.from(JSON.stringify({payload:payload.toString('base64'),signature:sign(null,Buffer.concat([prefix,payload]),key).toString('base64')}));
 if(envelope.length>1024*1024)fail('Catalog envelope exceeds client limit');
 await writeFile(join(out,'catalog.json'),envelope,{flag:'wx'});
 await writeFile(join(out,'catalog-readable.json'),JSON.stringify(catalog,null,2)+'\n',{flag:'wx'});
 await writeFile(join(out,'release-public-key.txt'),publicBytes(createPublicKey(key)).toString('base64')+'\n',{flag:'wx'});
 console.log('Prepared signed release '+catalog.revision+' with '+catalog.products.length+' products. Review catalog-readable.json before publishing.');
}
async function checkEnvelope(bytes,key){
 const envelope=JSON.parse(bytes),payload=Buffer.from(envelope.payload,'base64');
 fields(envelope,['payload','signature']);
 if(!verify(null,Buffer.concat([prefix,payload]),key,Buffer.from(envelope.signature,'base64')))fail('Release signature rejected');
 const catalog=JSON.parse(payload);validate(catalog);return catalog;
}
function credentials(){
 const env=process.env;
 for(const key of ['R2_ACCOUNT_ID','R2_BUCKET','R2_ACCESS_KEY_ID','R2_SECRET_ACCESS_KEY'])if(!env[key])fail('Missing '+key);
 if(!/^[a-f0-9]{32}$/.test(env.R2_ACCOUNT_ID)||!/^[a-z0-9][a-z0-9-]{1,62}$/.test(env.R2_BUCKET))fail('Invalid R2 destination');
 return env;
}
async function s3(method,key,body,headers={}){
 const e=credentials(),host=e.R2_ACCOUNT_ID+'.r2.cloudflarestorage.com';
 const path='/'+e.R2_BUCKET+'/'+key.split('/').map(encodeURIComponent).join('/');
 const date=new Date().toISOString().replace(/[:-]|\.\d{3}/g,''),day=date.slice(0,8);
 const payload=body?await fileHash(body):sha(Buffer.alloc(0));
 // Preserve strong object ETags: transparent gzip rewrites JSON ETags to weak validators.
 const hs={host,'accept-encoding':'identity','x-amz-content-sha256':payload,'x-amz-date':date,...headers};
 if(body)hs['content-length']=String((await stat(body)).size);
 const names=Object.keys(hs).sort(),signed=names.join(';');
 const canonical=[method,path,'',names.map(k=>k+':'+hs[k].trim()+'\n').join(''),signed,payload].join('\n');
 const scope=day+'/auto/s3/aws4_request';
 const hmac=(key,value)=>createHmac('sha256',key).update(value).digest();
 const signing=hmac(hmac(hmac(hmac('AWS4'+e.R2_SECRET_ACCESS_KEY,day),'auto'),'s3'),'aws4_request');
 const signature=hmac(signing,['AWS4-HMAC-SHA256',date,scope,sha(canonical)].join('\n')).toString('hex');
 hs.authorization='AWS4-HMAC-SHA256 Credential='+e.R2_ACCESS_KEY_ID+'/'+scope+', SignedHeaders='+signed+', Signature='+signature;
 return fetch('https://'+host+path,{method,headers:hs,body:body?createReadStream(body):undefined,duplex:'half',redirect:'error',signal:AbortSignal.timeout(600000)});
}
async function immutable(key,path){
 const response=await s3('PUT',key,path,{'if-none-match':'*'});
 if(response.status===412){
  const existing=await s3('GET',key);
  if(existing.status!==200)fail('Cannot verify existing immutable object: '+key);
  const h=createHash('sha256');let size=0;for await(const chunk of existing.body){h.update(chunk);size+=chunk.length;if(size>LIMIT)fail('Existing object exceeds limit');}
  if(h.digest('hex')!==await fileHash(path))fail('Immutable object collision: '+key);
 }else if(!response.ok)fail('Immutable upload failed ('+response.status+'): '+key);
 await response.body?.cancel();
}
async function publish(folder){
 const root=resolve(folder),key=createPublicKey(await readPrivate());
 const envelope=await readFile(join(root,'catalog.json'));
 const catalog=await checkEnvelope(envelope,key);
 const previous=await s3('GET','catalog/current.json');
 let etag=null;
 if(previous.status===200){
  etag=previous.headers.get('etag')||fail('Current catalog lacks ETag');
  if(etag.startsWith('W/'))fail('Current catalog has a weak ETag; inspect release transport encoding');
  let size=0;const chunks=[];
  for await(const part of previous.body){size+=part.length;if(size>1024*1024)fail('Existing catalog exceeds limit');chunks.push(part);}
  const bytes=Buffer.concat(chunks);
  // Previous catalogs can be expired; signature and revision still protect publication ordering.
  const old=JSON.parse(bytes),payload=Buffer.from(old.payload,'base64');
  if(!verify(null,Buffer.concat([prefix,payload]),key,Buffer.from(old.signature,'base64')))fail('Existing catalog uses another signing key; use the documented rotation procedure');
  const oldCatalog=JSON.parse(payload);
  if(oldCatalog.revision>=catalog.revision)fail('Publish a strictly higher catalog revision');
  for(const product of [...catalog.products,...(catalog.launcher?[catalog.launcher]:[])]){
   const prior=[...oldCatalog.products,...(oldCatalog.launcher?[oldCatalog.launcher]:[])].find(p=>p.id===product.id);
   if(prior){
    if(prior.release.upgrade_code!==product.release.upgrade_code||prior.release.directory!==product.release.directory)fail('Existing product MSI ownership changed; author an explicit migration before publication');
    const a=version(prior.release.version),b=version(product.release.version);
    if(a[0]>b[0]||(a[0]===b[0]&&a[1]>b[1])||(a[0]===b[0]&&a[1]===b[1]&&a[2]>b[2]))fail('Release downgrade rejected');
    if(prior.release.version===product.release.version&&prior.release.package.sha256!==product.release.package.sha256)fail('A released version cannot change package bytes; increment its version');
   }
  }
 }else if(previous.status!==404)fail('Read current catalog failed: '+previous.status);
 const objects=new Map();
 const add=(a,ext)=>objects.set(a.namespace+'/'+a.sha256+'.'+ext,a);
 add(catalog.banner,'png');
 for(const p of [...catalog.products,...(catalog.launcher?[catalog.launcher]:[])]){
  add(p.thumbnail,'png');add(p.banner,'png');add(p.release.package,'msi');
 }
 for(const [name,a] of objects){
  const path=join(root,'objects',name),s=await stat(path);
  if(s.size!==a.size||await fileHash(path)!==a.sha256)fail('Prepared artifact changed: '+name);
  await immutable('objects/'+name,path);
 }
 await immutable('catalog/history/'+catalog.revision+'.json',join(root,'catalog.json'));
 const result=await s3('PUT','catalog/current.json',join(root,'catalog.json'),etag?{'if-match':etag}:{'if-none-match':'*'});
 if(!result.ok)fail('Catalog commit rejected ('+result.status+'); prior ETag '+etag+', encoding '+previous.headers.get('content-encoding')+'. Inspect current revision before retrying.');
 await result.body?.cancel();
 console.log('Published catalog revision '+catalog.revision);
}
async function main(){
 const [command,a,b]=process.argv.slice(2);
 if(command==='prepare')assertCanonicalWorkspace(b||fail('New output directory required'));
 else if(command==='publish')assertCanonicalWorkspace(a||fail('Prepared release directory required'));
 else if(command==='renew')assertCanonicalWorkspace(a||fail('Current prepared release directory required'),b||fail('New output directory required'));
 else if(command==='publish-library')assertCanonicalWorkspace();
 if(command==='init')await init(a);
 else if(command==='prepare')await prepare(a||fail('Source JSON required'),b||fail('New output directory required'));
 else if(command==='publish')await publish(a||fail('Prepared release directory required'));
 else if(command==='renew')await renew(a||fail('Current prepared release directory required'),b||fail('New output directory required'));
 else if(command==='publish-library'){
  const file=resolve(a||fail('Library JSON required')),bytes=await readFile(file);
  if(bytes.length>65536)fail('Library metadata exceeds limit');
  const c=JSON.parse(bytes);fields(c,['schema','products','notices']);
  if(c.schema!==1||!Array.isArray(c.products)||!Array.isArray(c.notices)||c.products.length>65||c.notices.length>32)fail('Invalid library metadata');
  const ids=new Set(),dirs=new Set(),noticeIds=new Set();
  for(const p of c.products){
   fields(p,['id','access','purchase_url','directory','entropy']);
   if(!id(p.id)||ids.has(p.id)||!['free','paid'].includes(p.access))fail('Invalid library product');ids.add(p.id);
   if(p.access==='paid'){
    const url=new URL(p.purchase_url);
    if(url.protocol!=='https:'||url.username||url.password||url.hash||!/^\w[\w-]{0,63}$/.test(p.directory)||!relative(p.directory)||dirs.has(p.directory.toLowerCase())||!/^[a-z0-9-]{1,100}$/.test(p.entropy))fail('Invalid paid product metadata');
    dirs.add(p.directory.toLowerCase());
   }else if(p.purchase_url!==''||p.directory!==''||p.entropy!=='')fail('Free products must not have license storage');
  }
  if(!c.products.some(p=>p.id==='verux.launcher'&&p.access==='free'))fail('Launcher self-update must remain free');
  for(const n of c.notices){fields(n,['id','product_id','title','text']);if(!id(n.id)||noticeIds.has(n.id)||(n.product_id!==null&&!ids.has(n.product_id))||!text(n.title,320)||n.title.length>80||typeof n.text!=='string'||!n.text.trim()||n.text.length>4000||Buffer.byteLength(n.text)>16000||/[\x00-\x08\x0b\x0c\x0e-\x1f]/.test(n.text))fail('Invalid notice');noticeIds.add(n.id);}
  const old=await s3('GET','library/current.json');
  if(old.status!==200&&old.status!==404)fail('Read library failed');
  const etag=old.headers.get('etag');await old.body?.cancel();
  if(old.status===200&&!etag)fail('Library ETag missing');
  if(etag?.startsWith('W/'))fail('Library has a weak ETag; inspect release transport encoding');
  const result=await s3('PUT','library/current.json',file,etag?{'if-match':etag}:{'if-none-match':'*'});
  await result.body?.cancel();if(!result.ok)fail('Library publication rejected: '+result.status);
  console.log('Published library metadata ('+c.notices.length+' notices).');
 }
 else if(command==='validate'){
  const key=createPublicKey(await readPrivate());
  const c=await checkEnvelope(await readFile(a),key);console.log('VALID '+c.revision);
 }else fail('Usage: release.mjs init <private-key-path> | prepare <source.json> <new-folder> | validate <catalog.json> | publish <prepared-folder> | renew <current-prepared-folder> <new-folder>');
}
async function renew(folder,outFolder){
 const root=resolve(folder),out=resolve(outFolder),key=await readPrivate(),publicKey=createPublicKey(key);
 const envelope=await readFile(join(root,'catalog.json'));
 const remote=await s3('GET','catalog/current.json');
 if(!remote.ok)fail('Read current catalog failed: '+remote.status);
 let total=0;const chunks=[];
 for await(const chunk of remote.body){total+=chunk.length;if(total>1024*1024)fail('Published catalog exceeds size limit');chunks.push(chunk);}
 const current=Buffer.concat(chunks);
 if(!current.equals(envelope))fail('Prepared release is not the current published catalog; renew the latest release directory');
 const old=JSON.parse(envelope),payload=Buffer.from(old.payload,'base64');
 if(!verify(null,Buffer.concat([prefix,payload]),publicKey,Buffer.from(old.signature,'base64')))fail('Renewal signing key mismatch');
 const c=JSON.parse(payload),ttl=c.expires_at-c.issued_at;
 c.revision+=1;c.issued_at=Math.floor(Date.now()/1000);c.expires_at=c.issued_at+ttl;validate(c);
 const objects=new Map();const add=(asset,ext)=>objects.set(asset.namespace+'/'+asset.sha256+'.'+ext,asset);
 add(c.banner,'png');
 for(const p of [...c.products,...(c.launcher?[c.launcher]:[])]){add(p.thumbnail,'png');add(p.banner,'png');add(p.release.package,'msi');}
 await mkdir(out,{recursive:false});
 for(const [name,asset] of objects){
  const from=join(root,'objects',name),to=join(out,'objects',name);
  if((await stat(from)).size!==asset.size||await fileHash(from)!==asset.sha256)fail('Prepared artifact changed: '+name);
  await mkdir(dirname(to),{recursive:true});await copyFile(from,to,1);
 }
 const fresh=Buffer.from(JSON.stringify(c));
 await writeFile(join(out,'catalog.json'),JSON.stringify({payload:fresh.toString('base64'),signature:sign(null,Buffer.concat([prefix,fresh]),key).toString('base64')}),{flag:'wx'});
 await writeFile(join(out,'catalog-readable.json'),JSON.stringify(c,null,2)+'\n',{flag:'wx'});
 await writeFile(join(out,'release-public-key.txt'),publicBytes(publicKey).toString('base64')+'\n',{flag:'wx'});
 await publish(out);
 console.log('Renewed through '+new Date(c.expires_at*1000).toISOString());
}
main().catch(error=>{console.error(error.message);process.exitCode=1;});




