import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
const root='target/release/bundle';
const extension={win32:'.msi',darwin:'.dmg',linux:'.AppImage'}[process.platform];
if(!extension)throw new Error('Unsupported installer platform');
const files=[];
function walk(directory) {
 for(const entry of fs.readdirSync(directory,{withFileTypes:true})) {
  const file=path.join(directory,entry.name);
  if(entry.isDirectory())walk(file);else if(entry.name.endsWith(extension))files.push(file);
 }
}
walk(root);
if(!files.length)throw new Error(`Missing ${extension} installer`);
const rows=files.map(file=>({file:file.replaceAll('\\','/'),bytes:fs.statSync(file).size,
 sha256:crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex')}));
fs.writeFileSync(path.join(root,'installer-sizes.json'),JSON.stringify({limit_bytes:25_000_000,platform:process.platform,files:rows},null,2)+'\n');
console.log(rows);
if(rows.some(row=>row.bytes===0||row.bytes>25_000_000))throw new Error('Installer outside RNF09 limit');
