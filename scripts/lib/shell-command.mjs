import { spawn } from "node:child_process";

const fallbackShellPath = "/bin/sh";

function resolveShellPath() {
  const shellPath = process.env.SHELL?.trim();
  return shellPath && shellPath.length > 0 ? shellPath : fallbackShellPath;
}

export function spawnShell(command, options = {}) {
  return spawn(resolveShellPath(), ["-c", command], options);
}
