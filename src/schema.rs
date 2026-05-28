use serde_json::{Value, json};

pub fn workflow_plan_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "required": ["name", "objective", "tasks"],
      "properties": {
        "version": {"type": "integer"},
        "name": {"type": "string"},
        "objective": {"type": "string"},
        "riskLevel": {"enum": ["low", "medium", "high"]},
        "maxConcurrency": {"type": "integer", "minimum": 1, "maximum": 50},
        "tasks": {
          "type": "array",
          "minItems": 1,
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["id", "title", "prompt"],
            "properties": {
              "id": {"type": "string"},
              "title": {"type": "string"},
              "kind": {"enum": ["explore", "implement", "verify", "fix", "synthesize"]},
              "agent": {"type": "string"},
              "dependsOn": {"type": "array", "items": {"type": "string"}},
              "scope": {"type": "array", "items": {"type": "string"}},
              "prompt": {"type": "string"},
              "expectedOutput": {"enum": ["markdown", "json", "patch", "diff", "notes"]},
              "writes": {"type": "boolean"},
              "verify": {"type": "boolean"}
            }
          }
        },
        "verification": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "strategy": {"enum": ["none", "independent"]},
            "verifiersPerTask": {"type": "integer", "minimum": 0, "maximum": 3},
            "maxRetries": {"type": "integer", "minimum": 0, "maximum": 5},
            "prompt": {"type": "string"}
          }
        },
        "finalReport": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "format": {"enum": ["markdown", "json"]},
            "sections": {"type": "array", "items": {"type": "string"}}
          }
        }
      }
    })
}

pub fn verifier_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "required": ["status", "summary", "confidence"],
      "properties": {
        "status": {"enum": ["pass", "revise", "fail"]},
        "summary": {"type": "string"},
        "confidence": {"type": "number", "minimum": 0, "maximum": 1},
        "acceptedFindings": {"type": "array", "items": {"type": "string"}},
        "rejectedFindings": {"type": "array", "items": {"type": "string"}},
        "requiredChanges": {"type": "array", "items": {"type": "string"}}
      }
    })
}
