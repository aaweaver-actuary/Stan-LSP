const vscode = require('vscode');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');

let client;

function serverOptions() {
  const config = vscode.workspace.getConfiguration('stanLsp');
  const command = config.get('serverPath', 'stan-language-server');
  const stanc = config.get('stancPath', '');
  const env = { ...process.env };
  if (stanc) env.STANC = stanc;
  return {
    run: { command, transport: TransportKind.stdio, options: { env } },
    debug: { command, transport: TransportKind.stdio, options: { env } }
  };
}

async function start() {
  const config = vscode.workspace.getConfiguration('stanLsp');
  client = new LanguageClient(
    'stanLsp',
    'Stan Language Server',
    serverOptions(),
    {
      documentSelector: [{ scheme: 'file', language: 'stan' }],
      synchronize: { configurationSection: 'stanLsp' },
      initializationOptions: {
        stanVersion: config.get('stanVersion', '2.39.0'),
        stancPath: config.get('stancPath', '') || null,
        includePaths: config.get('includePaths', []),
        compilerOnSave: config.get('compilerOnSave', true),
        compilerOnChange: config.get('compilerOnChange', false),
        compilerDebounceMs: config.get('compilerDebounceMs', 500),
        formatIndentWidth: config.get('formatIndentWidth', 2),
        lintLevels: config.get('lintLevels', {})
      }
    }
  );
  await client.start();
}

async function restart() {
  if (client) await client.stop();
  await start();
}

async function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand('stanLsp.restart', restart),
    vscode.commands.registerCommand('stanLsp.format', () =>
      vscode.commands.executeCommand('editor.action.formatDocument')),
    vscode.commands.registerCommand('stanLsp.configureStanc', async () => {
      const current = vscode.workspace.getConfiguration('stanLsp').get('stancPath', '');
      const value = await vscode.window.showInputBox({
        title: 'Path to stanc3',
        value: current,
        placeHolder: '/path/to/stanc'
      });
      if (value !== undefined) {
        await vscode.workspace.getConfiguration('stanLsp').update(
          'stancPath', value, vscode.ConfigurationTarget.Workspace
        );
        await restart();
      }
    }),
    vscode.commands.registerCommand('stanLsp.locateStanc', async () => {
      const selected = await vscode.window.showOpenDialog({
        title: 'Locate stanc3 executable',
        canSelectFiles: true,
        canSelectFolders: false,
        canSelectMany: false
      });
      if (selected && selected[0]) {
        await vscode.workspace.getConfiguration('stanLsp').update(
          'stancPath', selected[0].fsPath, vscode.ConfigurationTarget.Workspace
        );
        await restart();
      }
    }),
    vscode.commands.registerCommand('stanLsp.workspaceDiagnostics', async () => {
      const documents = vscode.workspace.textDocuments.filter(document => document.languageId === 'stan');
      await Promise.all(documents.map(document => document.save()));
      vscode.window.showInformationMessage(`Requested validation for ${documents.length} Stan document(s).`);
    }),
    vscode.commands.registerCommand('stanLsp.explainDiagnostic', () =>
      vscode.env.openExternal(vscode.Uri.parse('https://github.com/stan-dev/stan/wiki')))
  );
  await start();
}

async function deactivate() {
  if (client) await client.stop();
}

module.exports = { activate, deactivate };
