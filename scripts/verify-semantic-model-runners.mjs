import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { access } from "node:fs/promises";
import { homedir } from "node:os";
import { resolve } from "node:path";

const repoRoot = process.cwd();
const embeddingRunnerPath = resolve(
  repoRoot,
  "scripts/evidence-embedding-runner.mjs",
);
const nliRunnerPath = resolve(repoRoot, "scripts/evidence-nli-runner.mjs");
const embeddingModelRoot = resolve(
  homedir(),
  ".touch-browser/models/evidence/embedding",
);
const nliModelRoot = resolve(homedir(), ".touch-browser/models/evidence/nli");

await access(resolve(embeddingModelRoot, ".ready.json"));
await access(resolve(nliModelRoot, ".ready.json"));

const embeddingResponse = await runJsonRunner(
  embeddingRunnerPath,
  {
    TOUCH_BROWSER_EVIDENCE_EMBEDDING_MODEL_PATH: embeddingModelRoot,
  },
  {
    modelId: "Xenova/multilingual-e5-small",
    texts: [
      "query: Rust memory safety",
      "passage: Rust has no runtime or garbage collector.",
    ],
  },
);
assert.equal(embeddingResponse.embeddings.length, 2);
assert.equal(embeddingResponse.embeddings[0].length > 0, true);

const nliResponse = await runJsonRunner(
  nliRunnerPath,
  {
    TOUCH_BROWSER_EVIDENCE_NLI_MODEL_PATH: nliModelRoot,
  },
  {
    modelId: "Xenova/nli-deberta-v3-xsmall",
    pairs: [
      {
        premise: "Rust has no runtime or garbage collector.",
        hypothesis: "Rust uses a garbage collector.",
      },
    ],
  },
);
assert.equal(nliResponse.results.length, 1);
assert.equal(nliResponse.results[0].contradiction > 0.9, true);
assert.equal(nliResponse.results[0].entailment < 0.1, true);

async function runJsonRunner(scriptPath, extraEnv, payload) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn("node", [scriptPath], {
      cwd: repoRoot,
      env: {
        ...process.env,
        ...extraEnv,
      },
      stdio: ["pipe", "pipe", "pipe"],
    });

    const stdoutChunks = [];
    const stderrChunks = [];

    child.stdout.on("data", (chunk) => stdoutChunks.push(chunk));
    child.stderr.on("data", (chunk) => stderrChunks.push(chunk));
    child.on("error", reject);
    child.on("close", (code) => {
      if (code !== 0) {
        reject(
          new Error(
            `Runner failed with code ${code}: ${Buffer.concat(stderrChunks).toString("utf8")}`,
          ),
        );
        return;
      }

      try {
        resolvePromise(
          JSON.parse(Buffer.concat(stdoutChunks).toString("utf8")),
        );
      } catch (error) {
        reject(error);
      }
    });

    child.stdin.end(JSON.stringify(payload));
  });
}
