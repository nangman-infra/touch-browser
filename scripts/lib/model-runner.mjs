import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

import { env } from "@huggingface/transformers";

export async function prepareModelRuntime({
  allowDownload,
  cliModelId,
  cliModelRoot,
  defaultModelId,
  defaultModelRoot,
  envModelIdKey,
  envModelRootKey,
}) {
  const modelId = cliModelId ?? process.env[envModelIdKey] ?? defaultModelId;
  const modelRoot = resolve(
    cliModelRoot ?? process.env[envModelRootKey] ?? defaultModelRoot,
  );

  await mkdir(modelRoot, { recursive: true });
  configureEnvironment(modelRoot, allowDownload);

  return { modelId, modelRoot };
}

export function configureEnvironment(modelRoot, allowDownload) {
  env.cacheDir = modelRoot;
  env.useFSCache = true;
  env.allowLocalModels = true;
  env.allowRemoteModels = allowDownload;
}

export function parseModelRunnerArgs(args) {
  const options = {
    warmup: false,
    allowDownload: false,
    modelRoot: null,
    modelId: null,
  };

  const remainingArgs = [...args];
  while (remainingArgs.length > 0) {
    const value = remainingArgs.shift();
    if (value === "--warmup") {
      options.warmup = true;
    } else if (value === "--allow-download") {
      options.allowDownload = true;
    } else if (value === "--model-root") {
      options.modelRoot = remainingArgs.shift() ?? null;
    } else if (value === "--model-id") {
      options.modelId = remainingArgs.shift() ?? null;
    }
  }

  return options;
}

export async function readJsonPayloadFromStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk);
  }

  return JSON.parse(Buffer.concat(chunks).toString("utf8"));
}

export async function writeReadyMarker(modelRoot, modelId) {
  await writeFile(
    resolve(modelRoot, ".ready.json"),
    `${JSON.stringify({ modelId, warmedAt: new Date().toISOString() }, null, 2)}\n`,
  );
}
