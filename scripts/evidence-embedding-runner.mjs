import { pipeline } from "@huggingface/transformers";

import {
  parseModelRunnerArgs,
  prepareModelRuntime,
  readJsonPayloadFromStdin,
  writeReadyMarker,
} from "./lib/model-runner.mjs";

const DEFAULT_MODEL_ID = "Xenova/multilingual-e5-small";
const DEFAULT_MODEL_ROOT = `${process.env.HOME}/.touch-browser/models/evidence/embedding`;

async function main() {
  const options = parseModelRunnerArgs(process.argv.slice(2));
  const { modelId, modelRoot } = await prepareModelRuntime({
    allowDownload: options.allowDownload,
    cliModelId: options.modelId,
    cliModelRoot: options.modelRoot,
    defaultModelId: DEFAULT_MODEL_ID,
    defaultModelRoot: DEFAULT_MODEL_ROOT,
    envModelIdKey: "TOUCH_BROWSER_EVIDENCE_EMBEDDING_MODEL_ID",
    envModelRootKey: "TOUCH_BROWSER_EVIDENCE_EMBEDDING_MODEL_PATH",
  });

  const extractor = await pipeline("feature-extraction", modelId, {
    quantized: true,
  });

  if (options.warmup) {
    await extractor(["query: warmup", "passage: warmup"], {
      pooling: "mean",
      normalize: true,
    });
    await writeReadyMarker(modelRoot, modelId);
    process.stdout.write(
      `${JSON.stringify({ status: "ok", modelId, modelRoot })}\n`,
    );
    return;
  }

  const payload = await readJsonPayloadFromStdin();
  const output = await extractor(payload.texts, {
    pooling: "mean",
    normalize: true,
  });
  process.stdout.write(
    `${JSON.stringify({ modelId, embeddings: tensorRows(output) })}\n`,
  );
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

await main();
