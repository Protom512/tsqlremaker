/**
 * SAP ASE T-SQL Language Server — VSCode extension client.
 *
 * Spawns the `ase-ls` binary via stdio and connects it to VSCode
 * using the vscode-languageclient library.
 */

import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  Trace,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

/**
 * Resolves the path to the ase-ls binary.
 *
 * Resolution order:
 *  1. `ase-ls.path` setting (absolute path)
 *  2. `ase-ls` on system PATH
 */
function serverCommand(): string {
  const config = vscode.workspace.getConfiguration("ase-ls");
  const configured = config.get<string>("path", "");
  if (configured.length > 0) {
    return configured;
  }
  return "ase-ls";
}

/** Builds the ServerOptions for spawning the language server binary. */
function serverOptions(): ServerOptions {
  const config = vscode.workspace.getConfiguration("ase-ls");
  const logLevel = config.get<string>("logLevel", "info");

  return {
    command: serverCommand(),
    args: ["--log-level", logLevel],
    options: {
      shell: true,
    },
  };
}

/** Builds the LanguageClientOptions. */
function clientOptions(): LanguageClientOptions {
  return {
    documentSelector: [{ scheme: "file", language: "sql" }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.sql"),
    },
    traceOutputChannel: vscode.window.createOutputChannel(
      "ASE Language Server Trace",
    ),
  };
}

export async function activate(
  _context: vscode.ExtensionContext,
): Promise<void> {
  const traceConfig = vscode.workspace
    .getConfiguration("ase-ls")
    .get<string>("trace.server", "off");

  client = new LanguageClient(
    "ase-ls",
    "SAP ASE Language Server",
    serverOptions(),
    clientOptions(),
  );

  await client.start();

  await client.setTrace(Trace.fromString(traceConfig));
}

export async function deactivate(): Promise<void> {
  if (client === undefined) {
    return;
  }
  await client.stop();
}
