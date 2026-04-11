import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import {
  type IncomingMessage,
  type ServerResponse,
  createServer,
} from "node:http";
import { tmpdir } from "node:os";
import path from "node:path";

import { describe, expect, it } from "vitest";

import { repoRoot } from "../support/paths.js";
import { spawnShellCommand } from "../support/shell.js";

describe("standalone lifecycle smoke", () => {
  it("installs, updates, and uninstalls the managed standalone command", async () => {
    const tempRoot = mkdtempSync(
      path.join(tmpdir(), "touch-browser-standalone-lifecycle-"),
    );
    const bundleWorkspace = path.join(tempRoot, "bundles");
    const dataRoot = path.join(tempRoot, "data");
    const installDir = path.join(tempRoot, "bin");
    const binaryPath = path.join(repoRoot, "target", "debug", "touch-browser");

    mkdirSync(bundleWorkspace, { recursive: true });
    mkdirSync(dataRoot, { recursive: true });
    mkdirSync(installDir, { recursive: true });

    let server: ReturnType<typeof createServer> | undefined;

    try {
      await runShell("cargo build -q -p touch-browser-cli", process.env);

      const platform = currentPlatformSlug();
      const arch = currentArchSlug();
      const releaseA = createFakeStandaloneBundle({
        bundleWorkspace,
        version: "v0.1.0",
        platform,
        arch,
        binaryPath,
      });
      const releaseB = createFakeStandaloneBundle({
        bundleWorkspace,
        version: "v0.1.1",
        platform,
        arch,
        binaryPath,
      });

      server = await startFakeReleaseServer(releaseB);
      const address = server.address();
      if (!address || typeof address === "string") {
        throw new Error("fake release server address was not available");
      }

      const updateApiRoot = `http://127.0.0.1:${address.port}/api`;
      const env = {
        ...process.env,
        TOUCH_BROWSER_DATA_ROOT: dataRoot,
        TOUCH_BROWSER_INSTALL_DIR: installDir,
        TOUCH_BROWSER_UPDATE_API_ROOT: updateApiRoot,
      };

      await runShell(
        `${shellQuote(path.join(releaseA.bundleRoot, "install.sh"))} ${shellQuote(releaseA.bundleRoot)}`,
        env,
      );

      const commandPath = path.join(installDir, "touch-browser");
      const preflight = JSON.parse(
        await runShell(
          `${shellQuote(commandPath)} update --check --version 0.1.1`,
          env,
        ),
      );
      expect(preflight.currentVersion).toBe("v0.1.0");
      expect(preflight.targetVersion).toBe("v0.1.1");
      expect(preflight.updateAvailable).toBe(true);

      await runShell(`${shellQuote(commandPath)} update --version 0.1.1`, env);

      const installManifest = JSON.parse(
        readFileSync(
          path.join(dataRoot, "install", "install-manifest.json"),
          "utf8",
        ),
      );
      expect(installManifest.version).toBe("v0.1.1");
      expect(installManifest.managedBundleRoot).toContain(releaseB.bundleName);

      const postUpdate = JSON.parse(
        await runShell(`${shellQuote(commandPath)} update --check`, env),
      );
      expect(postUpdate.currentVersion).toBe("v0.1.1");
      expect(postUpdate.targetVersion).toBe("v0.1.1");
      expect(postUpdate.updateAvailable).toBe(false);

      mkdirSync(path.join(dataRoot, "browser-search"), { recursive: true });
      mkdirSync(path.join(dataRoot, "models"), { recursive: true });
      writeFileSync(
        path.join(dataRoot, "browser-search", "state.json"),
        "{}\n",
        "utf8",
      );
      writeFileSync(
        path.join(dataRoot, "models", "model.bin"),
        "model\n",
        "utf8",
      );

      const telemetrySummary = JSON.parse(
        await runShell(`${shellQuote(commandPath)} telemetry-summary`, env),
      );
      expect(telemetrySummary.summary.totalEvents).toBeGreaterThanOrEqual(2);
      expect(
        telemetrySummary.summary.operationCounts.update,
      ).toBeGreaterThanOrEqual(2);

      await runShell(
        `${shellQuote(commandPath)} uninstall --purge-all --yes`,
        env,
      );

      expectPathMissing(commandPath);
      expectPathMissing(path.join(dataRoot, "install"));
      expectPathMissing(path.join(dataRoot, "browser-search"));
      expectPathMissing(path.join(dataRoot, "pilot"));
      expectPathMissing(path.join(dataRoot, "models"));
    } finally {
      const activeServer = server;
      if (activeServer) {
        await new Promise<void>((resolve, reject) => {
          activeServer.close((error) => {
            if (error) {
              reject(error);
              return;
            }
            resolve();
          });
        });
      }
      rmSync(tempRoot, { recursive: true, force: true });
    }
  }, 120_000);
});

type BundleFixture = {
  readonly bundleName: string;
  readonly bundleRoot: string;
  readonly tarballPath: string;
  readonly checksumPath: string;
  readonly version: string;
};

function createFakeStandaloneBundle(options: {
  readonly bundleWorkspace: string;
  readonly version: string;
  readonly platform: string;
  readonly arch: string;
  readonly binaryPath: string;
}): BundleFixture {
  const bundleName = `touch-browser-${options.version}-${options.platform}-${options.arch}`;
  const bundleRoot = path.join(options.bundleWorkspace, bundleName);
  const binDir = path.join(bundleRoot, "bin");
  mkdirSync(binDir, { recursive: true });

  const installScriptSource = path.join(
    repoRoot,
    "scripts",
    "install-standalone-bundle.sh",
  );
  const uninstallScriptSource = path.join(
    repoRoot,
    "scripts",
    "uninstall-standalone-bundle.sh",
  );
  const installScriptTarget = path.join(bundleRoot, "install.sh");
  const uninstallScriptTarget = path.join(bundleRoot, "uninstall.sh");

  writeFileSync(
    installScriptTarget,
    readFileSync(installScriptSource, "utf8"),
    "utf8",
  );
  writeFileSync(
    uninstallScriptTarget,
    readFileSync(uninstallScriptSource, "utf8"),
    "utf8",
  );
  writeFileSync(
    path.join(binDir, "touch-browser"),
    [
      "#!/usr/bin/env bash",
      "set -euo pipefail",
      `exec ${shellQuote(options.binaryPath)} "$@"`,
      "",
    ].join("\n"),
    "utf8",
  );

  chmodSync(installScriptTarget, 0o755);
  chmodSync(uninstallScriptTarget, 0o755);
  chmodSync(path.join(binDir, "touch-browser"), 0o755);

  const tarballPath = path.join(
    options.bundleWorkspace,
    `${bundleName}.tar.gz`,
  );
  execFileSync("tar", [
    "-czf",
    tarballPath,
    "-C",
    options.bundleWorkspace,
    bundleName,
  ]);

  const checksum = createHash("sha256")
    .update(readFileSync(tarballPath))
    .digest("hex");
  const checksumPath = path.join(
    options.bundleWorkspace,
    `${bundleName}.sha256`,
  );
  writeFileSync(checksumPath, `${checksum}\n`, "utf8");

  return {
    bundleName,
    bundleRoot,
    tarballPath,
    checksumPath,
    version: options.version,
  };
}

async function startFakeReleaseServer(release: BundleFixture) {
  const server = createServer((request, response) => {
    try {
      handleReleaseRequest(request, response, release);
    } catch (error) {
      response.statusCode = 500;
      response.end(String(error));
    }
  });

  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => resolve());
  });

  return server;
}

function handleReleaseRequest(
  request: IncomingMessage,
  response: ServerResponse,
  release: BundleFixture,
) {
  const host = request.headers.host;
  if (!host) {
    response.statusCode = 400;
    response.end("missing host header");
    return;
  }

  const requestUrl = new URL(request.url ?? "/", `http://${host}`);
  if (requestUrl.pathname === "/api/releases/latest") {
    respondWithReleaseJson(response, release, requestUrl.origin);
    return;
  }

  if (requestUrl.pathname === `/api/releases/tags/${release.version}`) {
    respondWithReleaseJson(response, release, requestUrl.origin);
    return;
  }

  if (requestUrl.pathname === `/assets/${path.basename(release.tarballPath)}`) {
    response.statusCode = 200;
    response.setHeader("Content-Type", "application/gzip");
    response.end(readFileSync(release.tarballPath));
    return;
  }

  if (
    requestUrl.pathname === `/assets/${path.basename(release.checksumPath)}`
  ) {
    response.statusCode = 200;
    response.setHeader("Content-Type", "text/plain; charset=utf-8");
    response.end(readFileSync(release.checksumPath));
    return;
  }

  if (requestUrl.pathname === `/releases/${release.version}`) {
    response.statusCode = 200;
    response.setHeader("Content-Type", "text/plain; charset=utf-8");
    response.end(`release ${release.version}`);
    return;
  }

  response.statusCode = 404;
  response.end("not found");
}

function respondWithReleaseJson(
  response: ServerResponse,
  release: BundleFixture,
  origin: string,
) {
  response.statusCode = 200;
  response.setHeader("Content-Type", "application/json; charset=utf-8");
  response.end(
    JSON.stringify({
      tag_name: release.version,
      html_url: `${origin}/releases/${release.version}`,
      assets: [
        {
          name: path.basename(release.tarballPath),
          browser_download_url: `${origin}/assets/${path.basename(release.tarballPath)}`,
        },
        {
          name: path.basename(release.checksumPath),
          browser_download_url: `${origin}/assets/${path.basename(release.checksumPath)}`,
        },
      ],
    }),
  );
}

function currentPlatformSlug(): "linux" | "macos" {
  if (process.platform === "linux") {
    return "linux";
  }
  if (process.platform === "darwin") {
    return "macos";
  }
  throw new Error(
    `unsupported platform for standalone lifecycle test: ${process.platform}`,
  );
}

function currentArchSlug(): "x86_64" | "arm64" {
  if (process.arch === "x64") {
    return "x86_64";
  }
  if (process.arch === "arm64") {
    return "arm64";
  }
  throw new Error(
    `unsupported architecture for standalone lifecycle test: ${process.arch}`,
  );
}

function shellQuote(value: string): string {
  return `'${value.replaceAll("'", `'\"'\"'`)}'`;
}

function expectPathMissing(targetPath: string) {
  expect(existsSync(targetPath)).toBe(false);
}

async function runShell(command: string, env: NodeJS.ProcessEnv) {
  const child = spawnShellCommand(command, {
    cwd: repoRoot,
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });

  const stdout: Buffer[] = [];
  const stderr: Buffer[] = [];
  child.stdout.on("data", (chunk) => stdout.push(chunk));
  child.stderr.on("data", (chunk) => stderr.push(chunk));

  const code = await new Promise<number>((resolve, reject) => {
    child.on("error", reject);
    child.on("close", (exitCode) => resolve(exitCode ?? 1));
  });

  if (code !== 0) {
    throw new Error(Buffer.concat(stderr).toString("utf8"));
  }

  return Buffer.concat(stdout).toString("utf8").trim();
}
