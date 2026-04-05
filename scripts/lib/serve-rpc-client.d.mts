export type ServeRpcClient = {
  readonly child: import("node:child_process").ChildProcessWithoutNullStreams;
  call<T = unknown>(
    method: string,
    params?: Record<string, unknown>,
  ): Promise<T>;
  close(): Promise<void>;
};

export function createServeRpcClient(options?: {
  readonly cwd?: string;
  readonly serveCommand?: string;
  readonly requestTimeoutMs?: number;
}): ServeRpcClient;
