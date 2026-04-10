import {
  AutoModelForSequenceClassification,
  AutoTokenizer,
} from "@huggingface/transformers";

import {
  parseModelRunnerArgs,
  prepareModelRuntime,
  readJsonPayloadFromStdin,
  writeReadyMarker,
} from "./lib/model-runner.mjs";

const DEFAULT_MODEL_ID = "Xenova/nli-deberta-v3-xsmall";
const DEFAULT_MODEL_ROOT = `${process.env.HOME}/.touch-browser/models/evidence/nli`;

async function main() {
  const options = parseModelRunnerArgs(process.argv.slice(2));
  const { modelId, modelRoot } = await prepareModelRuntime({
    allowDownload: options.allowDownload,
    cliModelId: options.modelId,
    cliModelRoot: options.modelRoot,
    defaultModelId: DEFAULT_MODEL_ID,
    defaultModelRoot: DEFAULT_MODEL_ROOT,
    envModelIdKey: "TOUCH_BROWSER_EVIDENCE_NLI_MODEL_ID",
    envModelRootKey: "TOUCH_BROWSER_EVIDENCE_NLI_MODEL_PATH",
  });

  const tokenizer = await AutoTokenizer.from_pretrained(modelId);
  const model = await AutoModelForSequenceClassification.from_pretrained(
    modelId,
    {
      dtype: "fp32",
    },
  );

  if (options.warmup) {
    await writeReadyMarker(modelRoot, modelId);
    process.stdout.write(
      `${JSON.stringify({ status: "ok", modelId, modelRoot })}\n`,
    );
    return;
  }

  const payload = await readJsonPayloadFromStdin();
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

function softmax(logits) {
  const max = Math.max(...logits);
  const exps = logits.map((value) => Math.exp(value - max));
  const total = exps.reduce((sum, value) => sum + value, 0);
  return exps.map((value) => value / total);
}

await main();
