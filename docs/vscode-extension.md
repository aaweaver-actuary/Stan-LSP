# VS Code extension

The VS Code package is a thin client. It associates `.stan` files, launches the
bundled or configured server over stdio, forwards initialization settings, and
exposes restart, formatting, diagnostics, explanation, and stanc configuration
commands.

No Stan grammar or semantic logic belongs in the extension. Client tests validate
the manifest, settings, commands, activation, and server-configuration contract.
Editor-neutral users can run the same server from any client supporting standard
LSP stdio transport.
