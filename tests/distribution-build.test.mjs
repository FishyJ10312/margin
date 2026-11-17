import assert from 'node:assert/strict';
import test from 'node:test';
import { distributionBuildEnvironment } from '../scripts/desktop-build.mjs';
test('distribution builds map home and project last without dropping compiler flags', () => {
  const env=distributionBuildEnvironment({RUSTFLAGS:'-C target-cpu=generic',TASK_FIXTURE:'kept'},'/build-agent/project','/build-agent');
  assert.deepEqual(env.CARGO_ENCODED_RUSTFLAGS.split('\x1f'),['-C','target-cpu=generic','--remap-path-prefix','/build-agent=/build/home','--remap-path-prefix','/build-agent/project=/src/margin']);
  assert.equal(env.TASK_FIXTURE,'kept');
});
test('encoded flags retain precedence and paths containing spaces remain one argument', () => {
  const env=distributionBuildEnvironment({CARGO_ENCODED_RUSTFLAGS:'-C\x1fopt-level=2',RUSTFLAGS:'ignored'},'C:\\Build Agent\\project','C:\\Build Agent');
  assert.deepEqual(env.CARGO_ENCODED_RUSTFLAGS.split('\x1f'),['-C','opt-level=2','--remap-path-prefix','C:\\Build Agent=/build/home','--remap-path-prefix','C:\\Build Agent\\project=/src/margin']);
});
