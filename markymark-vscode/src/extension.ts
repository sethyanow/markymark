import * as fs from 'fs';
import * as vscode from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from 'vscode-languageclient/node';
import { resolveBinaryPath } from './binary';

const RELEASE_URL = 'https://github.com/sethyanow/markymark/releases';

let client: LanguageClient | undefined;

/**
 * Activates the Markymark language server extension.
 * @param context - The extension context provided by VS Code.
 */
export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const configPath = vscode.workspace
    .getConfiguration('markymark')
    .get<string>('path') || undefined;

  const binaryPath = resolveBinaryPath(
    context.extensionPath,
    process.platform,
    process.arch,
    configPath,
  );

  if (!fs.existsSync(binaryPath)) {
    const action = await vscode.window.showErrorMessage(
      `Markymark: binary not found at ${binaryPath}. ` +
        `Install markymark and set markymark.path, or download from GitHub Releases.`,
      'Open Releases',
    );
    if (action === 'Open Releases') {
      await vscode.env.openExternal(vscode.Uri.parse(RELEASE_URL));
    }
    return;
  }

  if (process.platform !== 'win32') {
    try {
      fs.accessSync(binaryPath, fs.constants.X_OK);
    } catch {
      try {
        fs.chmodSync(binaryPath, 0o755);
      } catch (chmodErr) {
        vscode.window.showErrorMessage(
          `Markymark: failed to set executable permission on ${binaryPath}: ${chmodErr}`,
        );
        return;
      }
    }
  }

  const serverOptions: ServerOptions = {
    command: binaryPath,
    args: ['--lsp'],
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', language: 'markdown' },
      { scheme: 'file', language: 'mdx' },
    ],
    outputChannelName: 'Markymark',
  };

  client = new LanguageClient('markymark', 'Markymark', serverOptions, clientOptions);
  await client.start();
}

/**
 * Deactivates the extension by stopping the language client.
 */
export async function deactivate(): Promise<void> {
  await client?.stop();
}
