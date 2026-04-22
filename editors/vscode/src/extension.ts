// Sapphire VSCode extension.
//
// Launches the `sapphire-lsp` binary as a child process and talks LSP
// to it over stdio.
//
// Binary resolution (highest priority first):
//   1. `SAPPHIRE_LSP_PATH` environment variable.
//   2. `sapphire.lsp.path` workspace/user setting.
//   3. `sapphire-lsp` resolved from $PATH.
//
// Log level is derived the same way from `SAPPHIRE_LSP_LOG` /
// `sapphire.lsp.log`, and `sapphire.trace.server` controls LSP JSON
// message tracing on the client side.
//
// See docs/impl/23-vscode-extension-polish.md for the design notes.

import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

const CONFIG_SECTION = "sapphire";

function resolveServerPath(config: vscode.WorkspaceConfiguration): string {
  const envPath = process.env.SAPPHIRE_LSP_PATH;
  if (envPath && envPath.length > 0) {
    return envPath;
  }
  const configured = config.get<string>("lsp.path", "").trim();
  if (configured.length > 0) {
    return configured;
  }
  return "sapphire-lsp";
}

function resolveLogLevel(config: vscode.WorkspaceConfiguration): string {
  const envLog = process.env.SAPPHIRE_LSP_LOG;
  if (envLog && envLog.length > 0) {
    return envLog;
  }
  return config.get<string>("lsp.log", "info");
}

function buildServerOptions(
  config: vscode.WorkspaceConfiguration,
): ServerOptions {
  const serverPath = resolveServerPath(config);
  const logLevel = resolveLogLevel(config);
  const env = {
    ...process.env,
    SAPPHIRE_LSP_LOG: logLevel,
  };
  // Intentionally no `transport` field: in vscode-languageclient@9,
  // `transport: TransportKind.stdio` appends `--stdio` to argv, but
  // `sapphire-lsp` rejects any flag other than `--version` / `--help`
  // and exits non-zero. Omitting `transport` keeps the default pipe.
  const runDebug = {
    command: serverPath,
    options: { env },
  };
  return { run: runDebug, debug: runDebug };
}

export function activate(context: vscode.ExtensionContext): void {
  const config = vscode.workspace.getConfiguration(CONFIG_SECTION);

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "sapphire" }],
    outputChannelName: "Sapphire Language Server",
  };

  client = new LanguageClient(
    "sapphire",
    "Sapphire Language Server",
    buildServerOptions(config),
    clientOptions,
  );

  // React to configuration changes by asking the user to reload. The
  // server binary path, log level, and trace setting are all resolved
  // at client construction time, so they require a full reload to
  // take effect.
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (!event.affectsConfiguration(CONFIG_SECTION)) {
        return;
      }
      const reload = "Reload Window";
      void vscode.window
        .showInformationMessage(
          "Sapphire settings changed. Reload the window for them to take effect.",
          reload,
        )
        .then((choice) => {
          if (choice === reload) {
            void vscode.commands.executeCommand("workbench.action.reloadWindow");
          }
        });
    }),
  );

  context.subscriptions.push({
    dispose: () => {
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
