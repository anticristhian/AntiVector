// Offline DOM behavior test. Never loads resources or writes real human answers.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
const require=createRequire(path.resolve('app/package.json'));
const {JSDOM}=require('jsdom');
const directory='target/f3-smoke/out/visual';
const choices=fs.readdirSync(directory).filter(name=>name.startsWith('smoke-')&&fs.existsSync(path.join(directory,name,'index.html')))
 .sort((a,b)=>fs.statSync(path.join(directory,b,'index.html')).mtimeMs-fs.statSync(path.join(directory,a,'index.html')).mtimeMs);
assert(choices.length,'Run cargo test -p av-bench first to generate a synthetic evaluation');
const html=fs.readFileSync(path.join(directory,choices[0],'index.html'),'utf8');
function open(saved,blocked=false){
 const dom=new JSDOM(html,{url:'https://av-test.invalid/',runScripts:'outside-only'});
 const window=dom.window;
 // jsdom does not expose this browser built-in; use Node's identical operation.
 window.structuredClone=structuredClone;
 if(saved)window.localStorage.setItem(saved.key,saved.value);
 if(blocked)Object.defineProperty(window,'localStorage',{get(){throw new Error('storage disabled');}});
 window.eval(window.document.querySelector('script').textContent);
 const result=window.eval('result()');
 assert(result.label.startsWith('smoke-')&&result.answers.length===3,'Only synthetic 3-pair pages may be exercised');
 return dom;
}
function choose(window,pair,value){
 const radio=window.document.querySelector(`input[name=p${pair}][value="${value}"]`);
 radio.checked=true;radio.dispatchEvent(new window.Event('change',{bubbles:true}));
}
async function save(window){
 let blob,download;
 window.URL.createObjectURL=value=>{blob=value;return 'blob:offline-test';};
 window.URL.revokeObjectURL=()=>{};
 window.HTMLAnchorElement.prototype.click=function(){download=this.download;};
 window.document.getElementById('save').click();
 const value=await new Promise((resolve,reject)=>{const reader=new window.FileReader();reader.onload=()=>resolve(JSON.parse(reader.result));reader.onerror=reject;reader.readAsText(blob);});
 assert.equal(download,value.label+'.json');
 return value;
}
const dom=open();const window=dom.window;const document=window.document;
assert.match(document.getElementById('progress').textContent,/0 \/ 3/);
assert.equal(document.querySelectorAll('img[src$=".svg"]').length,6);
assert.equal(document.querySelectorAll('img[src$="-original.png"]').length,3);
document.querySelector('button[data-zoom="400"]').click();
assert.equal(document.body.dataset.zoom,'400');assert.equal(document.body.style.getPropertyValue('--scale'),'4');
const panes=document.querySelectorAll('.pair .viewport');
panes[0].scrollLeft=80;panes[0].scrollTop=40;panes[0].dispatchEvent(new window.Event('scroll'));
assert.equal(panes[1].scrollLeft,80);assert.equal(panes[2].scrollTop,40);
choose(window,0,'A');
const partial=await save(window);assert.equal(partial.status,'partial');assert.equal(partial.answers[0].choice,'A');
const key=window.localStorage.key(0);const saved={key,value:window.localStorage.getItem(key)};
const restored=open(saved);assert(restored.window.document.querySelector('input[name=p0][value="A"]').checked);
assert.match(restored.window.document.getElementById('progress').textContent,/1 \/ 3/);
choose(window,1,'B');choose(window,2,'tie');
const complete=await save(window);assert.equal(complete.status,'completed_by_human');
assert.deepEqual(complete.answers.map(answer=>answer.choice),['A','B','tie']);
const blocked=open(null,true);assert.match(blocked.window.document.getElementById('progress').textContent,/0 \/ 3/);
choose(blocked.window,0,'tie');assert.equal((await save(blocked.window)).status,'partial');
dom.window.close();restored.window.close();blocked.window.close();
console.log('Offline synthetic visual-page checks passed: vectors/original, zoom, scroll sync, draft persistence, partial/complete JSON, disabled storage. No real votes written.');
