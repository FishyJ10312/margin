#!/usr/bin/env node
// Build-time only. End users do not need Node or Pi.
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';

export function distributionBuildEnvironment(env, project, home) {
  // Cargo gives the encoded form precedence and splits RUSTFLAGS on whitespace.
  const flags = env.CARGO_ENCODED_RUSTFLAGS !== undefined
    ? env.CARGO_ENCODED_RUSTFLAGS.split('\x1f').filter(Boolean)
    : (env.RUSTFLAGS || '').split(/\s+/).filter(Boolean);
  // Rust uses the last matching mapping: the project is more specific than home.
  for (const [from, to] of [[home, '/build/home'], [project, '/src/margin']]) {
    flags.push('--remap-path-prefix', `${from}=${to}`);
  }
  return { ...env, CARGO_ENCODED_RUSTFLAGS: flags.join('\x1f') };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const project = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  const args = process.argv.slice(2);
  if (args.some(arg => arg.includes('qa-source-block'))) {
    throw new Error('Fault-injection features must not enter a distribution build');
  }
  const result = spawnSync(process.execPath,
    [path.join(project, 'node_modules/@tauri-apps/cli/tauri.js'), 'build', ...args],
    { cwd: project, env: distributionBuildEnvironment(process.env, project, os.homedir()), stdio: 'inherit' });
  if (result.error) throw result.error;
  process.exitCode = result.status ?? 1;
}
