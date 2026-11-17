#!/usr/bin/env node
// Passive inventory of modules that actually contribute code to the bundle.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'vite';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
if (!process.argv[2]) throw new Error('A separate audit output directory is required');
const output = path.resolve(process.argv[2]);
if (output === root || output.startsWith(root + path.sep)) throw new Error('Keep audit outputs outside the source tree');
if (fs.existsSync(output)) throw new Error('Refusing to replace an existing audit output');
fs.mkdirSync(output, { recursive: true });
await build({ root, configFile: path.join(root, 'vite.config.ts'),
  build: { outDir: path.join(output, 'dist'), emptyOutDir: false },
  plugins: [{ name: 'runtime-module-inventory', generateBundle(_options, bundle) {
    const packages = new Map();
    for (const chunk of Object.values(bundle)) {
      if (chunk.type !== 'chunk') continue;
      for (const [id, info] of Object.entries(chunk.modules)) {
        if (!info.renderedLength || !id.includes('/node_modules/')) continue;
        let folder = path.dirname(id.split('?')[0]);
        while (folder !== path.dirname(folder)) {
          const manifest = path.join(folder, 'package.json');
          if (fs.existsSync(manifest)) {
            const data = JSON.parse(fs.readFileSync(manifest, 'utf8'));
            if (data.name && data.version) {
              packages.set(`${data.name}@${data.version}`, { name: data.name, version: data.version, license: data.license });
              break;
            }
          }
          folder = path.dirname(folder);
        }
      }
    }
    fs.writeFileSync(path.join(output, 'runtime-packages.json'),
      JSON.stringify([...packages.values()].sort((a,b) => a.name.localeCompare(b.name)), null, 2) + '\n');
  } }],
});
