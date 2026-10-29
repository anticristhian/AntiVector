import fs from 'node:fs';
const css=fs.readFileSync('app/src/App.css','utf8');
const luminance=hex=>{
 const channels=hex.slice(1).match(/../g).map(part=>parseInt(part,16)/255).map(value=>value<=0.04045?value/12.92:((value+0.055)/1.055)**2.4);
 return channels[0]*0.2126+channels[1]*0.7152+channels[2]*0.0722;
};
const rows=[];
for(const [theme,pattern] of [['light',/:root\{(--surface:[^}]+)\}/],['dark',/:root\[data-theme=dark\]\{([^}]+)\}/]]) {
 const match=css.match(pattern); if(!match)throw new Error(`Missing ${theme} theme`);
 const colors=Object.fromEntries([...match[1].matchAll(/--([a-z-]+):\s*(#[0-9a-f]{6})/g)].map(m=>[m[1],m[2]]));
 for(const [foreground,background,minimum] of [
  ['text','surface',4.5],['muted','surface',4.5],['text','page',4.5],['muted','page',4.5],
  ['text','soft',4.5],['muted','soft',4.5],['on-accent','accent',4.5],['focus','surface',3],['line','surface',3],
 ]) {
  const a=luminance(colors[foreground]),b=luminance(colors[background]);
  const ratio=(Math.max(a,b)+0.05)/(Math.min(a,b)+0.05);
  rows.push({theme,foreground:colors[foreground],background:colors[background],usage:`${foreground}/${background}`,ratio,minimum,passed:ratio>=minimum});
 }
}
fs.mkdirSync('target',{recursive:true});
fs.writeFileSync('target/f9-contrast.json',JSON.stringify({method:'WCAG relative sRGB luminance, actual CSS theme tokens; excludes artwork and disabled controls',rows},null,2)+'\n');
console.log(rows.map(row=>`${row.theme} ${row.usage}: ${row.ratio.toFixed(3)} >= ${row.minimum}: ${row.passed}`).join('\n'));
if(rows.some(row=>!row.passed))process.exitCode=1;
