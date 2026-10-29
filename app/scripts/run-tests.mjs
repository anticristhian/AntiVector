// Keep Vite's atomic module writes inside the workspace on sandboxed Windows.
import { mkdirSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, URL } from 'node:url';
import { spawn } from 'node:child_process';

const root = fileURLToPath(new URL('../../', import.meta.url));
const temporary = resolve(root, 'target', 'vitest-tmp');
mkdirSync(temporary, { recursive: true });
const require = createRequire(import.meta.url);
const metadataPath = require.resolve('vitest/package.json');
const metadata = JSON.parse(readFileSync(metadataPath, 'utf8'));
const cli = resolve(dirname(metadataPath), typeof metadata.bin === 'string' ? metadata.bin : metadata.bin.vitest);
const child = spawn(process.execPath, [cli, 'run', ...process.argv.slice(2)], {
  stdio: 'inherit', env: { ...process.env, TEMP: temporary, TMP: temporary, TMPDIR: temporary },
});
child.on('error', error => { console.error(error.message); process.exitCode = 1; });
child.on('exit', code => { process.exitCode = code ?? 1; });
