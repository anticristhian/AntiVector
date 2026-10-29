import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';

// cargo metadata is authoritative for the locked desktop dependency closure.
const metadata = JSON.parse(execFileSync('cargo', ['metadata','--locked','--offline','--format-version','1'], {maxBuffer:20_000_000,encoding:'utf8'}));
const packages = new Map(metadata.packages.map(pkg=>[pkg.id,pkg]));
const nodes = new Map(metadata.resolve.nodes.map(node=>[node.id,node]));
const desktop = metadata.packages.find(pkg=>pkg.name==='av-desktop');
const visited = new Set();
function visit(id) {
 if(visited.has(id))return;
 visited.add(id);
 for(const dependency of nodes.get(id)?.deps??[]) {
  if(dependency.dep_kinds.some(kind=>kind.kind!=='dev'))visit(dependency.pkg);
 }
}
visit(desktop.id);
const records=[];
const parts=['AntiVector — third-party notices\nGenerated from Cargo.lock and installed production JavaScript packages.\nIncludes desktop build and target-specific dependencies; some are not present on every platform.\nNo license for AntiVector itself is granted by this file.\n'];
const missing=[];
function add(name,version,license,root,explicit) {
 const files=[];
 function scan(directory,depth=0) {
  for(const entry of fs.readdirSync(directory,{withFileTypes:true})) {
   const location=path.join(directory,entry.name);
   if(entry.isFile()&&(/^(licen[sc]e|copying|notice|copyright)([._-]|$)/i.test(entry.name)||name.startsWith('cargo:r-efi')&&entry.name==='AUTHORS'||location===explicit))files.push(location);
   if(entry.isDirectory()&&depth<2&&/^(licenses?|legal)$/i.test(entry.name))scan(location,depth+1);
  }
 }
 scan(root);
 const supplement=path.resolve('third-party/licenses',`${name.replace(/^cargo:/,'')}-${version}`);
 if(!files.length&&fs.existsSync(supplement))scan(supplement);
 if(explicit&&fs.existsSync(explicit)&&!files.includes(explicit))files.push(explicit);
 files.sort();
 if(!files.length)missing.push(`${name}@${version}`);
 const source=name.startsWith('cargo:')?`https://crates.io/api/v1/crates/${name.slice(6)}/${version}/download`:`https://www.npmjs.com/package/${name.slice(4)}/v/${version}`;
 parts.push(`\n================ ${name}@${version} ================\nDeclared license: ${license}\nUnmodified upstream source: ${source}\n`);
 const texts=files.map(file=>{
  const bytes=fs.readFileSync(file);
  const relative=file.startsWith(root+path.sep)?path.relative(root,file).replaceAll('\\','/'):`upstream/${path.basename(file)}`;
  parts.push(`\n--- ${relative} ---\n${bytes.toString('utf8')}\n`);
  return {file:relative,sha256:crypto.createHash('sha256').update(bytes).digest('hex')};
 });
 records.push({name,version,license,texts});
}
for(const pkg of [...visited].map(id=>packages.get(id)).filter(pkg=>pkg.source).sort((a,b)=>a.name.localeCompare(b.name)||a.version.localeCompare(b.version))) {
 const root=path.dirname(pkg.manifest_path);
 add(`cargo:${pkg.name}`,pkg.version,pkg.license??'SEE LICENSE FILE',root,pkg.license_file&&path.resolve(root,pkg.license_file));
}
const jsSeen=new Set();
function visitJs(manifest) {
 const real=fs.realpathSync(manifest); if(jsSeen.has(real))return; jsSeen.add(real);
 const pkg=JSON.parse(fs.readFileSync(real,'utf8')),root=path.dirname(real);
 add(`npm:${pkg.name}`,pkg.version,pkg.license??'SEE LICENSE FILE',root);
 const require=createRequire(real);
 for(const name of Object.keys({...pkg.dependencies,...pkg.optionalDependencies}).sort()) {
  let next;
  try {next=require.resolve(`${name}/package.json`);}catch {
   let directory=root;
   while(directory!==path.dirname(directory)) {
    const candidate=path.join(directory,'node_modules',name,'package.json');
    if(fs.existsSync(candidate)){next=candidate;break;}
    directory=path.dirname(directory);
   }
  }
  if(!next)throw new Error(`Cannot locate installed production dependency ${name}`);
  visitJs(next);
 }
}
const app=JSON.parse(fs.readFileSync('app/package.json','utf8'));
for(const name of Object.keys(app.dependencies).sort())visitJs(path.resolve('app/node_modules',name,'package.json'));
const output='app/src-tauri/notices';fs.mkdirSync(output,{recursive:true});
fs.writeFileSync(`${output}/THIRD_PARTY_NOTICES.txt`,parts.join(''));
fs.writeFileSync(`${output}/manifest.json`,JSON.stringify({packages:records,missing_texts:missing},null,2)+'\n');
console.log(`${records.length} packages; ${missing.length} missing license texts`);
if(missing.length){console.error(missing.join('\n'));process.exitCode=1;}
