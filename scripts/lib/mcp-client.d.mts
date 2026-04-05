export type McpClient = {
  readonly child: import("node:child_process").ChildProcessWithoutNullStreams;
  initialize(): Promise<void>;
  listTools(): Promise<{
    readonly tools: ReadonlyArray<{
      readonly name: string;
    }>;
  }>;
  callTool<T = unknown>(
    name: string,
    args: Record<string, unknown>,
  ): Promise<T>;
  close(): Promise<void>;
};

export function createMcpClient(options: {
  readonly cwd: string;
  readonly bridgeCommand: string;
  readonly clientInfo: {
    readonly name: string;
    readonly version: string;
  };
  readonly requestTimeoutMs?: number;
}): McpClient;
