import { describe, it, expect } from 'vitest';
import path from 'path';
import { resolveBinaryPath } from '../src/binary';

const EXT = '/ext';

describe('resolveBinaryPath', () => {
  it('darwin-arm64 → aarch64-apple-darwin', () => {
    expect(resolveBinaryPath(EXT, 'darwin', 'arm64')).toBe(
      path.join(EXT, 'bin', 'markymark-aarch64-apple-darwin'),
    );
  });

  it('darwin-x64 → x86_64-apple-darwin', () => {
    expect(resolveBinaryPath(EXT, 'darwin', 'x64')).toBe(
      path.join(EXT, 'bin', 'markymark-x86_64-apple-darwin'),
    );
  });

  it('linux-arm64 → aarch64-unknown-linux-gnu', () => {
    expect(resolveBinaryPath(EXT, 'linux', 'arm64')).toBe(
      path.join(EXT, 'bin', 'markymark-aarch64-unknown-linux-gnu'),
    );
  });

  it('linux-x64 → x86_64-unknown-linux-gnu', () => {
    expect(resolveBinaryPath(EXT, 'linux', 'x64')).toBe(
      path.join(EXT, 'bin', 'markymark-x86_64-unknown-linux-gnu'),
    );
  });

  it('win32-x64 → x86_64-pc-windows-msvc.exe', () => {
    expect(resolveBinaryPath(EXT, 'win32', 'x64')).toBe(
      path.join(EXT, 'bin', 'markymark-x86_64-pc-windows-msvc.exe'),
    );
  });

  it('win32-arm64 falls back to win32-x64 via emulation', () => {
    expect(resolveBinaryPath(EXT, 'win32', 'arm64')).toBe(
      path.join(EXT, 'bin', 'markymark-x86_64-pc-windows-msvc.exe'),
    );
  });

  it('win32 unknown arch returns bare markymark.exe for PATH lookup', () => {
    expect(resolveBinaryPath(EXT, 'win32', 'ia32')).toBe('markymark.exe');
  });

  it('unknown platform returns bare markymark for PATH lookup', () => {
    expect(resolveBinaryPath(EXT, 'freebsd', 'x64')).toBe('markymark');
  });

  it('configPath override takes precedence over platform detection', () => {
    expect(resolveBinaryPath(EXT, 'linux', 'x64', '/custom/markymark')).toBe(
      '/custom/markymark',
    );
  });
});
