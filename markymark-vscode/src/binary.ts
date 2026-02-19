import path from 'path';

/** Maps platform-arch keys to bundled binary filenames. */
const BINARY_MAP: Record<string, string> = {
  'darwin-arm64': 'markymark-aarch64-apple-darwin',
  'darwin-x64': 'markymark-x86_64-apple-darwin',
  'linux-arm64': 'markymark-aarch64-unknown-linux-gnu',
  'linux-x64': 'markymark-x86_64-unknown-linux-gnu',
  'win32-x64': 'markymark-x86_64-pc-windows-msvc.exe',
};

/**
 * Platforms where a different binary can run via OS emulation.
 * Maps the missing key to the key whose binary to use instead.
 */
const EMULATION_FALLBACK: Record<string, string> = {
  'win32-arm64': 'win32-x64', // Windows ARM64 runs x64 via built-in emulation
};

/**
 * Resolve the path to the markymark binary.
 *
 * Resolution order:
 *   1. Explicit configPath (user override via markymark.path setting)
 *   2. Bundled binary matched by platform + arch
 *   3. Emulation fallback (e.g. win32-arm64 → win32-x64 binary)
 *   4. Bare name fallback: markymark.exe on Windows, markymark elsewhere
 *      (will succeed only if the binary is on the system PATH)
 *
 * All paths are joined with extensionPath/bin/ except for configPath overrides.
 */
export function resolveBinaryPath(
  extensionPath: string,
  platform: string,
  arch: string,
  configPath?: string,
): string {
  if (configPath) {
    return configPath;
  }

  const key = `${platform}-${arch}`;
  let name = BINARY_MAP[key];

  if (!name) {
    const fallbackKey = EMULATION_FALLBACK[key];
    if (fallbackKey) {
      name = BINARY_MAP[fallbackKey];
    }
  }

  if (!name) {
    name = platform === 'win32' ? 'markymark.exe' : 'markymark';
  }

  return path.join(extensionPath, 'bin', name);
}
