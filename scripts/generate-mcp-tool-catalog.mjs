import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

function requiredConst(properties, field) {
  const value = properties?.[field]?.const;
  if (value === undefined) {
    throw new Error(`MCP tool catalog schema is missing \`${field}\` const.`);
  }
  return value;
}

function stringSchema(extra = {}) {
  return {
    type: "string",
    ...extra,
  };
}

function booleanSchema(extra = {}) {
  return {
    type: "boolean",
    ...extra,
  };
}

function integerSchema(extra = {}) {
  return {
    type: "integer",
    ...extra,
  };
}

function numberSchema(extra = {}) {
  return {
    type: "number",
    ...extra,
  };
}

function arraySchema(items, extra = {}) {
  return {
    type: "array",
    items,
    ...extra,
  };
}

function objectSchema({
  properties = {},
  required = [],
  additionalProperties = false,
  ...rest
} = {}) {
  const schema = {
    type: "object",
    additionalProperties,
    ...rest,
  };
  if (Object.keys(properties).length > 0) {
    schema.properties = properties;
  }
  if (required.length > 0) {
    schema.required = required;
  }
  return schema;
}

function nullableStringSchema() {
  return {
    type: ["string", "null"],
  };
}

function anyObjectSchema() {
  return {
    type: "object",
    additionalProperties: true,
  };
}

function recordSchema(valueSchema) {
  return {
    type: "object",
    additionalProperties: valueSchema,
  };
}

function buildOutputSchemas() {
  const arbitraryObject = anyObjectSchema();

  const policySignalSchema = objectSchema({
    properties: {
      kind: stringSchema(),
      origin: stringSchema(),
      stableRef: stringSchema(),
      detail: stringSchema(),
    },
    required: ["kind", "origin", "detail"],
  });

  const policyReportSchema = objectSchema({
    properties: {
      decision: stringSchema(),
      sourceRisk: stringSchema(),
      riskClass: stringSchema(),
      blockedRefs: arraySchema(stringSchema()),
      signals: arraySchema(policySignalSchema),
      allowlistedDomains: arraySchema(stringSchema()),
    },
    required: ["decision", "sourceRisk", "riskClass", "blockedRefs", "signals"],
  });

  const sessionStateSchema = objectSchema({
    properties: {
      version: stringSchema(),
      sessionId: stringSchema(),
      mode: stringSchema(),
      status: stringSchema(),
      policyProfile: stringSchema(),
      currentUrl: nullableStringSchema(),
      openedAt: stringSchema(),
      updatedAt: stringSchema(),
      visitedUrls: arraySchema(stringSchema()),
      snapshotIds: arraySchema(stringSchema()),
      workingSetRefs: arraySchema(stringSchema()),
    },
    required: [
      "version",
      "sessionId",
      "mode",
      "status",
      "policyProfile",
      "openedAt",
      "updatedAt",
      "visitedUrls",
      "snapshotIds",
    ],
  });

  const actionResultSchema = objectSchema({
    properties: {
      version: stringSchema(),
      action: stringSchema(),
      status: stringSchema(),
      payloadType: stringSchema(),
      output: {
        type: ["object", "null"],
        additionalProperties: true,
      },
      diagnostics: {
        type: ["object", "null"],
        additionalProperties: true,
      },
      policy: policyReportSchema,
      failureKind: stringSchema(),
      message: stringSchema(),
    },
    required: ["version", "action", "status", "payloadType", "message"],
  });

  const snapshotDocumentSchema = objectSchema({
    properties: {
      version: stringSchema(),
      stableRefVersion: stringSchema(),
      source: objectSchema({
        properties: {
          sourceUrl: stringSchema(),
          sourceType: stringSchema(),
          title: stringSchema(),
        },
        required: ["sourceUrl", "sourceType"],
      }),
      budget: objectSchema({
        properties: {
          requestedTokens: integerSchema({ minimum: 1 }),
          estimatedTokens: integerSchema({ minimum: 0 }),
          emittedTokens: integerSchema({ minimum: 0 }),
          truncated: booleanSchema(),
        },
        required: [
          "requestedTokens",
          "estimatedTokens",
          "emittedTokens",
          "truncated",
        ],
      }),
      blocks: arraySchema(arbitraryObject),
    },
    required: ["version", "stableRefVersion", "source", "budget", "blocks"],
  });

  const evidenceReportSchema = objectSchema({
    properties: {
      version: stringSchema(),
      generatedAt: stringSchema(),
      source: objectSchema({
        properties: {
          sourceUrl: stringSchema(),
          sourceType: stringSchema(),
          sourceRisk: stringSchema(),
          sourceLabel: stringSchema(),
        },
        required: ["sourceUrl", "sourceType", "sourceRisk"],
      }),
      evidenceSupportedClaims: arraySchema(arbitraryObject),
      contradictedClaims: arraySchema(arbitraryObject),
      insufficientEvidenceClaims: arraySchema(arbitraryObject),
      needsMoreBrowsingClaims: arraySchema(arbitraryObject),
      claimOutcomes: arraySchema(arbitraryObject),
    },
    required: [
      "version",
      "generatedAt",
      "source",
      "evidenceSupportedClaims",
      "insufficientEvidenceClaims",
      "claimOutcomes",
    ],
  });

  const sessionSynthesisReportSchema = objectSchema({
    properties: {
      version: stringSchema(),
      sessionId: stringSchema(),
      generatedAt: stringSchema(),
      snapshotCount: integerSchema({ minimum: 0 }),
      evidenceReportCount: integerSchema({ minimum: 0 }),
      visitedUrls: arraySchema(stringSchema()),
      workingSetRefs: arraySchema(stringSchema()),
      synthesizedNotes: arraySchema(stringSchema()),
      evidenceSupportedClaims: arraySchema(arbitraryObject),
      contradictedClaims: arraySchema(arbitraryObject),
      insufficientEvidenceClaims: arraySchema(arbitraryObject),
      needsMoreBrowsingClaims: arraySchema(arbitraryObject),
    },
    required: [
      "version",
      "sessionId",
      "generatedAt",
      "snapshotCount",
      "evidenceReportCount",
      "visitedUrls",
      "workingSetRefs",
      "synthesizedNotes",
      "evidenceSupportedClaims",
      "insufficientEvidenceClaims",
    ],
  });

  const readViewRefSchema = objectSchema({
    properties: {
      id: stringSchema(),
      kind: stringSchema(),
      ref: stringSchema(),
    },
    required: ["id", "kind", "ref"],
  });

  const readViewSchema = objectSchema({
    properties: {
      sourceUrl: stringSchema(),
      sourceTitle: stringSchema(),
      markdownText: stringSchema(),
      approxTokens: integerSchema({ minimum: 0 }),
      charCount: integerSchema({ minimum: 0 }),
      lineCount: integerSchema({ minimum: 0 }),
      mainOnly: booleanSchema(),
      mainContentQuality: stringSchema(),
      mainContentReason: stringSchema(),
      mainContentHint: stringSchema(),
      refIndex: arraySchema(readViewRefSchema),
      sessionState: sessionStateSchema,
    },
    required: [
      "sourceUrl",
      "sourceTitle",
      "markdownText",
      "approxTokens",
      "charCount",
      "lineCount",
      "mainOnly",
      "mainContentQuality",
      "mainContentReason",
      "refIndex",
      "sessionState",
    ],
  });

  const extractResultSchema = objectSchema({
    properties: {
      open: actionResultSchema,
      extract: actionResultSchema,
      sessionState: sessionStateSchema,
    },
    required: ["open", "extract", "sessionState"],
  });

  const directPolicyResultSchema = objectSchema({
    properties: {
      policy: policyReportSchema,
      sessionState: sessionStateSchema,
    },
    required: ["policy", "sessionState"],
  });

  const searchResultItemSchema = objectSchema({
    properties: {
      rank: integerSchema({ minimum: 1 }),
      url: stringSchema(),
      title: stringSchema(),
      snippet: stringSchema(),
      domain: stringSchema(),
      officialLikely: booleanSchema(),
      recommendedSurface: stringSchema(),
      selectionScore: numberSchema(),
    },
    required: [
      "rank",
      "url",
      "title",
      "snippet",
      "domain",
      "officialLikely",
      "recommendedSurface",
      "selectionScore",
    ],
  });

  const nextActionHintSchema = objectSchema({
    properties: {
      action: stringSchema(),
      actor: stringSchema(),
      canAutoRun: booleanSchema(),
      detail: stringSchema(),
      headedRequired: booleanSchema(),
      resultRanks: arraySchema(integerSchema({ minimum: 1 })),
    },
    required: [
      "action",
      "actor",
      "canAutoRun",
      "detail",
      "headedRequired",
      "resultRanks",
    ],
  });

  const searchRecoveryAttemptSchema = objectSchema({
    properties: {
      engine: stringSchema(),
      status: stringSchema(),
    },
    required: ["engine", "status"],
  });

  const searchRecoverySchema = objectSchema({
    properties: {
      recovered: booleanSchema(),
      humanInterventionRequiredNow: booleanSchema(),
      finalEngine: stringSchema(),
      attempts: arraySchema(searchRecoveryAttemptSchema),
    },
    required: [
      "recovered",
      "humanInterventionRequiredNow",
      "finalEngine",
      "attempts",
    ],
  });

  const searchReportSchema = objectSchema({
    properties: {
      version: stringSchema(),
      status: stringSchema(),
      statusDetail: stringSchema(),
      query: stringSchema(),
      engine: stringSchema(),
      searchUrl: stringSchema(),
      finalUrl: stringSchema(),
      generatedAt: stringSchema(),
      resultCount: integerSchema({ minimum: 0 }),
      results: arraySchema(searchResultItemSchema),
      recommendedResultRanks: arraySchema(integerSchema({ minimum: 1 })),
      nextActionHints: arraySchema(nextActionHintSchema),
      recovery: searchRecoverySchema,
    },
    required: [
      "version",
      "status",
      "query",
      "engine",
      "searchUrl",
      "finalUrl",
      "generatedAt",
      "resultCount",
      "results",
      "recommendedResultRanks",
      "nextActionHints",
      "recovery",
    ],
  });

  const searchSessionResultSchema = objectSchema({
    properties: {
      browserContextDir: nullableStringSchema(),
      browserProfileDir: nullableStringSchema(),
      engine: stringSchema(),
      query: stringSchema(),
      result: searchReportSchema,
      search: searchReportSchema,
      resultCount: integerSchema({ minimum: 0 }),
      searchUrl: stringSchema(),
      sessionFile: stringSchema(),
      sessionState: sessionStateSchema,
    },
    required: [
      "engine",
      "query",
      "result",
      "search",
      "resultCount",
      "searchUrl",
      "sessionFile",
      "sessionState",
    ],
  });

  const sessionTabEnvelope = (resultSchema = arbitraryObject) =>
    objectSchema({
      properties: {
        sessionId: stringSchema(),
        tabId: stringSchema(),
        diagnostics: arbitraryObject,
        result: resultSchema,
      },
      required: ["sessionId", "tabId", "result"],
    });

  const searchOpenResultSchema = objectSchema({
    properties: {
      sessionId: stringSchema(),
      searchTabId: stringSchema(),
      openedTabId: stringSchema(),
      selectionStrategy: stringSchema(),
      selectedResult: searchResultItemSchema,
      diagnostics: arbitraryObject,
      result: arbitraryObject,
    },
    required: [
      "sessionId",
      "searchTabId",
      "openedTabId",
      "selectionStrategy",
      "selectedResult",
      "result",
    ],
  });

  const searchOpenedTabSchema = objectSchema({
    properties: {
      tabId: stringSchema(),
      selectedResult: searchResultItemSchema,
      diagnostics: arbitraryObject,
      result: arbitraryObject,
    },
    required: ["tabId", "selectedResult", "result"],
  });

  const searchOpenTopSchema = objectSchema({
    properties: {
      sessionId: stringSchema(),
      searchTabId: stringSchema(),
      openedCount: integerSchema({ minimum: 0 }),
      openedTabs: arraySchema(searchOpenedTabSchema),
    },
    required: ["sessionId", "searchTabId", "openedCount", "openedTabs"],
  });

  const tabSummarySchema = objectSchema({
    properties: {
      tabId: stringSchema(),
      active: booleanSchema(),
      sessionFile: stringSchema(),
      hasState: booleanSchema(),
      currentUrl: nullableStringSchema(),
      visitedUrlCount: integerSchema({ minimum: 0 }),
      snapshotCount: integerSchema({ minimum: 0 }),
      latestSearchQuery: nullableStringSchema(),
      latestSearchResultCount: integerSchema({ minimum: 0 }),
    },
    required: [
      "tabId",
      "active",
      "sessionFile",
      "hasState",
      "visitedUrlCount",
      "snapshotCount",
      "latestSearchResultCount",
    ],
  });

  const tabOpenSchema = objectSchema({
    properties: {
      sessionId: stringSchema(),
      activeTabId: stringSchema(),
      tab: tabSummarySchema,
    },
    required: ["sessionId", "activeTabId", "tab"],
  });

  const tabListSchema = objectSchema({
    properties: {
      sessionId: stringSchema(),
      activeTabId: nullableStringSchema(),
      tabs: arraySchema(tabSummarySchema),
    },
    required: ["sessionId", "activeTabId", "tabs"],
  });

  const tabSelectSchema = objectSchema({
    properties: {
      sessionId: stringSchema(),
      activeTabId: stringSchema(),
      tab: tabSummarySchema,
    },
    required: ["sessionId", "activeTabId", "tab"],
  });

  const tabCloseSchema = objectSchema({
    properties: {
      sessionId: stringSchema(),
      tabId: stringSchema(),
      removed: booleanSchema(),
      removedState: booleanSchema(),
      activeTabId: nullableStringSchema(),
      remainingTabCount: integerSchema({ minimum: 0 }),
    },
    required: [
      "sessionId",
      "tabId",
      "removed",
      "removedState",
      "activeTabId",
      "remainingTabCount",
    ],
  });

  const sessionApprovedSchema = objectSchema({
    properties: {
      sessionId: stringSchema(),
      approvedRisks: arraySchema(stringSchema()),
      policyProfile: stringSchema(),
    },
    required: ["sessionId", "approvedRisks", "policyProfile"],
  });

  const secretStoreSchema = objectSchema({
    properties: {
      sessionId: stringSchema(),
      stored: booleanSchema(),
      targetRef: stringSchema(),
      secretCount: integerSchema({ minimum: 0 }),
    },
    required: ["sessionId", "stored", "targetRef", "secretCount"],
  });

  const secretClearSchema = objectSchema({
    properties: {
      sessionId: stringSchema(),
      removed: booleanSchema(),
      secretCount: integerSchema({ minimum: 0 }),
    },
    required: ["sessionId", "removed", "secretCount"],
  });

  const sessionSynthesizeSchema = objectSchema({
    properties: {
      sessionId: stringSchema(),
      activeTabId: nullableStringSchema(),
      tabCount: integerSchema({ minimum: 0 }),
      format: stringSchema(),
      markdown: {
        type: ["string", "null"],
      },
      report: sessionSynthesisReportSchema,
      tabReports: arraySchema(
        objectSchema({
          properties: {
            tabId: stringSchema(),
            report: sessionSynthesisReportSchema,
          },
          required: ["tabId", "report"],
        }),
      ),
    },
    required: [
      "sessionId",
      "activeTabId",
      "tabCount",
      "format",
      "report",
      "tabReports",
    ],
  });

  const sessionCloseSchema = objectSchema({
    properties: {
      sessionId: stringSchema(),
      removed: booleanSchema(),
      removedTabs: integerSchema({ minimum: 0 }),
    },
    required: ["sessionId", "removed", "removedTabs"],
  });

  const telemetryAggregateSchema = objectSchema({
    properties: {
      dbPath: stringSchema(),
      distinctSessionCount: integerSchema({ minimum: 0 }),
      latestRecordedAtMs: integerSchema({ minimum: 0 }),
      operationCounts: recordSchema(integerSchema({ minimum: 0 })),
      statusCounts: recordSchema(integerSchema({ minimum: 0 })),
      surfaceCounts: recordSchema(integerSchema({ minimum: 0 })),
      totalEvents: integerSchema({ minimum: 0 }),
    },
    required: [
      "dbPath",
      "distinctSessionCount",
      "latestRecordedAtMs",
      "operationCounts",
      "statusCounts",
      "surfaceCounts",
      "totalEvents",
    ],
  });

  const telemetryEventSchema = objectSchema({
    properties: {
      operation: stringSchema(),
      payload: arbitraryObject,
      recordedAtMs: integerSchema({ minimum: 0 }),
      status: stringSchema(),
      surface: stringSchema(),
      sessionId: stringSchema(),
      policyProfile: stringSchema(),
      policyDecision: stringSchema(),
      riskClass: stringSchema(),
      currentUrl: stringSchema(),
    },
    required: ["operation", "recordedAtMs", "status", "surface"],
    additionalProperties: true,
  });

  const telemetrySummarySchema = objectSchema({
    properties: {
      result: telemetryAggregateSchema,
      summary: telemetryAggregateSchema,
    },
    required: ["result", "summary"],
  });

  const telemetryRecentSchema = objectSchema({
    properties: {
      limit: integerSchema({ minimum: 1 }),
      events: arraySchema(telemetryEventSchema),
      result: arraySchema(telemetryEventSchema),
    },
    required: ["limit", "events", "result"],
  });

  return {
    tb_status: objectSchema({
      properties: {
        status: stringSchema(),
        transport: stringSchema(),
        version: stringSchema(),
        daemon: booleanSchema(),
        methods: arraySchema(stringSchema()),
      },
      required: ["status", "transport", "version", "daemon", "methods"],
    }),
    tb_session_create: objectSchema({
      properties: {
        sessionId: stringSchema(),
        activeTabId: stringSchema(),
        headless: booleanSchema(),
        allowDomains: arraySchema(stringSchema()),
        tabCount: integerSchema({ minimum: 1 }),
      },
      required: [
        "sessionId",
        "activeTabId",
        "headless",
        "allowDomains",
        "tabCount",
      ],
    }),
    tb_open: {
      anyOf: [actionResultSchema, sessionTabEnvelope(actionResultSchema)],
    },
    tb_search: sessionTabEnvelope(searchSessionResultSchema),
    tb_search_open_result: searchOpenResultSchema,
    tb_search_open_top: searchOpenTopSchema,
    tb_extract: {
      anyOf: [extractResultSchema, sessionTabEnvelope(arbitraryObject)],
    },
    tb_read_view: {
      anyOf: [readViewSchema, sessionTabEnvelope(arbitraryObject)],
    },
    tb_policy: {
      anyOf: [directPolicyResultSchema, sessionTabEnvelope(arbitraryObject)],
    },
    tb_tab_open: tabOpenSchema,
    tb_tab_list: tabListSchema,
    tb_tab_select: tabSelectSchema,
    tb_tab_close: tabCloseSchema,
    tb_checkpoint: sessionTabEnvelope(arbitraryObject),
    tb_profile: sessionTabEnvelope(arbitraryObject),
    tb_profile_set: sessionTabEnvelope(arbitraryObject),
    tb_click: sessionTabEnvelope(arbitraryObject),
    tb_type: sessionTabEnvelope(arbitraryObject),
    tb_approve: sessionApprovedSchema,
    tb_secret_store: secretStoreSchema,
    tb_secret_clear: secretClearSchema,
    tb_type_secret: sessionTabEnvelope(arbitraryObject),
    tb_submit: sessionTabEnvelope(arbitraryObject),
    tb_refresh: sessionTabEnvelope(arbitraryObject),
    tb_telemetry_summary: telemetrySummarySchema,
    tb_telemetry_recent: telemetryRecentSchema,
    tb_session_synthesize: sessionSynthesizeSchema,
    tb_session_close: sessionCloseSchema,
  };
}

function extractToolCatalog(schema) {
  if (schema.$id !== "mcp-tool-catalog.schema.json") {
    throw new Error("Unexpected MCP tool catalog schema id.");
  }
  if (schema.type !== "array" || !Array.isArray(schema.prefixItems)) {
    throw new Error(
      "MCP tool catalog schema must be an array with prefixItems.",
    );
  }

  const outputSchemas = buildOutputSchemas();
  return schema.prefixItems.map((item) => {
    const properties = item?.properties;
    const name = requiredConst(properties, "name");
    const outputSchema = outputSchemas[name];
    if (!outputSchema) {
      throw new Error(
        `MCP tool catalog output schema is missing for \`${name}\`.`,
      );
    }
    return {
      name,
      title: requiredConst(properties, "title"),
      description: requiredConst(properties, "description"),
      inputSchema: requiredConst(properties, "inputSchema"),
      outputSchema,
    };
  });
}

export async function generateMcpToolCatalog(root) {
  const schemaPath = path.join(
    root,
    "contracts",
    "schemas",
    "mcp-tool-catalog.schema.json",
  );
  const generatedDir = path.join(root, "contracts", "generated");
  const generatedJsonPath = path.join(generatedDir, "mcp-tool-catalog.json");
  const generatedModulePath = path.join(generatedDir, "mcp-tool-catalog.mjs");
  const schema = JSON.parse(await readFile(schemaPath, "utf8"));
  const toolCatalog = extractToolCatalog(schema);

  await mkdir(generatedDir, { recursive: true });
  await writeFile(
    `${generatedJsonPath}`,
    `${JSON.stringify(toolCatalog, null, 2)}\n`,
  );
  await writeFile(
    generatedModulePath,
    `export const toolCatalog = ${JSON.stringify(toolCatalog, null, 2)};\n`,
  );

  return {
    schema: path.relative(root, schemaPath),
    generatedJson: path.relative(root, generatedJsonPath),
    generatedModule: path.relative(root, generatedModulePath),
    toolCount: toolCatalog.length,
  };
}

async function main() {
  const result = await generateMcpToolCatalog(process.cwd());
  console.log(JSON.stringify({ status: "ok", ...result }, null, 2));
}

if (import.meta.url === new URL(process.argv[1], "file:").href) {
  try {
    await main();
  } catch (error) {
    console.error(
      JSON.stringify(
        {
          status: "error",
          message: error instanceof Error ? error.message : String(error),
        },
        null,
        2,
      ),
    );
    process.exitCode = 1;
  }
}
