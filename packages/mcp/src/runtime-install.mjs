import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";

import { x as extractTar } from "tar";

const REPOSITORY = "nangman-infra/touch-browser";
const PACKAGE_JSON_PATH = fileURLToPath(
  new URL("../package.json", import.meta.url),
);
const USER_AGENT = "touch-browser-mcp-installer";

export async function readPackageMetadata() {
  return JSON.parse(await fs.promises.readFile(PACKAGE_JSON_PATH, "utf8"));
}

export function resolveRuntimeTarget() {
  const platform = (() => {
    switch (process.platform) {
      case "darwin":
        return "macos";
      case "linux":
        return "linux";
      default:
        throw new Error(
          `Unsupported platform for @nangman-infra/touch-browser-mcp: ${process.platform}`,
        );
    }
  })();

  const arch = (() => {
    switch (process.arch) {
      case "x64":
        return "x86_64";
      case "arm64":
        return "arm64";
      default:
        throw new Error(
          `Unsupported architecture for @nangman-infra/touch-browser-mcp: ${process.arch}`,
        );
    }
  })();

  return { platform, arch };
}

export function resolveDataRoot(env = process.env) {
  const configured = env.TOUCH_BROWSER_DATA_ROOT?.trim();
  if (!configured) {
    return path.join(os.homedir(), ".touch-browser");
  }
  if (configured === "~") {
    return os.homedir();
  }
  if (configured.startsWith("~/")) {
    return path.join(os.homedir(), configured.slice(2));
  }
  return configured;
}

export async function resolveRuntimeDescriptor(env = process.env) {
  const metadata = await readPackageMetadata();
  const { platform, arch } = resolveRuntimeTarget();
  const tag = `v${metadata.version}`;
  const bundleName = `touch-browser-${tag}-${platform}-${arch}`;
  const assetName = `${bundleName}.tar.gz`;
  const checksumName = `${bundleName}.sha256`;
  const releaseBaseUrl =
    env.TOUCH_BROWSER_MCP_RELEASE_BASE_URL?.trim() ||
    `https://github.com/${REPOSITORY}/releases/download/${tag}`;
  const dataRoot = resolveDataRoot(env);
  const versionsRoot = path.join(dataRoot, "npm-mcp", "versions");
  const bundleRoot = path.join(versionsRoot, bundleName);
  const binaryPath = path.join(bundleRoot, "bin", "touch-browser");
  const lockPath = path.join(versionsRoot, `${bundleName}.lock`);

  return {
    metadata,
    tag,
    platform,
    arch,
    bundleName,
    assetName,
    checksumName,
    releaseBaseUrl,
    assetUrl: `${releaseBaseUrl}/${assetName}`,
    checksumUrl: `${releaseBaseUrl}/${checksumName}`,
    dataRoot,
    versionsRoot,
    bundleRoot,
    binaryPath,
    lockPath,
  };
}

export async function ensureRuntimeInstalled({
  env = process.env,
  stderr = process.stderr,
} = {}) {
  const descriptor = await resolveRuntimeDescriptor(env);

  if (await isExecutable(descriptor.binaryPath)) {
    return descriptor;
  }

  await fs.promises.mkdir(descriptor.versionsRoot, { recursive: true });
  const releaseLock = await acquireLock(descriptor.lockPath, stderr);
  let tempRoot;

  try {
    if (await isExecutable(descriptor.binaryPath)) {
      return descriptor;
    }

    tempRoot = await fs.promises.mkdtemp(
      path.join(os.tmpdir(), "touch-browser-mcp-"),
    );
    const archivePath = path.join(tempRoot, descriptor.assetName);

    stderr.write(
      `[touch-browser-mcp] Installing ${descriptor.bundleName} from GitHub Releases...\n`,
    );
    await downloadFile(descriptor.assetUrl, archivePath);
    const expectedSha = await downloadExpectedSha(descriptor.checksumUrl);
    const actualSha = await sha256File(archivePath);
    if (expectedSha !== actualSha) {
      throw new Error(
        `Checksum mismatch for ${descriptor.assetName}. expected=${expectedSha} actual=${actualSha}`,
      );
    }

    await fs.promises.rm(descriptor.bundleRoot, {
      recursive: true,
      force: true,
    });
    await extractTar({
      file: archivePath,
      cwd: descriptor.versionsRoot,
    });

    if (!(await isExecutable(descriptor.binaryPath))) {
      throw new Error(
        `Installed bundle did not produce an executable at ${descriptor.binaryPath}`,
      );
    }

    return descriptor;
  } finally {
    await releaseLock();
    if (tempRoot) {
      await fs.promises.rm(tempRoot, { recursive: true, force: true });
    }
  }
}

async function acquireLock(lockPath, stderr) {
  while (true) {
    try {
      await fs.promises.mkdir(lockPath);
      return async () => {
        await fs.promises.rm(lockPath, { recursive: true, force: true });
      };
    } catch (error) {
      if (error?.code !== "EEXIST") {
        throw error;
      }

      stderr.write(
        "[touch-browser-mcp] Waiting for an existing runtime install to finish...\n",
      );
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
  }
}

async function downloadExpectedSha(url) {
  const response = await fetch(url, {
    headers: {
      "user-agent": USER_AGENT,
    },
  });
  if (!response.ok) {
    throw new Error(`Failed to download checksum: ${url} (${response.status})`);
  }
  const checksumText = await response.text();
  const sha = checksumText.trim().split(/\s+/)[0];
  if (!sha) {
    throw new Error(`Checksum file at ${url} was empty.`);
  }
  return sha;
}

async function downloadFile(url, destinationPath) {
  const response = await fetch(url, {
    headers: {
      "user-agent": USER_AGENT,
    },
  });
  if (!response.ok || !response.body) {
    throw new Error(
      `Failed to download release asset: ${url} (${response.status})`,
    );
  }

  const destination = fs.createWriteStream(destinationPath);
  await pipeline(Readable.fromWeb(response.body), destination);
}

async function sha256File(filePath) {
  const hash = createHash("sha256");
  const stream = fs.createReadStream(filePath);
  for await (const chunk of stream) {
    hash.update(chunk);
  }
  return hash.digest("hex");
}

async function isExecutable(filePath) {
  try {
    await fs.promises.access(filePath, fs.constants.X_OK);
    return true;
  } catch {
    return false;
  }
}
