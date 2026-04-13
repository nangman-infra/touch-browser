import { spawn } from "node:child_process";
import { access } from "node:fs/promises";
import process from "node:process";

import {
  ensureRuntimeInstalled,
  resolveRuntimeDescriptor,
} from "./runtime-install.mjs";

const HELP_TEXT = `
touch-browser-mcp

Default behavior:
  Start the installed touch-browser MCP bridge for public docs and research web workflows.

Commands:
  install       Download and verify the matching standalone runtime bundle
  doctor        Print runtime resolution details
  bundle-path   Print the installed bundle path
  help          Show this help text
`.trim();

const args = process.argv.slice(2);
const command = args[0];

if (command === "help" || command === "--help" || command === "-h") {
  console.error(HELP_TEXT);
  process.exit(0);
}

if (command === "doctor") {
  const descriptor = await resolveRuntimeDescriptor(process.env);
  const installed = await fileExists(descriptor.binaryPath);
  process.stdout.write(
    `${JSON.stringify(
      {
        package: descriptor.metadata.name,
        version: descriptor.metadata.version,
        tag: descriptor.tag,
        platform: descriptor.platform,
        arch: descriptor.arch,
        assetUrl: descriptor.assetUrl,
        checksumUrl: descriptor.checksumUrl,
        bundleRoot: descriptor.bundleRoot,
        binaryPath: descriptor.binaryPath,
        installed,
      },
      null,
      2,
    )}\n`,
  );
  process.exit(0);
}

if (command === "bundle-path") {
  const descriptor = await ensureRuntimeInstalled();
  process.stdout.write(`${descriptor.bundleRoot}\n`);
  process.exit(0);
}

if (command === "install") {
  const descriptor = await ensureRuntimeInstalled();
  process.stderr.write(
    `[touch-browser-mcp] Installed runtime at ${descriptor.bundleRoot}\n`,
  );
  process.exit(0);
}

const passthroughArgs = command === "mcp" ? args.slice(1) : args;
const descriptor = await ensureRuntimeInstalled();
const child = spawn(descriptor.binaryPath, ["mcp", ...passthroughArgs], {
  stdio: "inherit",
  env: process.env,
});

for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
  process.on(signal, () => {
    child.kill(signal);
  });
}

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 0);
});

child.on("error", (error) => {
  console.error(
    `[touch-browser-mcp] Failed to launch runtime: ${error instanceof Error ? error.message : String(error)}`,
  );
  process.exit(1);
});

async function fileExists(targetPath) {
  try {
    await access(targetPath);
    return true;
  } catch {
    return false;
  }
}
