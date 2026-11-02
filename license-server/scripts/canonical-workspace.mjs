// Deploys, catalog preparation and publication run only from the canonical
// checkout; see AGENTS.md "Workspace and sandbox rule". Run directly by the
// Wrangler [build] step, and imported by release.mjs.
import {existsSync,realpathSync,statSync} from 'node:fs';
import {join,resolve,sep} from 'node:path';
import {fileURLToPath} from 'node:url';

const ROOT=process.env.VERUX_SOURCE_ROOT||'C:\\Users\\Admin\\Documents\\source_repos';
const REPO=join(ROOT,'robust_rust');
const real=p=>realpathSync.native(p).toLowerCase();
const fail=m=>{throw new Error('Workspace rule: '+m)};

export function assertCanonicalWorkspace(...outputs){
  const here=fileURLToPath(new URL('../..',import.meta.url));
  if(!existsSync(REPO)||real(here)!==real(REPO))fail(`run from ${REPO}, not ${here}. Move the work into the main repo instead of using a copy or sandbox.`);
  if(!statSync(join(here,'.git'),{throwIfNoEntry:false})?.isDirectory())fail(`${here} is a git worktree or has no repository; use the main checkout.`);
  const target=real(join(REPO,'target'))+sep;
  for(const output of outputs){
    const full=resolve(output);
    const path=existsSync(full)?real(full):full.toLowerCase();
    if(!path.startsWith(target))fail(`release output ${full} must be under ${join(REPO,'target')}.`);
  }
}

if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url)){
  try{assertCanonicalWorkspace();}catch(error){console.error(error.message);process.exit(1);}
}
