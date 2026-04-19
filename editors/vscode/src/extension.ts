// Sapphire VSCode extension (L1 scaffold).
//
// Spawns the `sapphire-lsp` binary as a child process and talks LSP
// to it over stdio. The server binary is resolved from
// `SAPPHIRE_LSP_PATH` if set, otherwise the plain `sapphire-lsp`
// name (which requires the binary on $PATH).
//
// Later milestones (L2+) will add client-side configuration, status
// bar items, commands, etc. This file intentionally stays small.

import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
  const serverPath = process.env.SAPPHIRE_LSP_PATH ?? "sapphire-lsp";

  const serverOptions: ServerOptions = {
    run: { command: serverPath, transport: TransportKind.stdio },
    debug: { command: serverPath, transport: TransportKind.stdio },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "sapphire" }],
    outputChannelName: "Sapphire Language Server",
  };

  client = new LanguageClient(
    "sapphire",
    "Sapphire Language Server",
    serverOptions,
    clientOptions,
  );

  context.subscriptions.push({
    dispose: () => {
      // Ensure the client is stopped when the extension is disposed.
      if (client) {
        void client.stop();
      }
    },
  });

  void client.start();
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
