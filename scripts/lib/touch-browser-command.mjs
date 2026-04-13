import fs from "node:fs";
import path from "node:path";

const SHELL_SINGLE_QUOTE_ESCAPE = ["'", '"', "'", '"', "'"].join("");

const REPO_BINARY_CANDIDATES = [
  ["bin", "touch-browser"],
  ["dist", "touch-browser"],
  ["target", "release", "touch-browser"],
  ["target", "debug", "touch-browser"],
];

const REPO_CHECKOUT_BINARY_CANDIDATES = [
  ["target", "debug", "touch-browser"],
  ["target", "release", "touch-browser"],
  ["bin", "touch-browser"],
  ["dist", "touch-browser"],
];

export function resolveTouchBrowserBinaryPath({
  cwd = process.cwd(),
  env = process.env,
} = {}) {
  const explicitBinary = resolveExplicitBinary(env);
  if (explicitBinary && isFile(explicitBinary)) {
    return explicitBinary;
  }

  if (looksLikeRepoCheckout(cwd)) {
    const repoBinary = resolveRepoBinary(cwd, REPO_CHECKOUT_BINARY_CANDIDATES);
    if (repoBinary) {
      return repoBinary;
    }
  }

  const pathMatch = resolveBinaryFromPath(env.PATH);
  if (pathMatch) {
    return pathMatch;
  }

  const standaloneMatch = resolveStandaloneBundleBinary(cwd);
  if (standaloneMatch) {
    return standaloneMatch;
  }

  for (const candidate of REPO_BINARY_CANDIDATES) {
    const absolutePath = resolveRepoBinary(cwd, [candidate]);
    if (absolutePath) {
      return absolutePath;
    }
  }

  return null;
}

export function resolveTouchBrowserServeCommand({
  cwd = process.cwd(),
  env = process.env,
} = {}) {
  const override = env.TOUCH_BROWSER_SERVE_COMMAND?.trim();
  if (override) {
    return override;
  }

  const binaryPath = resolveTouchBrowserBinaryPath({ cwd, env });
  if (binaryPath) {
    return `${shellEscape(binaryPath)} serve`;
  }

  throw new Error(buildMissingBinaryMessage(cwd));
}

export function buildMissingBinaryMessage(cwd = process.cwd()) {
  return [
    "Could not resolve a touch-browser binary for serve mode.",
    `Checked install and build paths from ${cwd}.`,
    "Install a standalone release bundle and run its install.sh, or build the repo once so target/release or target/debug contains touch-browser.",
    "You can also set TOUCH_BROWSER_SERVE_COMMAND or TOUCH_BROWSER_SERVE_BINARY explicitly.",
  ].join(" ");
}

function resolveExplicitBinary(env) {
  for (const key of ["TOUCH_BROWSER_SERVE_BINARY", "TOUCH_BROWSER_BINARY"]) {
    const value = env[key]?.trim();
    if (value) {
      return expandHome(value);
    }
  }
  return null;
}

function resolveBinaryFromPath(pathValue) {
  if (!pathValue) {
    return null;
  }

  for (const segment of pathValue.split(path.delimiter)) {
    if (!segment) {
      continue;
    }
    const binaryPath = path.join(expandHome(segment), "touch-browser");
    if (isFile(binaryPath)) {
      return binaryPath;
    }
  }

  return null;
}

function resolveStandaloneBundleBinary(cwd) {
  const standaloneRoot = path.resolve(cwd, "dist", "standalone");
  if (!isDirectory(standaloneRoot)) {
    return null;
  }

  const bundleDirs = fs
    .readdirSync(standaloneRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(standaloneRoot, entry.name))
    .sort(compareByModifiedTimeDesc);

  for (const bundleDir of bundleDirs) {
    const binaryPath = path.join(bundleDir, "bin", "touch-browser");
    if (isFile(binaryPath)) {
      return binaryPath;
    }
  }

  return null;
}

function resolveRepoBinary(cwd, candidates) {
  for (const candidate of candidates) {
    const absolutePath = path.resolve(cwd, ...candidate);
    if (isFile(absolutePath)) {
      return absolutePath;
    }
  }
  return null;
}

function looksLikeRepoCheckout(cwd) {
  return (
    isFile(path.resolve(cwd, "Cargo.toml")) &&
    isFile(path.resolve(cwd, "package.json")) &&
    isDirectory(path.resolve(cwd, "core")) &&
    isDirectory(path.resolve(cwd, "integrations"))
  );
}

function compareByModifiedTimeDesc(left, right) {
  const rightTime = fs.statSync(right).mtimeMs;
  const leftTime = fs.statSync(left).mtimeMs;
  return rightTime - leftTime;
}

function expandHome(value) {
  if (!value || value === "~") {
    return process.env.HOME ?? value;
  }

  if (value.startsWith("~/")) {
    return path.join(process.env.HOME ?? "~", value.slice(2));
  }

  return value;
}

function isDirectory(targetPath) {
  try {
    return fs.statSync(targetPath).isDirectory();
  } catch {
    return false;
  }
}

function isFile(targetPath) {
  try {
    return fs.statSync(targetPath).isFile();
  } catch {
    return false;
  }
}

function shellEscape(value) {
  const escapedValue = String(value).replaceAll("'", SHELL_SINGLE_QUOTE_ESCAPE);
  return `'${escapedValue}'`;
}
