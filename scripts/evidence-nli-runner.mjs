import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

import {
  AutoModelForSequenceClassification,
  AutoTokenizer,
  env,
} from "@huggingface/transformers";

const DEFAULT_MODEL_ID = "Xenova/nli-deberta-v3-xsmall";

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const modelId =
    options.modelId ??
    process.env.TOUCH_BROWSER_EVIDENCE_NLI_MODEL_ID ??
    DEFAULT_MODEL_ID;
  const modelRoot = resolve(
    options.modelRoot ??
      process.env.TOUCH_BROWSER_EVIDENCE_NLI_MODEL_PATH ??
      `${process.env.HOME}/.touch-browser/models/evidence/nli`,
  );

  await mkdir(modelRoot, { recursive: true });
  configureEnvironment(modelRoot, options.allowDownload);

  const tokenizer = await AutoTokenizer.from_pretrained(modelId);
  const model = await AutoModelForSequenceClassification.from_pretrained(
    modelId,
    {
      dtype: "fp32",
    },
  );

  if (options.warmup) {
    await writeMarker(modelRoot, modelId);
    process.stdout.write(
      `${JSON.stringify({ status: "ok", modelId, modelRoot })}\n`,
    );
    return;
  }

  const payload = JSON.parse(await readStdin());
  const premises = payload.pairs.map((pair) => pair.premise);
  const hypotheses = payload.pairs.map((pair) => pair.hypothesis);
  const inputs = await tokenizer(premises, {
    text_pair: hypotheses,
    truncation: true,
    padding: true,
  });
  const output = await model(inputs);
  const dims = output.logits.dims;
  const data = Array.from(output.logits.data);
  const stride = dims.at(-1) ?? 3;
  const results = [];

  for (let row = 0; row < dims[0]; row += 1) {
    const offset = row * stride;
    const scores = softmax(data.slice(offset, offset + stride));
    results.push({
      contradiction: scores[0] ?? 0,
      entailment: scores[1] ?? 0,
      neutral: scores[2] ?? 0,
    });
  }

  process.stdout.write(`${JSON.stringify({ modelId, results })}\n`);
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

function softmax(logits) {
  const max = Math.max(...logits);
  const exps = logits.map((value) => Math.exp(value - max));
  const total = exps.reduce((sum, value) => sum + value, 0);
  return exps.map((value) => value / total);
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
