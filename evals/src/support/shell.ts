import {
  type ChildProcess,
  type ChildProcessByStdio,
  type SpawnOptions,
  spawn,
} from "node:child_process";
import type { Readable, Writable } from "node:stream";

const fallbackShellPath = "/bin/sh";

type PipedChildProcess = ChildProcessByStdio<Writable, Readable, Readable>;
type ReadOnlyChildProcess = ChildProcessByStdio<null, Readable, Readable>;

function resolveShellPath(): string {
  const shellPath = process.env.SHELL?.trim();
  return shellPath && shellPath.length > 0 ? shellPath : fallbackShellPath;
}

export function spawnShellCommand(
  command: string,
  options: SpawnOptions & { stdio: ["pipe", "pipe", "pipe"] },
): PipedChildProcess;
export function spawnShellCommand(
  command: string,
  options: SpawnOptions & { stdio: ["ignore", "pipe", "pipe"] },
): ReadOnlyChildProcess;
export function spawnShellCommand(
  command: string,
  options: SpawnOptions = {},
): ChildProcess {
  return spawn(resolveShellPath(), ["-c", command], options);
}
