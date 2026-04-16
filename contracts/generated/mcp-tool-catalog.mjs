export const toolCatalog = [
  {
    "name": "tb_status",
    "title": "Touch Browser Status",
    "description": "Return runtime and daemon capability status.",
    "inputSchema": {
      "type": "object",
      "properties": {}
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "status": {
          "type": "string"
        },
        "transport": {
          "type": "string"
        },
        "version": {
          "type": "string"
        },
        "daemon": {
          "type": "boolean"
        },
        "methods": {
          "type": "array",
          "items": {
            "type": "string"
          }
        }
      },
      "required": [
        "status",
        "transport",
        "version",
        "daemon",
        "methods"
      ]
    }
  },
  {
    "name": "tb_session_create",
    "title": "Create Browser Session",
    "description": "Create a headless research session for public docs and reference workflows.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "allowDomains": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "Domains this session is allowed to visit."
        }
      }
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "activeTabId": {
          "type": "string"
        },
        "headless": {
          "type": "boolean"
        },
        "allowDomains": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "tabCount": {
          "type": "integer",
          "minimum": 1
        }
      },
      "required": [
        "sessionId",
        "activeTabId",
        "headless",
        "allowDomains",
        "tabCount"
      ]
    }
  },
  {
    "name": "tb_open",
    "title": "Open Target",
    "description": "Open a public web document or official reference page headlessly, either statelessly or inside a daemon session/tab. Use this for direct URLs after narrowing scope.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "target": {
          "type": "string",
          "description": "The URL or public web target to open."
        },
        "browser": {
          "type": "boolean",
          "description": "When true, open the target through the browser-backed acquisition path."
        },
        "budget": {
          "type": "number",
          "description": "The processing budget to spend on this operation."
        },
        "sourceRisk": {
          "type": "string",
          "description": "The source risk label to attach to the result or citation."
        },
        "sourceLabel": {
          "type": "string",
          "description": "The human-readable source label to attach to the result or citation."
        },
        "verifierCommand": {
          "type": "string",
          "description": "An optional verifier command to run for this operation."
        },
        "allowDomains": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "Restrict navigation for this open request to these domains."
        },
        "sessionId": {
          "type": "string",
          "description": "An existing session to reuse for this open request."
        },
        "tabId": {
          "type": "string",
          "description": "A specific session tab to reuse for this open request."
        }
      },
      "required": [
        "target"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "minProperties": 1,
      "properties": {
        "version": {
          "type": "string"
        },
        "action": {
          "type": "string"
        },
        "status": {
          "type": "string"
        },
        "payloadType": {
          "type": "string"
        },
        "output": {
          "type": [
            "object",
            "null"
          ],
          "additionalProperties": true
        },
        "diagnostics": {
          "type": "object",
          "additionalProperties": true
        },
        "policy": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "decision": {
              "type": "string"
            },
            "sourceRisk": {
              "type": "string"
            },
            "riskClass": {
              "type": "string"
            },
            "blockedRefs": {
              "type": "array",
              "items": {
                "type": "string"
              }
            },
            "signals": {
              "type": "array",
              "items": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                  "kind": {
                    "type": "string"
                  },
                  "origin": {
                    "type": "string"
                  },
                  "stableRef": {
                    "type": "string"
                  },
                  "detail": {
                    "type": "string"
                  }
                },
                "required": [
                  "kind",
                  "origin",
                  "detail"
                ]
              }
            },
            "allowlistedDomains": {
              "type": "array",
              "items": {
                "type": "string"
              }
            }
          },
          "required": [
            "decision",
            "sourceRisk",
            "riskClass",
            "blockedRefs",
            "signals"
          ]
        },
        "failureKind": {
          "type": "string"
        },
        "message": {
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        },
        "tabId": {
          "type": "string"
        },
        "result": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "version": {
              "type": "string"
            },
            "action": {
              "type": "string"
            },
            "status": {
              "type": "string"
            },
            "payloadType": {
              "type": "string"
            },
            "output": {
              "type": [
                "object",
                "null"
              ],
              "additionalProperties": true
            },
            "diagnostics": {
              "type": [
                "object",
                "null"
              ],
              "additionalProperties": true
            },
            "policy": {
              "type": "object",
              "additionalProperties": false,
              "properties": {
                "decision": {
                  "type": "string"
                },
                "sourceRisk": {
                  "type": "string"
                },
                "riskClass": {
                  "type": "string"
                },
                "blockedRefs": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                },
                "signals": {
                  "type": "array",
                  "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                      "kind": {
                        "type": "string"
                      },
                      "origin": {
                        "type": "string"
                      },
                      "stableRef": {
                        "type": "string"
                      },
                      "detail": {
                        "type": "string"
                      }
                    },
                    "required": [
                      "kind",
                      "origin",
                      "detail"
                    ]
                  }
                },
                "allowlistedDomains": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                }
              },
              "required": [
                "decision",
                "sourceRisk",
                "riskClass",
                "blockedRefs",
                "signals"
              ]
            },
            "failureKind": {
              "type": "string"
            },
            "message": {
              "type": "string"
            }
          },
          "required": [
            "version",
            "action",
            "status",
            "payloadType",
            "message"
          ]
        }
      }
    }
  },
  {
    "name": "tb_search",
    "title": "Search The Web",
    "description": "Search the public docs/research web headlessly and structure the result page for follow-up browsing. Search engine selection is automatic; follow tb_search_open_top before read-view or extract.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The session that will run the web search."
        },
        "tabId": {
          "type": "string",
          "description": "An existing tab to reuse for the search results page."
        },
        "query": {
          "type": "string",
          "description": "The search query to run."
        },
        "budget": {
          "type": "number",
          "description": "The processing budget to spend on this operation."
        }
      },
      "required": [
        "sessionId",
        "query"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "tabId": {
          "type": "string"
        },
        "diagnostics": {
          "type": "object",
          "additionalProperties": true
        },
        "result": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "browserContextDir": {
              "type": [
                "string",
                "null"
              ]
            },
            "browserProfileDir": {
              "type": [
                "string",
                "null"
              ]
            },
            "engine": {
              "type": "string"
            },
            "query": {
              "type": "string"
            },
            "result": {
              "type": "object",
              "additionalProperties": false,
              "properties": {
                "version": {
                  "type": "string"
                },
                "status": {
                  "type": "string"
                },
                "statusDetail": {
                  "type": "string"
                },
                "query": {
                  "type": "string"
                },
                "engine": {
                  "type": "string"
                },
                "searchUrl": {
                  "type": "string"
                },
                "finalUrl": {
                  "type": "string"
                },
                "generatedAt": {
                  "type": "string"
                },
                "resultCount": {
                  "type": "integer",
                  "minimum": 0
                },
                "results": {
                  "type": "array",
                  "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                      "rank": {
                        "type": "integer",
                        "minimum": 1
                      },
                      "url": {
                        "type": "string"
                      },
                      "title": {
                        "type": "string"
                      },
                      "snippet": {
                        "type": "string"
                      },
                      "domain": {
                        "type": "string"
                      },
                      "officialLikely": {
                        "type": "boolean"
                      },
                      "recommendedSurface": {
                        "type": "string"
                      },
                      "selectionScore": {
                        "type": "number"
                      }
                    },
                    "required": [
                      "rank",
                      "url",
                      "title",
                      "snippet",
                      "domain",
                      "officialLikely",
                      "recommendedSurface",
                      "selectionScore"
                    ]
                  }
                },
                "recommendedResultRanks": {
                  "type": "array",
                  "items": {
                    "type": "integer",
                    "minimum": 1
                  }
                },
                "nextActionHints": {
                  "type": "array",
                  "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                      "action": {
                        "type": "string"
                      },
                      "actor": {
                        "type": "string"
                      },
                      "canAutoRun": {
                        "type": "boolean"
                      },
                      "detail": {
                        "type": "string"
                      },
                      "headedRequired": {
                        "type": "boolean"
                      },
                      "resultRanks": {
                        "type": "array",
                        "items": {
                          "type": "integer",
                          "minimum": 1
                        }
                      }
                    },
                    "required": [
                      "action",
                      "actor",
                      "canAutoRun",
                      "detail",
                      "headedRequired",
                      "resultRanks"
                    ]
                  }
                },
                "recovery": {
                  "type": "object",
                  "additionalProperties": false,
                  "properties": {
                    "recovered": {
                      "type": "boolean"
                    },
                    "humanInterventionRequiredNow": {
                      "type": "boolean"
                    },
                    "finalEngine": {
                      "type": "string"
                    },
                    "attempts": {
                      "type": "array",
                      "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                          "engine": {
                            "type": "string"
                          },
                          "status": {
                            "type": "string"
                          }
                        },
                        "required": [
                          "engine",
                          "status"
                        ]
                      }
                    }
                  },
                  "required": [
                    "recovered",
                    "humanInterventionRequiredNow",
                    "finalEngine",
                    "attempts"
                  ]
                }
              },
              "required": [
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
                "recovery"
              ]
            },
            "search": {
              "type": "object",
              "additionalProperties": false,
              "properties": {
                "version": {
                  "type": "string"
                },
                "status": {
                  "type": "string"
                },
                "statusDetail": {
                  "type": "string"
                },
                "query": {
                  "type": "string"
                },
                "engine": {
                  "type": "string"
                },
                "searchUrl": {
                  "type": "string"
                },
                "finalUrl": {
                  "type": "string"
                },
                "generatedAt": {
                  "type": "string"
                },
                "resultCount": {
                  "type": "integer",
                  "minimum": 0
                },
                "results": {
                  "type": "array",
                  "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                      "rank": {
                        "type": "integer",
                        "minimum": 1
                      },
                      "url": {
                        "type": "string"
                      },
                      "title": {
                        "type": "string"
                      },
                      "snippet": {
                        "type": "string"
                      },
                      "domain": {
                        "type": "string"
                      },
                      "officialLikely": {
                        "type": "boolean"
                      },
                      "recommendedSurface": {
                        "type": "string"
                      },
                      "selectionScore": {
                        "type": "number"
                      }
                    },
                    "required": [
                      "rank",
                      "url",
                      "title",
                      "snippet",
                      "domain",
                      "officialLikely",
                      "recommendedSurface",
                      "selectionScore"
                    ]
                  }
                },
                "recommendedResultRanks": {
                  "type": "array",
                  "items": {
                    "type": "integer",
                    "minimum": 1
                  }
                },
                "nextActionHints": {
                  "type": "array",
                  "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                      "action": {
                        "type": "string"
                      },
                      "actor": {
                        "type": "string"
                      },
                      "canAutoRun": {
                        "type": "boolean"
                      },
                      "detail": {
                        "type": "string"
                      },
                      "headedRequired": {
                        "type": "boolean"
                      },
                      "resultRanks": {
                        "type": "array",
                        "items": {
                          "type": "integer",
                          "minimum": 1
                        }
                      }
                    },
                    "required": [
                      "action",
                      "actor",
                      "canAutoRun",
                      "detail",
                      "headedRequired",
                      "resultRanks"
                    ]
                  }
                },
                "recovery": {
                  "type": "object",
                  "additionalProperties": false,
                  "properties": {
                    "recovered": {
                      "type": "boolean"
                    },
                    "humanInterventionRequiredNow": {
                      "type": "boolean"
                    },
                    "finalEngine": {
                      "type": "string"
                    },
                    "attempts": {
                      "type": "array",
                      "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                          "engine": {
                            "type": "string"
                          },
                          "status": {
                            "type": "string"
                          }
                        },
                        "required": [
                          "engine",
                          "status"
                        ]
                      }
                    }
                  },
                  "required": [
                    "recovered",
                    "humanInterventionRequiredNow",
                    "finalEngine",
                    "attempts"
                  ]
                }
              },
              "required": [
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
                "recovery"
              ]
            },
            "resultCount": {
              "type": "integer",
              "minimum": 0
            },
            "searchUrl": {
              "type": "string"
            },
            "sessionFile": {
              "type": "string"
            },
            "sessionState": {
              "type": "object",
              "additionalProperties": false,
              "properties": {
                "version": {
                  "type": "string"
                },
                "sessionId": {
                  "type": "string"
                },
                "mode": {
                  "type": "string"
                },
                "status": {
                  "type": "string"
                },
                "policyProfile": {
                  "type": "string"
                },
                "currentUrl": {
                  "type": [
                    "string",
                    "null"
                  ]
                },
                "openedAt": {
                  "type": "string"
                },
                "updatedAt": {
                  "type": "string"
                },
                "visitedUrls": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                },
                "snapshotIds": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                },
                "workingSetRefs": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                }
              },
              "required": [
                "version",
                "sessionId",
                "mode",
                "status",
                "policyProfile",
                "openedAt",
                "updatedAt",
                "visitedUrls",
                "snapshotIds"
              ]
            }
          },
          "required": [
            "engine",
            "query",
            "result",
            "search",
            "resultCount",
            "searchUrl",
            "sessionFile",
            "sessionState"
          ]
        }
      },
      "required": [
        "sessionId",
        "tabId",
        "result"
      ]
    }
  },
  {
    "name": "tb_search_open_result",
    "title": "Open One Search Result",
    "description": "Open one structured search result into a new research tab when you must override the recommended order after tb_search.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "tabId": {
          "type": "string",
          "description": "The specific tab to use inside the session. If omitted, the active tab is used."
        },
        "rank": {
          "type": "number",
          "description": "The 1-based rank of the search result to open in a new tab."
        }
      },
      "required": [
        "sessionId",
        "rank"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "searchTabId": {
          "type": "string"
        },
        "openedTabId": {
          "type": "string"
        },
        "selectionStrategy": {
          "type": "string"
        },
        "selectedResult": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "rank": {
              "type": "integer",
              "minimum": 1
            },
            "url": {
              "type": "string"
            },
            "title": {
              "type": "string"
            },
            "snippet": {
              "type": "string"
            },
            "domain": {
              "type": "string"
            },
            "officialLikely": {
              "type": "boolean"
            },
            "recommendedSurface": {
              "type": "string"
            },
            "selectionScore": {
              "type": "number"
            }
          },
          "required": [
            "rank",
            "url",
            "title",
            "snippet",
            "domain",
            "officialLikely",
            "recommendedSurface",
            "selectionScore"
          ]
        },
        "diagnostics": {
          "type": "object",
          "additionalProperties": true
        },
        "result": {
          "type": "object",
          "additionalProperties": true
        }
      },
      "required": [
        "sessionId",
        "searchTabId",
        "openedTabId",
        "selectionStrategy",
        "selectedResult",
        "result"
      ]
    }
  },
  {
    "name": "tb_search_open_top",
    "title": "Open Top Search Results",
    "description": "Open the top recommended search results into new research tabs. Prefer this immediately after tb_search, then inspect tabs with tb_read_view before tb_extract.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "tabId": {
          "type": "string",
          "description": "The specific tab to use inside the session. If omitted, the active tab is used."
        },
        "limit": {
          "type": "number",
          "description": "The maximum number of top-ranked search results to open."
        }
      },
      "required": [
        "sessionId"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "searchTabId": {
          "type": "string"
        },
        "openedCount": {
          "type": "integer",
          "minimum": 0
        },
        "openedTabs": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
              "tabId": {
                "type": "string"
              },
              "selectedResult": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                  "rank": {
                    "type": "integer",
                    "minimum": 1
                  },
                  "url": {
                    "type": "string"
                  },
                  "title": {
                    "type": "string"
                  },
                  "snippet": {
                    "type": "string"
                  },
                  "domain": {
                    "type": "string"
                  },
                  "officialLikely": {
                    "type": "boolean"
                  },
                  "recommendedSurface": {
                    "type": "string"
                  },
                  "selectionScore": {
                    "type": "number"
                  }
                },
                "required": [
                  "rank",
                  "url",
                  "title",
                  "snippet",
                  "domain",
                  "officialLikely",
                  "recommendedSurface",
                  "selectionScore"
                ]
              },
              "diagnostics": {
                "type": "object",
                "additionalProperties": true
              },
              "result": {
                "type": "object",
                "additionalProperties": true
              }
            },
            "required": [
              "tabId",
              "selectedResult",
              "result"
            ]
          }
        }
      },
      "required": [
        "sessionId",
        "searchTabId",
        "openedCount",
        "openedTabs"
      ]
    }
  },
  {
    "name": "tb_extract",
    "title": "Extract Evidence",
    "description": "Extract evidence-supported and insufficient-evidence claims from the current target or daemon tab. Use this after tb_read_view confirms the tab scope on public docs or reference pages.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "target": {
          "type": "string",
          "description": "The URL or target whose evidence should be extracted."
        },
        "claims": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "A list of claims to verify against the target and support with citations when possible."
        },
        "browser": {
          "type": "boolean",
          "description": "When true, use the browser-backed acquisition path for this operation."
        },
        "budget": {
          "type": "number",
          "description": "The processing budget to spend on this operation."
        },
        "mainOnly": {
          "type": "boolean",
          "description": "When true, extract only from the page's main content."
        },
        "verifierCommand": {
          "type": "string",
          "description": "An optional verifier command to run for this operation."
        },
        "sourceRisk": {
          "type": "string",
          "description": "The source risk label to attach to the result or citation."
        },
        "sourceLabel": {
          "type": "string",
          "description": "The human-readable source label to attach to the result or citation."
        },
        "allowDomains": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "Restrict the operation to this list of allowed domains."
        },
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "tabId": {
          "type": "string",
          "description": "The specific tab to use inside the session. If omitted, the active tab is used."
        }
      },
      "required": [
        "claims"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "minProperties": 1,
      "properties": {
        "open": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "version": {
              "type": "string"
            },
            "action": {
              "type": "string"
            },
            "status": {
              "type": "string"
            },
            "payloadType": {
              "type": "string"
            },
            "output": {
              "type": [
                "object",
                "null"
              ],
              "additionalProperties": true
            },
            "diagnostics": {
              "type": [
                "object",
                "null"
              ],
              "additionalProperties": true
            },
            "policy": {
              "type": "object",
              "additionalProperties": false,
              "properties": {
                "decision": {
                  "type": "string"
                },
                "sourceRisk": {
                  "type": "string"
                },
                "riskClass": {
                  "type": "string"
                },
                "blockedRefs": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                },
                "signals": {
                  "type": "array",
                  "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                      "kind": {
                        "type": "string"
                      },
                      "origin": {
                        "type": "string"
                      },
                      "stableRef": {
                        "type": "string"
                      },
                      "detail": {
                        "type": "string"
                      }
                    },
                    "required": [
                      "kind",
                      "origin",
                      "detail"
                    ]
                  }
                },
                "allowlistedDomains": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                }
              },
              "required": [
                "decision",
                "sourceRisk",
                "riskClass",
                "blockedRefs",
                "signals"
              ]
            },
            "failureKind": {
              "type": "string"
            },
            "message": {
              "type": "string"
            }
          },
          "required": [
            "version",
            "action",
            "status",
            "payloadType",
            "message"
          ]
        },
        "extract": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "version": {
              "type": "string"
            },
            "action": {
              "type": "string"
            },
            "status": {
              "type": "string"
            },
            "payloadType": {
              "type": "string"
            },
            "output": {
              "type": [
                "object",
                "null"
              ],
              "additionalProperties": true
            },
            "diagnostics": {
              "type": [
                "object",
                "null"
              ],
              "additionalProperties": true
            },
            "policy": {
              "type": "object",
              "additionalProperties": false,
              "properties": {
                "decision": {
                  "type": "string"
                },
                "sourceRisk": {
                  "type": "string"
                },
                "riskClass": {
                  "type": "string"
                },
                "blockedRefs": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                },
                "signals": {
                  "type": "array",
                  "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                      "kind": {
                        "type": "string"
                      },
                      "origin": {
                        "type": "string"
                      },
                      "stableRef": {
                        "type": "string"
                      },
                      "detail": {
                        "type": "string"
                      }
                    },
                    "required": [
                      "kind",
                      "origin",
                      "detail"
                    ]
                  }
                },
                "allowlistedDomains": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                }
              },
              "required": [
                "decision",
                "sourceRisk",
                "riskClass",
                "blockedRefs",
                "signals"
              ]
            },
            "failureKind": {
              "type": "string"
            },
            "message": {
              "type": "string"
            }
          },
          "required": [
            "version",
            "action",
            "status",
            "payloadType",
            "message"
          ]
        },
        "sessionState": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "version": {
              "type": "string"
            },
            "sessionId": {
              "type": "string"
            },
            "mode": {
              "type": "string"
            },
            "status": {
              "type": "string"
            },
            "policyProfile": {
              "type": "string"
            },
            "currentUrl": {
              "type": [
                "string",
                "null"
              ]
            },
            "openedAt": {
              "type": "string"
            },
            "updatedAt": {
              "type": "string"
            },
            "visitedUrls": {
              "type": "array",
              "items": {
                "type": "string"
              }
            },
            "snapshotIds": {
              "type": "array",
              "items": {
                "type": "string"
              }
            },
            "workingSetRefs": {
              "type": "array",
              "items": {
                "type": "string"
              }
            }
          },
          "required": [
            "version",
            "sessionId",
            "mode",
            "status",
            "policyProfile",
            "openedAt",
            "updatedAt",
            "visitedUrls",
            "snapshotIds"
          ]
        },
        "sessionId": {
          "type": "string"
        },
        "tabId": {
          "type": "string"
        },
        "diagnostics": {
          "type": "object",
          "additionalProperties": true
        },
        "result": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "open": {
              "type": "object",
              "additionalProperties": false,
              "properties": {
                "version": {
                  "type": "string"
                },
                "action": {
                  "type": "string"
                },
                "status": {
                  "type": "string"
                },
                "payloadType": {
                  "type": "string"
                },
                "output": {
                  "type": [
                    "object",
                    "null"
                  ],
                  "additionalProperties": true
                },
                "diagnostics": {
                  "type": [
                    "object",
                    "null"
                  ],
                  "additionalProperties": true
                },
                "policy": {
                  "type": "object",
                  "additionalProperties": false,
                  "properties": {
                    "decision": {
                      "type": "string"
                    },
                    "sourceRisk": {
                      "type": "string"
                    },
                    "riskClass": {
                      "type": "string"
                    },
                    "blockedRefs": {
                      "type": "array",
                      "items": {
                        "type": "string"
                      }
                    },
                    "signals": {
                      "type": "array",
                      "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                          "kind": {
                            "type": "string"
                          },
                          "origin": {
                            "type": "string"
                          },
                          "stableRef": {
                            "type": "string"
                          },
                          "detail": {
                            "type": "string"
                          }
                        },
                        "required": [
                          "kind",
                          "origin",
                          "detail"
                        ]
                      }
                    },
                    "allowlistedDomains": {
                      "type": "array",
                      "items": {
                        "type": "string"
                      }
                    }
                  },
                  "required": [
                    "decision",
                    "sourceRisk",
                    "riskClass",
                    "blockedRefs",
                    "signals"
                  ]
                },
                "failureKind": {
                  "type": "string"
                },
                "message": {
                  "type": "string"
                }
              },
              "required": [
                "version",
                "action",
                "status",
                "payloadType",
                "message"
              ]
            },
            "extract": {
              "type": "object",
              "additionalProperties": false,
              "properties": {
                "version": {
                  "type": "string"
                },
                "action": {
                  "type": "string"
                },
                "status": {
                  "type": "string"
                },
                "payloadType": {
                  "type": "string"
                },
                "output": {
                  "type": [
                    "object",
                    "null"
                  ],
                  "additionalProperties": true
                },
                "diagnostics": {
                  "type": [
                    "object",
                    "null"
                  ],
                  "additionalProperties": true
                },
                "policy": {
                  "type": "object",
                  "additionalProperties": false,
                  "properties": {
                    "decision": {
                      "type": "string"
                    },
                    "sourceRisk": {
                      "type": "string"
                    },
                    "riskClass": {
                      "type": "string"
                    },
                    "blockedRefs": {
                      "type": "array",
                      "items": {
                        "type": "string"
                      }
                    },
                    "signals": {
                      "type": "array",
                      "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                          "kind": {
                            "type": "string"
                          },
                          "origin": {
                            "type": "string"
                          },
                          "stableRef": {
                            "type": "string"
                          },
                          "detail": {
                            "type": "string"
                          }
                        },
                        "required": [
                          "kind",
                          "origin",
                          "detail"
                        ]
                      }
                    },
                    "allowlistedDomains": {
                      "type": "array",
                      "items": {
                        "type": "string"
                      }
                    }
                  },
                  "required": [
                    "decision",
                    "sourceRisk",
                    "riskClass",
                    "blockedRefs",
                    "signals"
                  ]
                },
                "failureKind": {
                  "type": "string"
                },
                "message": {
                  "type": "string"
                }
              },
              "required": [
                "version",
                "action",
                "status",
                "payloadType",
                "message"
              ]
            },
            "sessionState": {
              "type": "object",
              "additionalProperties": false,
              "properties": {
                "version": {
                  "type": "string"
                },
                "sessionId": {
                  "type": "string"
                },
                "mode": {
                  "type": "string"
                },
                "status": {
                  "type": "string"
                },
                "policyProfile": {
                  "type": "string"
                },
                "currentUrl": {
                  "type": [
                    "string",
                    "null"
                  ]
                },
                "openedAt": {
                  "type": "string"
                },
                "updatedAt": {
                  "type": "string"
                },
                "visitedUrls": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                },
                "snapshotIds": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                },
                "workingSetRefs": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                }
              },
              "required": [
                "version",
                "sessionId",
                "mode",
                "status",
                "policyProfile",
                "openedAt",
                "updatedAt",
                "visitedUrls",
                "snapshotIds"
              ]
            }
          },
          "required": [
            "open",
            "extract",
            "sessionState"
          ]
        }
      }
    }
  },
  {
    "name": "tb_read_view",
    "title": "Read View",
    "description": "Return a readable Markdown view of a target or daemon tab for scope checking on public docs and reference pages. Inspect mainContentQuality and mainContentReason before extracting claims.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "target": {
          "type": "string",
          "description": "The URL or target to convert into a readable view."
        },
        "browser": {
          "type": "boolean",
          "description": "When true, use the browser-backed acquisition path for this operation."
        },
        "budget": {
          "type": "number",
          "description": "The processing budget to spend on this operation."
        },
        "mainOnly": {
          "type": "boolean",
          "description": "When true, return only the page's main content in the read view."
        },
        "sourceRisk": {
          "type": "string",
          "description": "The source risk label to attach to the result or citation."
        },
        "sourceLabel": {
          "type": "string",
          "description": "The human-readable source label to attach to the result or citation."
        },
        "allowDomains": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "Restrict the operation to this list of allowed domains."
        },
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "tabId": {
          "type": "string",
          "description": "The specific tab to use inside the session. If omitted, the active tab is used."
        }
      }
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "minProperties": 1,
      "properties": {
        "sourceUrl": {
          "type": "string"
        },
        "sourceTitle": {
          "type": "string"
        },
        "markdownText": {
          "type": "string"
        },
        "approxTokens": {
          "type": "integer",
          "minimum": 0
        },
        "charCount": {
          "type": "integer",
          "minimum": 0
        },
        "lineCount": {
          "type": "integer",
          "minimum": 0
        },
        "mainOnly": {
          "type": "boolean"
        },
        "mainContentQuality": {
          "type": "string"
        },
        "mainContentReason": {
          "type": "string"
        },
        "mainContentHint": {
          "type": "string"
        },
        "refIndex": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
              "id": {
                "type": "string"
              },
              "kind": {
                "type": "string"
              },
              "ref": {
                "type": "string"
              }
            },
            "required": [
              "id",
              "kind",
              "ref"
            ]
          }
        },
        "sessionState": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "version": {
              "type": "string"
            },
            "sessionId": {
              "type": "string"
            },
            "mode": {
              "type": "string"
            },
            "status": {
              "type": "string"
            },
            "policyProfile": {
              "type": "string"
            },
            "currentUrl": {
              "type": [
                "string",
                "null"
              ]
            },
            "openedAt": {
              "type": "string"
            },
            "updatedAt": {
              "type": "string"
            },
            "visitedUrls": {
              "type": "array",
              "items": {
                "type": "string"
              }
            },
            "snapshotIds": {
              "type": "array",
              "items": {
                "type": "string"
              }
            },
            "workingSetRefs": {
              "type": "array",
              "items": {
                "type": "string"
              }
            }
          },
          "required": [
            "version",
            "sessionId",
            "mode",
            "status",
            "policyProfile",
            "openedAt",
            "updatedAt",
            "visitedUrls",
            "snapshotIds"
          ]
        },
        "sessionId": {
          "type": "string"
        },
        "tabId": {
          "type": "string"
        },
        "diagnostics": {
          "type": "object",
          "additionalProperties": true
        },
        "result": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "sourceUrl": {
              "type": "string"
            },
            "sourceTitle": {
              "type": "string"
            },
            "markdownText": {
              "type": "string"
            },
            "approxTokens": {
              "type": "integer",
              "minimum": 0
            },
            "charCount": {
              "type": "integer",
              "minimum": 0
            },
            "lineCount": {
              "type": "integer",
              "minimum": 0
            },
            "mainOnly": {
              "type": "boolean"
            },
            "mainContentQuality": {
              "type": "string"
            },
            "mainContentReason": {
              "type": "string"
            },
            "mainContentHint": {
              "type": "string"
            },
            "refIndex": {
              "type": "array",
              "items": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                  "id": {
                    "type": "string"
                  },
                  "kind": {
                    "type": "string"
                  },
                  "ref": {
                    "type": "string"
                  }
                },
                "required": [
                  "id",
                  "kind",
                  "ref"
                ]
              }
            },
            "sessionState": {
              "type": "object",
              "additionalProperties": false,
              "properties": {
                "version": {
                  "type": "string"
                },
                "sessionId": {
                  "type": "string"
                },
                "mode": {
                  "type": "string"
                },
                "status": {
                  "type": "string"
                },
                "policyProfile": {
                  "type": "string"
                },
                "currentUrl": {
                  "type": [
                    "string",
                    "null"
                  ]
                },
                "openedAt": {
                  "type": "string"
                },
                "updatedAt": {
                  "type": "string"
                },
                "visitedUrls": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                },
                "snapshotIds": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                },
                "workingSetRefs": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                }
              },
              "required": [
                "version",
                "sessionId",
                "mode",
                "status",
                "policyProfile",
                "openedAt",
                "updatedAt",
                "visitedUrls",
                "snapshotIds"
              ]
            }
          },
          "required": [
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
            "sessionState"
          ]
        }
      }
    }
  },
  {
    "name": "tb_policy",
    "title": "Policy Report",
    "description": "Return the policy evaluation for a target or daemon tab on the public docs/research web surface.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "target": {
          "type": "string",
          "description": "The URL or target whose policy classification should be reported."
        },
        "browser": {
          "type": "boolean",
          "description": "When true, use the browser-backed acquisition path for this operation."
        },
        "sourceRisk": {
          "type": "string",
          "description": "The source risk label to attach to the result or citation."
        },
        "sourceLabel": {
          "type": "string",
          "description": "The human-readable source label to attach to the result or citation."
        },
        "allowDomains": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "Restrict the operation to this list of allowed domains."
        },
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "tabId": {
          "type": "string",
          "description": "The specific tab to use inside the session. If omitted, the active tab is used."
        }
      }
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "minProperties": 1,
      "properties": {
        "policy": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "decision": {
              "type": "string"
            },
            "sourceRisk": {
              "type": "string"
            },
            "riskClass": {
              "type": "string"
            },
            "blockedRefs": {
              "type": "array",
              "items": {
                "type": "string"
              }
            },
            "signals": {
              "type": "array",
              "items": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                  "kind": {
                    "type": "string"
                  },
                  "origin": {
                    "type": "string"
                  },
                  "stableRef": {
                    "type": "string"
                  },
                  "detail": {
                    "type": "string"
                  }
                },
                "required": [
                  "kind",
                  "origin",
                  "detail"
                ]
              }
            },
            "allowlistedDomains": {
              "type": "array",
              "items": {
                "type": "string"
              }
            }
          },
          "required": [
            "decision",
            "sourceRisk",
            "riskClass",
            "blockedRefs",
            "signals"
          ]
        },
        "sessionState": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "version": {
              "type": "string"
            },
            "sessionId": {
              "type": "string"
            },
            "mode": {
              "type": "string"
            },
            "status": {
              "type": "string"
            },
            "policyProfile": {
              "type": "string"
            },
            "currentUrl": {
              "type": [
                "string",
                "null"
              ]
            },
            "openedAt": {
              "type": "string"
            },
            "updatedAt": {
              "type": "string"
            },
            "visitedUrls": {
              "type": "array",
              "items": {
                "type": "string"
              }
            },
            "snapshotIds": {
              "type": "array",
              "items": {
                "type": "string"
              }
            },
            "workingSetRefs": {
              "type": "array",
              "items": {
                "type": "string"
              }
            }
          },
          "required": [
            "version",
            "sessionId",
            "mode",
            "status",
            "policyProfile",
            "openedAt",
            "updatedAt",
            "visitedUrls",
            "snapshotIds"
          ]
        },
        "sessionId": {
          "type": "string"
        },
        "tabId": {
          "type": "string"
        },
        "diagnostics": {
          "type": "object",
          "additionalProperties": true
        },
        "result": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "policy": {
              "type": "object",
              "additionalProperties": false,
              "properties": {
                "decision": {
                  "type": "string"
                },
                "sourceRisk": {
                  "type": "string"
                },
                "riskClass": {
                  "type": "string"
                },
                "blockedRefs": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                },
                "signals": {
                  "type": "array",
                  "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                      "kind": {
                        "type": "string"
                      },
                      "origin": {
                        "type": "string"
                      },
                      "stableRef": {
                        "type": "string"
                      },
                      "detail": {
                        "type": "string"
                      }
                    },
                    "required": [
                      "kind",
                      "origin",
                      "detail"
                    ]
                  }
                },
                "allowlistedDomains": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                }
              },
              "required": [
                "decision",
                "sourceRisk",
                "riskClass",
                "blockedRefs",
                "signals"
              ]
            },
            "sessionState": {
              "type": "object",
              "additionalProperties": false,
              "properties": {
                "version": {
                  "type": "string"
                },
                "sessionId": {
                  "type": "string"
                },
                "mode": {
                  "type": "string"
                },
                "status": {
                  "type": "string"
                },
                "policyProfile": {
                  "type": "string"
                },
                "currentUrl": {
                  "type": [
                    "string",
                    "null"
                  ]
                },
                "openedAt": {
                  "type": "string"
                },
                "updatedAt": {
                  "type": "string"
                },
                "visitedUrls": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                },
                "snapshotIds": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                },
                "workingSetRefs": {
                  "type": "array",
                  "items": {
                    "type": "string"
                  }
                }
              },
              "required": [
                "version",
                "sessionId",
                "mode",
                "status",
                "policyProfile",
                "openedAt",
                "updatedAt",
                "visitedUrls",
                "snapshotIds"
              ]
            }
          },
          "required": [
            "policy",
            "sessionState"
          ]
        }
      }
    }
  },
  {
    "name": "tb_tab_open",
    "title": "Open New Tab",
    "description": "Create a new headless research tab, optionally opening a narrowed public web target immediately.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "target": {
          "type": "string",
          "description": "An optional URL or target to open immediately in the new tab."
        },
        "sourceRisk": {
          "type": "string",
          "description": "The source risk label to attach to the result or citation."
        },
        "sourceLabel": {
          "type": "string",
          "description": "The human-readable source label to attach to the result or citation."
        },
        "allowDomains": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "Restrict the operation to this list of allowed domains."
        }
      },
      "required": [
        "sessionId"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "activeTabId": {
          "type": "string"
        },
        "tab": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "tabId": {
              "type": "string"
            },
            "active": {
              "type": "boolean"
            },
            "sessionFile": {
              "type": "string"
            },
            "hasState": {
              "type": "boolean"
            },
            "currentUrl": {
              "type": [
                "string",
                "null"
              ]
            },
            "visitedUrlCount": {
              "type": "integer",
              "minimum": 0
            },
            "snapshotCount": {
              "type": "integer",
              "minimum": 0
            },
            "latestSearchQuery": {
              "type": [
                "string",
                "null"
              ]
            },
            "latestSearchResultCount": {
              "type": "integer",
              "minimum": 0
            }
          },
          "required": [
            "tabId",
            "active",
            "sessionFile",
            "hasState",
            "visitedUrlCount",
            "snapshotCount",
            "latestSearchResultCount"
          ]
        }
      },
      "required": [
        "sessionId",
        "activeTabId",
        "tab"
      ]
    }
  },
  {
    "name": "tb_tab_list",
    "title": "List Session Tabs",
    "description": "List all daemon tabs registered for a session.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        }
      },
      "required": [
        "sessionId"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "activeTabId": {
          "type": [
            "string",
            "null"
          ]
        },
        "tabs": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
              "tabId": {
                "type": "string"
              },
              "active": {
                "type": "boolean"
              },
              "sessionFile": {
                "type": "string"
              },
              "hasState": {
                "type": "boolean"
              },
              "currentUrl": {
                "type": [
                  "string",
                  "null"
                ]
              },
              "visitedUrlCount": {
                "type": "integer",
                "minimum": 0
              },
              "snapshotCount": {
                "type": "integer",
                "minimum": 0
              },
              "latestSearchQuery": {
                "type": [
                  "string",
                  "null"
                ]
              },
              "latestSearchResultCount": {
                "type": "integer",
                "minimum": 0
              }
            },
            "required": [
              "tabId",
              "active",
              "sessionFile",
              "hasState",
              "visitedUrlCount",
              "snapshotCount",
              "latestSearchResultCount"
            ]
          }
        }
      },
      "required": [
        "sessionId",
        "activeTabId",
        "tabs"
      ]
    }
  },
  {
    "name": "tb_tab_select",
    "title": "Select Active Tab",
    "description": "Set the active daemon tab for a session.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "tabId": {
          "type": "string",
          "description": "The specific tab to use inside the session. If omitted, the active tab is used."
        }
      },
      "required": [
        "sessionId",
        "tabId"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "activeTabId": {
          "type": "string"
        },
        "tab": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "tabId": {
              "type": "string"
            },
            "active": {
              "type": "boolean"
            },
            "sessionFile": {
              "type": "string"
            },
            "hasState": {
              "type": "boolean"
            },
            "currentUrl": {
              "type": [
                "string",
                "null"
              ]
            },
            "visitedUrlCount": {
              "type": "integer",
              "minimum": 0
            },
            "snapshotCount": {
              "type": "integer",
              "minimum": 0
            },
            "latestSearchQuery": {
              "type": [
                "string",
                "null"
              ]
            },
            "latestSearchResultCount": {
              "type": "integer",
              "minimum": 0
            }
          },
          "required": [
            "tabId",
            "active",
            "sessionFile",
            "hasState",
            "visitedUrlCount",
            "snapshotCount",
            "latestSearchResultCount"
          ]
        }
      },
      "required": [
        "sessionId",
        "activeTabId",
        "tab"
      ]
    }
  },
  {
    "name": "tb_tab_close",
    "title": "Close Session Tab",
    "description": "Close one daemon tab and update the active tab if needed.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "tabId": {
          "type": "string",
          "description": "The specific tab to use inside the session. If omitted, the active tab is used."
        }
      },
      "required": [
        "sessionId",
        "tabId"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "tabId": {
          "type": "string"
        },
        "removed": {
          "type": "boolean"
        },
        "removedState": {
          "type": "boolean"
        },
        "activeTabId": {
          "type": [
            "string",
            "null"
          ]
        },
        "remainingTabCount": {
          "type": "integer",
          "minimum": 0
        }
      },
      "required": [
        "sessionId",
        "tabId",
        "removed",
        "removedState",
        "activeTabId",
        "remainingTabCount"
      ]
    }
  },
  {
    "name": "tb_checkpoint",
    "title": "Session Checkpoint",
    "description": "Return the current supervised checkpoint guidance for a daemon session/tab.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "tabId": {
          "type": "string",
          "description": "The specific tab to use inside the session. If omitted, the active tab is used."
        }
      },
      "required": [
        "sessionId"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "tabId": {
          "type": "string"
        },
        "diagnostics": {
          "type": "object",
          "additionalProperties": true
        },
        "result": {
          "type": "object",
          "additionalProperties": true
        }
      },
      "required": [
        "sessionId",
        "tabId",
        "result"
      ]
    }
  },
  {
    "name": "tb_profile",
    "title": "Get Session Policy Profile",
    "description": "Return the active policy profile for a daemon session/tab.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "tabId": {
          "type": "string",
          "description": "The specific tab to use inside the session. If omitted, the active tab is used."
        }
      },
      "required": [
        "sessionId"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "tabId": {
          "type": "string"
        },
        "diagnostics": {
          "type": "object",
          "additionalProperties": true
        },
        "result": {
          "type": "object",
          "additionalProperties": true
        }
      },
      "required": [
        "sessionId",
        "tabId",
        "result"
      ]
    }
  },
  {
    "name": "tb_profile_set",
    "title": "Set Session Policy Profile",
    "description": "Set the active policy profile for a daemon session/tab.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "tabId": {
          "type": "string",
          "description": "The specific tab to use inside the session. If omitted, the active tab is used."
        },
        "profile": {
          "type": "string",
          "description": "The policy profile name to apply to the session."
        }
      },
      "required": [
        "sessionId",
        "profile"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "tabId": {
          "type": "string"
        },
        "diagnostics": {
          "type": "object",
          "additionalProperties": true
        },
        "result": {
          "type": "object",
          "additionalProperties": true
        }
      },
      "required": [
        "sessionId",
        "tabId",
        "result"
      ]
    }
  },
  {
    "name": "tb_click",
    "title": "Click Interactive Target",
    "description": "Click an interactive target inside an existing daemon session/tab.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "tabId": {
          "type": "string",
          "description": "The specific tab to use inside the session. If omitted, the active tab is used."
        },
        "targetRef": {
          "type": "string",
          "description": "The interactive element reference to click."
        },
        "ackRisks": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "The supervised risk codes acknowledged for this action."
        }
      },
      "required": [
        "sessionId",
        "targetRef"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "tabId": {
          "type": "string"
        },
        "diagnostics": {
          "type": "object",
          "additionalProperties": true
        },
        "result": {
          "type": "object",
          "additionalProperties": true
        }
      },
      "required": [
        "sessionId",
        "tabId",
        "result"
      ]
    }
  },
  {
    "name": "tb_type",
    "title": "Type Into Interactive Field",
    "description": "Type into an interactive field inside an existing daemon session/tab.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "tabId": {
          "type": "string",
          "description": "The specific tab to use inside the session. If omitted, the active tab is used."
        },
        "targetRef": {
          "type": "string",
          "description": "The interactive field reference to type into."
        },
        "value": {
          "type": "string",
          "description": "The text value to type into the target field."
        },
        "sensitive": {
          "type": "boolean",
          "description": "When true, treat the typed value as sensitive."
        },
        "ackRisks": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "The supervised risk codes acknowledged for this action."
        }
      },
      "required": [
        "sessionId",
        "targetRef",
        "value"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "tabId": {
          "type": "string"
        },
        "diagnostics": {
          "type": "object",
          "additionalProperties": true
        },
        "result": {
          "type": "object",
          "additionalProperties": true
        }
      },
      "required": [
        "sessionId",
        "tabId",
        "result"
      ]
    }
  },
  {
    "name": "tb_approve",
    "title": "Approve Supervised Risks",
    "description": "Persist supervised approval risks for the current daemon session so repeated ack flags are not required.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "ackRisks": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "The risk codes to persist as approved for the session."
        }
      },
      "required": [
        "sessionId",
        "ackRisks"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "approvedRisks": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "policyProfile": {
          "type": "string"
        }
      },
      "required": [
        "sessionId",
        "approvedRisks",
        "policyProfile"
      ]
    }
  },
  {
    "name": "tb_secret_store",
    "title": "Store Session Secret",
    "description": "Store a sensitive value only in daemon memory for a specific target ref.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "targetRef": {
          "type": "string",
          "description": "The target reference that this stored secret is intended for."
        },
        "value": {
          "type": "string",
          "description": "The sensitive value to store only in session memory."
        }
      },
      "required": [
        "sessionId",
        "targetRef",
        "value"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "stored": {
          "type": "boolean"
        },
        "targetRef": {
          "type": "string"
        },
        "secretCount": {
          "type": "integer",
          "minimum": 0
        }
      },
      "required": [
        "sessionId",
        "stored",
        "targetRef",
        "secretCount"
      ]
    }
  },
  {
    "name": "tb_secret_clear",
    "title": "Clear Session Secret",
    "description": "Clear one stored daemon secret or all secrets for a session.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "targetRef": {
          "type": "string",
          "description": "The target reference whose stored secret should be cleared. Omit to clear all stored secrets for the session."
        }
      },
      "required": [
        "sessionId"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "removed": {
          "type": "boolean"
        },
        "secretCount": {
          "type": "integer",
          "minimum": 0
        }
      },
      "required": [
        "sessionId",
        "removed",
        "secretCount"
      ]
    }
  },
  {
    "name": "tb_type_secret",
    "title": "Type Stored Secret",
    "description": "Type a previously stored daemon secret into a sensitive field without persisting it to disk.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "tabId": {
          "type": "string",
          "description": "The specific tab to use inside the session. If omitted, the active tab is used."
        },
        "targetRef": {
          "type": "string",
          "description": "The sensitive field reference that should receive the stored secret."
        },
        "ackRisks": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "The supervised risk codes acknowledged for this action."
        }
      },
      "required": [
        "sessionId",
        "targetRef"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "tabId": {
          "type": "string"
        },
        "diagnostics": {
          "type": "object",
          "additionalProperties": true
        },
        "result": {
          "type": "object",
          "additionalProperties": true
        }
      },
      "required": [
        "sessionId",
        "tabId",
        "result"
      ]
    }
  },
  {
    "name": "tb_submit",
    "title": "Submit Interactive Form",
    "description": "Submit a form or submit control inside an existing daemon session/tab.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "tabId": {
          "type": "string",
          "description": "The specific tab to use inside the session. If omitted, the active tab is used."
        },
        "targetRef": {
          "type": "string",
          "description": "The form or submit control reference to trigger."
        },
        "ackRisks": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "The supervised risk codes acknowledged for this action."
        }
      },
      "required": [
        "sessionId",
        "targetRef"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "tabId": {
          "type": "string"
        },
        "diagnostics": {
          "type": "object",
          "additionalProperties": true
        },
        "result": {
          "type": "object",
          "additionalProperties": true
        }
      },
      "required": [
        "sessionId",
        "tabId",
        "result"
      ]
    }
  },
  {
    "name": "tb_refresh",
    "title": "Refresh Live Session",
    "description": "Refresh the current headless research tab after query or filter changes.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "tabId": {
          "type": "string",
          "description": "The specific tab to use inside the session. If omitted, the active tab is used."
        }
      },
      "required": [
        "sessionId"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "tabId": {
          "type": "string"
        },
        "diagnostics": {
          "type": "object",
          "additionalProperties": true
        },
        "result": {
          "type": "object",
          "additionalProperties": true
        }
      },
      "required": [
        "sessionId",
        "tabId",
        "result"
      ]
    }
  },
  {
    "name": "tb_telemetry_summary",
    "title": "Pilot Telemetry Summary",
    "description": "Return the aggregated pilot telemetry summary for the current runtime.",
    "inputSchema": {
      "type": "object",
      "properties": {}
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "result": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "dbPath": {
              "type": "string"
            },
            "distinctSessionCount": {
              "type": "integer",
              "minimum": 0
            },
            "latestRecordedAtMs": {
              "type": "integer",
              "minimum": 0
            },
            "operationCounts": {
              "type": "object",
              "additionalProperties": {
                "type": "integer",
                "minimum": 0
              }
            },
            "statusCounts": {
              "type": "object",
              "additionalProperties": {
                "type": "integer",
                "minimum": 0
              }
            },
            "surfaceCounts": {
              "type": "object",
              "additionalProperties": {
                "type": "integer",
                "minimum": 0
              }
            },
            "totalEvents": {
              "type": "integer",
              "minimum": 0
            }
          },
          "required": [
            "dbPath",
            "distinctSessionCount",
            "latestRecordedAtMs",
            "operationCounts",
            "statusCounts",
            "surfaceCounts",
            "totalEvents"
          ]
        },
        "summary": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "dbPath": {
              "type": "string"
            },
            "distinctSessionCount": {
              "type": "integer",
              "minimum": 0
            },
            "latestRecordedAtMs": {
              "type": "integer",
              "minimum": 0
            },
            "operationCounts": {
              "type": "object",
              "additionalProperties": {
                "type": "integer",
                "minimum": 0
              }
            },
            "statusCounts": {
              "type": "object",
              "additionalProperties": {
                "type": "integer",
                "minimum": 0
              }
            },
            "surfaceCounts": {
              "type": "object",
              "additionalProperties": {
                "type": "integer",
                "minimum": 0
              }
            },
            "totalEvents": {
              "type": "integer",
              "minimum": 0
            }
          },
          "required": [
            "dbPath",
            "distinctSessionCount",
            "latestRecordedAtMs",
            "operationCounts",
            "statusCounts",
            "surfaceCounts",
            "totalEvents"
          ]
        }
      },
      "required": [
        "result",
        "summary"
      ]
    }
  },
  {
    "name": "tb_telemetry_recent",
    "title": "Recent Pilot Telemetry",
    "description": "Return recent pilot telemetry events for the current runtime.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "limit": {
          "type": "integer",
          "minimum": 1,
          "description": "The maximum number of recent telemetry events to return."
        }
      }
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "limit": {
          "type": "integer",
          "minimum": 1
        },
        "events": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": true,
            "properties": {
              "operation": {
                "type": "string"
              },
              "payload": {
                "type": "object",
                "additionalProperties": true
              },
              "recordedAtMs": {
                "type": "integer",
                "minimum": 0
              },
              "status": {
                "type": "string"
              },
              "surface": {
                "type": "string"
              },
              "sessionId": {
                "type": "string"
              },
              "policyProfile": {
                "type": "string"
              },
              "policyDecision": {
                "type": "string"
              },
              "riskClass": {
                "type": "string"
              },
              "currentUrl": {
                "type": "string"
              }
            },
            "required": [
              "operation",
              "recordedAtMs",
              "status",
              "surface"
            ]
          }
        },
        "result": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": true,
            "properties": {
              "operation": {
                "type": "string"
              },
              "payload": {
                "type": "object",
                "additionalProperties": true
              },
              "recordedAtMs": {
                "type": "integer",
                "minimum": 0
              },
              "status": {
                "type": "string"
              },
              "surface": {
                "type": "string"
              },
              "sessionId": {
                "type": "string"
              },
              "policyProfile": {
                "type": "string"
              },
              "policyDecision": {
                "type": "string"
              },
              "riskClass": {
                "type": "string"
              },
              "currentUrl": {
                "type": "string"
              }
            },
            "required": [
              "operation",
              "recordedAtMs",
              "status",
              "surface"
            ]
          }
        }
      },
      "required": [
        "limit",
        "events",
        "result"
      ]
    }
  },
  {
    "name": "tb_session_synthesize",
    "title": "Synthesize Session",
    "description": "Aggregate visited tabs inside a daemon session into a citation-ready synthesis report.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        },
        "noteLimit": {
          "type": "integer",
          "description": "The maximum number of notes to include in the synthesis report."
        }
      },
      "required": [
        "sessionId"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "activeTabId": {
          "type": [
            "string",
            "null"
          ]
        },
        "tabCount": {
          "type": "integer",
          "minimum": 0
        },
        "format": {
          "type": "string"
        },
        "markdown": {
          "type": [
            "string",
            "null"
          ]
        },
        "report": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "version": {
              "type": "string"
            },
            "sessionId": {
              "type": "string"
            },
            "generatedAt": {
              "type": "string"
            },
            "snapshotCount": {
              "type": "integer",
              "minimum": 0
            },
            "evidenceReportCount": {
              "type": "integer",
              "minimum": 0
            },
            "visitedUrls": {
              "type": "array",
              "items": {
                "type": "string"
              }
            },
            "workingSetRefs": {
              "type": "array",
              "items": {
                "type": "string"
              }
            },
            "synthesizedNotes": {
              "type": "array",
              "items": {
                "type": "string"
              }
            },
            "evidenceSupportedClaims": {
              "type": "array",
              "items": {
                "type": "object",
                "additionalProperties": true
              }
            },
            "contradictedClaims": {
              "type": "array",
              "items": {
                "type": "object",
                "additionalProperties": true
              }
            },
            "insufficientEvidenceClaims": {
              "type": "array",
              "items": {
                "type": "object",
                "additionalProperties": true
              }
            },
            "needsMoreBrowsingClaims": {
              "type": "array",
              "items": {
                "type": "object",
                "additionalProperties": true
              }
            }
          },
          "required": [
            "version",
            "sessionId",
            "generatedAt",
            "snapshotCount",
            "evidenceReportCount",
            "visitedUrls",
            "workingSetRefs",
            "synthesizedNotes",
            "evidenceSupportedClaims",
            "insufficientEvidenceClaims"
          ]
        },
        "tabReports": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
              "tabId": {
                "type": "string"
              },
              "report": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                  "version": {
                    "type": "string"
                  },
                  "sessionId": {
                    "type": "string"
                  },
                  "generatedAt": {
                    "type": "string"
                  },
                  "snapshotCount": {
                    "type": "integer",
                    "minimum": 0
                  },
                  "evidenceReportCount": {
                    "type": "integer",
                    "minimum": 0
                  },
                  "visitedUrls": {
                    "type": "array",
                    "items": {
                      "type": "string"
                    }
                  },
                  "workingSetRefs": {
                    "type": "array",
                    "items": {
                      "type": "string"
                    }
                  },
                  "synthesizedNotes": {
                    "type": "array",
                    "items": {
                      "type": "string"
                    }
                  },
                  "evidenceSupportedClaims": {
                    "type": "array",
                    "items": {
                      "type": "object",
                      "additionalProperties": true
                    }
                  },
                  "contradictedClaims": {
                    "type": "array",
                    "items": {
                      "type": "object",
                      "additionalProperties": true
                    }
                  },
                  "insufficientEvidenceClaims": {
                    "type": "array",
                    "items": {
                      "type": "object",
                      "additionalProperties": true
                    }
                  },
                  "needsMoreBrowsingClaims": {
                    "type": "array",
                    "items": {
                      "type": "object",
                      "additionalProperties": true
                    }
                  }
                },
                "required": [
                  "version",
                  "sessionId",
                  "generatedAt",
                  "snapshotCount",
                  "evidenceReportCount",
                  "visitedUrls",
                  "workingSetRefs",
                  "synthesizedNotes",
                  "evidenceSupportedClaims",
                  "insufficientEvidenceClaims"
                ]
              }
            },
            "required": [
              "tabId",
              "report"
            ]
          }
        }
      },
      "required": [
        "sessionId",
        "activeTabId",
        "tabCount",
        "format",
        "report",
        "tabReports"
      ]
    }
  },
  {
    "name": "tb_session_close",
    "title": "Close Session",
    "description": "Close a daemon session and clean up all tab state.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "sessionId": {
          "type": "string",
          "description": "The browser session to use for this operation."
        }
      },
      "required": [
        "sessionId"
      ]
    },
    "outputSchema": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "removed": {
          "type": "boolean"
        },
        "removedTabs": {
          "type": "integer",
          "minimum": 0
        }
      },
      "required": [
        "sessionId",
        "removed",
        "removedTabs"
      ]
    }
  }
];
