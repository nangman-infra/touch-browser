import type { ChildProcessWithoutNullStreams } from "node:child_process";

import { afterEach, describe, expect, it } from "vitest";

import { repoRoot } from "../support/paths.js";
import { spawnShellCommand } from "../support/shell.js";

type RpcClient = {
  readonly child: ChildProcessWithoutNullStreams;
  call<T>(method: string, params?: Record<string, unknown>): Promise<T>;
  close(): Promise<void>;
};

type RuntimeBlock = {
  readonly kind: string;
  readonly ref?: string;
  readonly text?: string;
};

type SessionCreateResponse = {
  readonly activeTabId: string;
  readonly sessionId: string;
};

type SessionOpenResponse = {
  readonly result: {
    readonly output: {
      readonly blocks: readonly RuntimeBlock[];
    };
    readonly status: string;
  };
  readonly tabId: string;
};

type SessionActionResponse = {
  readonly result: {
    readonly action: {
      readonly failureKind?: string;
      readonly output: {
        readonly adapter: {
          readonly visibleText: string;
        };
      };
      readonly status: string;
    };
    readonly sessionState?: {
      readonly mode: string;
    };
  };
  readonly tabId?: string;
};

type TabListResponse = {
  readonly tabs: readonly unknown[];
};

type SessionSynthesisResponse = {
  readonly report: {
    readonly snapshotCount: number;
    readonly visitedUrls: readonly string[];
  };
  readonly tabCount: number;
  readonly tabReports: readonly unknown[];
};

type SessionCloseResponse = {
  readonly removed: boolean;
  readonly removedTabs?: number;
};

type SessionCheckpointResponse = {
  readonly result: {
    readonly checkpoint: {
      readonly approvalPanel: {
        readonly recommendedPolicyProfile: string;
      };
      readonly playbook: {
        readonly provider: string;
      };
      readonly requiredAckRisks: readonly string[];
    };
  };
};

type SessionApprovalResponse = {
  readonly approvedRisks: readonly string[];
  readonly policyProfile: string;
};

type SessionProfileResponse = {
  readonly result: {
    readonly policyProfile: string;
  };
};

type SecretStoreResponse = {
  readonly stored: boolean;
};

type TelemetrySummaryResponse = {
  readonly summary: {
    readonly totalEvents: number;
  };
};

type RuntimeStatusResponse = {
  readonly daemon: boolean;
  readonly methods: readonly string[];
};

describe("serve daemon session registry", () => {
  const clients: RpcClient[] = [];

  afterEach(async () => {
    await Promise.allSettled(clients.map((client) => client.close()));
    clients.length = 0;
  });

  it("supports long-lived browser sessions, multi-tab navigation, and synthesis", async () => {
    const client = createRpcClient();
    clients.push(client);

    const status = await client.call<RuntimeStatusResponse>("runtime.status");
    expect(status.daemon).toBe(true);
    expect(status.methods).toContain("runtime.session.create");
    expect(status.methods).toContain("runtime.tab.open");
    expect(status.methods).toContain("runtime.search");
    expect(status.methods).toContain("runtime.search.openResult");
    expect(status.methods).toContain("runtime.search.openTop");

    const created = await client.call<SessionCreateResponse>(
      "runtime.session.create",
      {},
    );
    expect(created.sessionId).toMatch(/^srvsess-/);
    expect(created.activeTabId).toMatch(/^tab-/);

    const firstTab = created.activeTabId as string;
    const open = await client.call<SessionOpenResponse>(
      "runtime.session.open",
      {
        sessionId: created.sessionId,
        tabId: firstTab,
        target: "fixture://research/navigation/browser-follow",
      },
    );
    expect(open.tabId).toBe(firstTab);
    expect(open.result.status).toBe("succeeded");

    const followRef = open.result.output.blocks.find(
      (block: { readonly kind: string }) => block.kind === "link",
    )?.ref;
    expect(followRef).toBeTruthy();

    const secondTabOpen = await client.call<SessionOpenResponse>(
      "runtime.tab.open",
      {
        sessionId: created.sessionId,
        target: "fixture://research/navigation/browser-expand",
      },
    );
    expect(secondTabOpen.tabId).toMatch(/^tab-/);
    expect(secondTabOpen.tabId).not.toBe(firstTab);
    expect(secondTabOpen.result.status).toBe("succeeded");

    const listed = await client.call<TabListResponse>("runtime.tab.list", {
      sessionId: created.sessionId,
    });
    expect(listed.tabs).toHaveLength(2);

    const expandRef = secondTabOpen.result.output.blocks.find(
      (block: { readonly kind: string }) => block.kind === "button",
    )?.ref;
    expect(expandRef).toBeTruthy();

    const expand = await client.call<SessionActionResponse>(
      "runtime.session.expand",
      {
        sessionId: created.sessionId,
        targetRef: expandRef,
      },
    );
    expect(expand.tabId).toBe(secondTabOpen.tabId);
    expect(expand.result.action.status).toBe("succeeded");

    const selectedFirst = await client.call<{ readonly activeTabId: string }>(
      "runtime.tab.select",
      {
        sessionId: created.sessionId,
        tabId: firstTab,
      },
    );
    expect(selectedFirst.activeTabId).toBe(firstTab);

    const followed = await client.call<SessionActionResponse>(
      "runtime.session.follow",
      {
        sessionId: created.sessionId,
        targetRef: followRef,
      },
    );
    expect(followed.tabId).toBe(firstTab);
    expect(followed.result.action.status).toBe("succeeded");

    const synthesis = await client.call<SessionSynthesisResponse>(
      "runtime.session.synthesize",
      {
        sessionId: created.sessionId,
        noteLimit: 8,
      },
    );
    expect(synthesis.tabCount).toBe(2);
    expect(synthesis.tabReports).toHaveLength(2);
    expect(synthesis.report.snapshotCount).toBeGreaterThanOrEqual(4);
    expect(synthesis.report.visitedUrls.length).toBeGreaterThanOrEqual(2);

    const closed = await client.call<SessionCloseResponse>(
      "runtime.session.close",
      {
        sessionId: created.sessionId,
      },
    );
    expect(closed.removed).toBe(true);
    expect(closed.removedTabs).toBe(2);
  }, 60_000);

  it("supports allowlisted interactive typing and submit inside a daemon tab", async () => {
    const client = createRpcClient();
    clients.push(client);

    const created = await client.call<SessionCreateResponse>(
      "runtime.session.create",
      {
        allowDomains: ["research"],
      },
    );
    const firstTab = created.activeTabId as string;

    const opened = await client.call<SessionOpenResponse>(
      "runtime.session.open",
      {
        sessionId: created.sessionId,
        tabId: firstTab,
        target: "fixture://research/navigation/browser-login-form",
        allowDomains: ["research"],
      },
    );
    const emailRef = opened.result.output.blocks.find(
      (block) =>
        block.kind === "input" &&
        block.text?.includes("agent@example.com") === true,
    )?.ref;
    const formRef = opened.result.output.blocks.find(
      (block: { readonly kind: string }) => block.kind === "form",
    )?.ref;
    const buttonRef = opened.result.output.blocks.find(
      (block: { readonly kind: string }) => block.kind === "button",
    )?.ref;
    expect(emailRef).toBeTruthy();
    expect(formRef).toBeTruthy();
    expect(buttonRef).toBeTruthy();

    const typed = await client.call<SessionActionResponse>(
      "runtime.session.type",
      {
        sessionId: created.sessionId,
        targetRef: emailRef,
        value: "agent@example.com",
      },
    );
    expect(typed.result.action.status).toBe("succeeded");
    expect(typed.result.sessionState?.mode).toBe("interactive");

    const submitted = await client.call<SessionActionResponse>(
      "runtime.session.submit",
      {
        sessionId: created.sessionId,
        targetRef: formRef,
      },
    );
    expect(submitted.result.action.status).toBe("succeeded");
    expect(submitted.result.action.output.adapter.visibleText).toContain(
      "Signed in draft session ready for review.",
    );

    const closed = await client.call<SessionCloseResponse>(
      "runtime.session.close",
      {
        sessionId: created.sessionId,
      },
    );
    expect(closed.removed).toBe(true);
  }, 20_000);

  it("supports supervised challenge refresh, daemon secrets, and high-risk acknowledgements", async () => {
    const client = createRpcClient();
    clients.push(client);

    const created = await client.call<SessionCreateResponse>(
      "runtime.session.create",
      {
        allowDomains: ["research"],
      },
    );

    const mfaOpen = await client.call<SessionOpenResponse>(
      "runtime.session.open",
      {
        sessionId: created.sessionId,
        target: "fixture://research/navigation/browser-mfa-challenge",
        allowDomains: ["research"],
      },
    );
    const otpRef = mfaOpen.result.output.blocks.find(
      (block) =>
        block.kind === "input" &&
        block.text?.toLowerCase().includes("otp") === true,
    )?.ref;
    const formRef = mfaOpen.result.output.blocks.find(
      (block: { readonly kind: string }) => block.kind === "form",
    )?.ref;
    expect(otpRef).toBeTruthy();
    expect(formRef).toBeTruthy();

    const blockedSubmit = await client.call<SessionActionResponse>(
      "runtime.session.submit",
      {
        sessionId: created.sessionId,
        targetRef: formRef,
      },
    );
    expect(blockedSubmit.result.action.status).toBe("rejected");
    expect(blockedSubmit.result.action.failureKind).toBe("policy-blocked");

    const checkpoint = await client.call<SessionCheckpointResponse>(
      "runtime.session.checkpoint",
      {
        sessionId: created.sessionId,
      },
    );
    expect(checkpoint.result.checkpoint.requiredAckRisks).toContain("mfa");
    expect(checkpoint.result.checkpoint.requiredAckRisks).toContain("auth");
    expect(checkpoint.result.checkpoint.playbook.provider).toBe("generic-auth");
    expect(
      checkpoint.result.checkpoint.approvalPanel.recommendedPolicyProfile,
    ).toBe("interactive-supervised-auth");

    const approval = await client.call<SessionApprovalResponse>(
      "runtime.session.approve",
      {
        sessionId: created.sessionId,
        ackRisks: ["mfa", "auth"],
      },
    );
    expect(approval.approvedRisks).toContain("mfa");
    expect(approval.approvedRisks).toContain("auth");
    expect(approval.policyProfile).toBe("interactive-supervised-auth");

    const profile = await client.call<SessionProfileResponse>(
      "runtime.session.profile.get",
      {
        sessionId: created.sessionId,
      },
    );
    expect(profile.result.policyProfile).toBe("interactive-supervised-auth");

    const stored = await client.call<SecretStoreResponse>(
      "runtime.session.secret.store",
      {
        sessionId: created.sessionId,
        targetRef: otpRef,
        value: "123456",
      },
    );
    expect(stored.stored).toBe(true);

    const typedSecret = await client.call<SessionActionResponse>(
      "runtime.session.typeSecret",
      {
        sessionId: created.sessionId,
        targetRef: otpRef,
      },
    );
    expect(typedSecret.result.action.status).toBe("succeeded");

    const submittedMfa = await client.call<SessionActionResponse>(
      "runtime.session.submit",
      {
        sessionId: created.sessionId,
        targetRef: formRef,
      },
    );
    expect(submittedMfa.result.action.status).toBe("succeeded");
    expect(submittedMfa.result.action.output.adapter.visibleText).toContain(
      "Verification code accepted for supervised review.",
    );

    const refreshed = await client.call<SessionActionResponse>(
      "runtime.session.refresh",
      {
        sessionId: created.sessionId,
      },
    );
    expect(refreshed.result.action.status).toBe("succeeded");

    const checkoutTab = await client.call<SessionOpenResponse>(
      "runtime.tab.open",
      {
        sessionId: created.sessionId,
        target: "fixture://research/navigation/browser-high-risk-checkout",
        allowDomains: ["research"],
      },
    );
    const checkoutFormRef = checkoutTab.result.output.blocks.find(
      (block: { readonly kind: string }) => block.kind === "form",
    )?.ref;
    expect(checkoutFormRef).toBeTruthy();

    const blockedCheckout = await client.call<SessionActionResponse>(
      "runtime.session.submit",
      {
        sessionId: created.sessionId,
        tabId: checkoutTab.tabId,
        targetRef: checkoutFormRef,
      },
    );
    expect(blockedCheckout.result.action.status).toBe("rejected");
    expect(blockedCheckout.result.action.failureKind).toBe("policy-blocked");

    const approvedCheckoutRisk = await client.call<SessionApprovalResponse>(
      "runtime.session.approve",
      {
        sessionId: created.sessionId,
        ackRisks: ["high-risk-write"],
      },
    );
    expect(approvedCheckoutRisk.approvedRisks).toContain("high-risk-write");
    expect(approvedCheckoutRisk.policyProfile).toBe(
      "interactive-supervised-write",
    );

    const approvedCheckout = await client.call<SessionActionResponse>(
      "runtime.session.submit",
      {
        sessionId: created.sessionId,
        tabId: checkoutTab.tabId,
        targetRef: checkoutFormRef,
      },
    );
    expect(approvedCheckout.result.action.status).toBe("succeeded");
    expect(approvedCheckout.result.action.output.adapter.visibleText).toContain(
      "Purchase confirmation staged for supervised review.",
    );

    const closed = await client.call<SessionCloseResponse>(
      "runtime.session.close",
      {
        sessionId: created.sessionId,
      },
    );
    expect(closed.removed).toBe(true);

    const telemetrySummary = await client.call<TelemetrySummaryResponse>(
      "runtime.telemetry.summary",
      {},
    );
    expect(telemetrySummary.summary.totalEvents).toBeGreaterThan(0);
  }, 35_000);
});

function createRpcClient(): RpcClient {
  const child = spawnShellCommand("target/debug/touch-browser serve", {
    cwd: repoRoot,
    stdio: ["pipe", "pipe", "pipe"],
  }) as ChildProcessWithoutNullStreams;

  const pending = new Map<
    number,
    {
      resolve: (value: unknown) => void;
      reject: (error: Error) => void;
    }
  >();
  let nextId = 0;
  let stdoutBuffer = "";
  let stderrBuffer = "";

  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    stdoutBuffer += chunk;
    const lines = stdoutBuffer.split("\n");
    stdoutBuffer = lines.pop() ?? "";

    for (const line of lines) {
      if (!line.trim()) {
        continue;
      }
      const payload = JSON.parse(line);
      const handler = pending.get(payload.id);
      if (!handler) {
        continue;
      }
      pending.delete(payload.id);
      if (payload.error) {
        handler.reject(new Error(payload.error.message));
      } else {
        handler.resolve(payload.result);
      }
    }
  });

  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => {
    stderrBuffer += chunk;
  });

  child.on("exit", (code) => {
    if (pending.size === 0) {
      return;
    }
    const error = new Error(
      `serve daemon exited with code ${code ?? -1}: ${stderrBuffer.trim()}`,
    );
    for (const handler of pending.values()) {
      handler.reject(error);
    }
    pending.clear();
  });

  return {
    child,
    call<T>(method: string, params: Record<string, unknown> = {}) {
      const id = ++nextId;
      const payload = JSON.stringify({
        jsonrpc: "2.0",
        id,
        method,
        params,
      });

      return new Promise<T>((resolve, reject) => {
        pending.set(id, { resolve: (value) => resolve(value as T), reject });
        child.stdin.write(`${payload}\n`, "utf8", (error) => {
          if (error) {
            pending.delete(id);
            reject(error);
          }
        });
      });
    },
    async close() {
      if (child.killed || child.exitCode !== null) {
        return;
      }

      child.stdin.end();
      await new Promise<void>((resolve) => {
        child.once("close", () => resolve());
        setTimeout(() => {
          if (child.exitCode === null) {
            child.kill("SIGTERM");
          }
        }, 250);
      });
    },
  };
}
