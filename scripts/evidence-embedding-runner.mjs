import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

import { env, pipeline } from "@huggingface/transformers";

const DEFAULT_MODEL_ID = "Xenova/multilingual-e5-small";

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const modelId =
    options.modelId ??
    process.env.TOUCH_BROWSER_EVIDENCE_EMBEDDING_MODEL_ID ??
    DEFAULT_MODEL_ID;
  const modelRoot = resolve(
    options.modelRoot ??
      process.env.TOUCH_BROWSER_EVIDENCE_EMBEDDING_MODEL_PATH ??
      `${process.env.HOME}/.touch-browser/models/evidence/embedding`,
  );

  await mkdir(modelRoot, { recursive: true });
  configureEnvironment(modelRoot, options.allowDownload);

  const extractor = await pipeline("feature-extraction", modelId, {
    quantized: true,
  });

  if (options.warmup) {
    await extractor(["query: warmup", "passage: warmup"], {
      pooling: "mean",
      normalize: true,
    });
    await writeMarker(modelRoot, modelId);
    process.stdout.write(
      `${JSON.stringify({ status: "ok", modelId, modelRoot })}\n`,
    );
    return;
  }

  const payload = JSON.parse(await readStdin());
  const output = await extractor(payload.texts, {
    pooling: "mean",
    normalize: true,
  });
  process.stdout.write(
    `${JSON.stringify({ modelId, embeddings: tensorRows(output) })}\n`,
  );
}

function configureEnvironment(modelRoot, allowDownload) {
  env.cacheDir = modelRoot;
  env.useFSCache = true;
  env.allowLocalModels = false;
  env.allowRemoteModels = allowDownload;
}

function parseArgs(args) {
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

function tensorRows(tensor) {
  const dims = tensor.dims ?? [];
  const stride = dims.at(-1) ?? 0;
  if (stride === 0) {
    return [];
  }

  if (dims.length === 1) {
    return [Array.from(tensor.data)];
  }

  const rowCount = dims[0] ?? 0;
  const rows = [];
  for (let row = 0; row < rowCount; row += 1) {
    const offset = row * stride;
    rows.push(Array.from(tensor.data.slice(offset, offset + stride)));
  }
  return rows;
}

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk);
  }
  return Buffer.concat(chunks).toString("utf8");
}

async function writeMarker(modelRoot, modelId) {
  await writeFile(
    resolve(modelRoot, ".ready.json"),
    `${JSON.stringify({ modelId, warmedAt: new Date().toISOString() }, null, 2)}\n`,
  );
}

await main();
