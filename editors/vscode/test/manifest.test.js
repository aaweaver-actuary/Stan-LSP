const assert = require('node:assert/strict');
const test = require('node:test');
const manifest = require('../package.json');

test('activates for Stan and exposes the expected commands and settings', () => {
  assert.ok(manifest.activationEvents.includes('onLanguage:stan'));
  const commands = new Set(manifest.contributes.commands.map(command => command.command));
  for (const command of [
    'stanLsp.restart',
    'stanLsp.format',
    'stanLsp.configureStanc',
    'stanLsp.locateStanc',
    'stanLsp.workspaceDiagnostics',
    'stanLsp.explainDiagnostic'
  ]) assert.ok(commands.has(command), command);
  assert.equal(
    manifest.contributes.configuration.properties['stanLsp.stanVersion'].default,
    '2.39.0'
  );
});
