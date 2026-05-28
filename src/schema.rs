use serde_json::{Value, json};

pub fn workflow_plan_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "required": ["version", "name", "objective", "riskLevel", "maxConcurrency", "defaults", "tasks", "verification", "finalReport"],
      "properties": {
        "version": {"type": "integer"},
        "name": {"type": "string"},
        "objective": {"type": "string"},
        "riskLevel": {"enum": ["low", "medium", "high"]},
        "maxConcurrency": {"type": "integer", "minimum": 1, "maximum": 50},
        "defaults": {
          "type": "object",
          "additionalProperties": false,
          "required": ["agent", "agentBin", "agentCommand", "model", "sandbox", "writeSandbox", "verifierAgent", "verifierAgentBin", "verifierAgentCommand", "verifierModel", "verifierSandbox"],
          "properties": {
            "agent": nullable_string(),
            "agentBin": nullable_string(),
            "agentCommand": nullable_string(),
            "model": nullable_string(),
            "sandbox": nullable_sandbox(),
            "writeSandbox": nullable_sandbox(),
            "verifierAgent": nullable_string(),
            "verifierAgentBin": nullable_string(),
            "verifierAgentCommand": nullable_string(),
            "verifierModel": nullable_string(),
            "verifierSandbox": nullable_sandbox()
          }
        },
        "tasks": {
          "type": "array",
          "minItems": 1,
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["id", "title", "kind", "role", "agent", "agentBin", "agentCommand", "model", "sandbox", "dependsOn", "scope", "prompt", "expectedOutput", "writes", "verify", "maxRetries", "verifiersPerTask", "verificationPrompt", "verifierAgent", "verifierAgentBin", "verifierAgentCommand", "verifierModel", "verifierSandbox"],
            "properties": {
              "id": {"type": "string"},
              "title": {"type": "string"},
              "kind": {"enum": ["explore", "implement", "verify", "fix", "synthesize"]},
              "role": {"type": "string"},
              "agent": nullable_string(),
              "agentBin": nullable_string(),
              "agentCommand": nullable_string(),
              "model": nullable_string(),
              "sandbox": nullable_sandbox(),
              "dependsOn": {"type": "array", "items": {"type": "string"}},
              "scope": {"type": "array", "items": {"type": "string"}},
              "prompt": {"type": "string"},
              "expectedOutput": {"enum": ["markdown", "json", "patch", "diff", "notes"]},
              "writes": {"type": "boolean"},
              "verify": {"type": "boolean"},
              "maxRetries": nullable_integer(0, 5),
              "verifiersPerTask": nullable_integer(0, 3),
              "verificationPrompt": nullable_string(),
              "verifierAgent": nullable_string(),
              "verifierAgentBin": nullable_string(),
              "verifierAgentCommand": nullable_string(),
              "verifierModel": nullable_string(),
              "verifierSandbox": nullable_sandbox()
            }
          }
        },
        "verification": {
          "type": "object",
          "additionalProperties": false,
          "required": ["strategy", "verifiersPerTask", "maxRetries", "prompt"],
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
          "required": ["format", "sections"],
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
      "required": ["status", "summary", "confidence", "acceptedFindings", "rejectedFindings", "requiredChanges"],
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

fn nullable_string() -> Value {
    json!({"anyOf": [{"type": "string"}, {"type": "null"}]})
}

fn nullable_sandbox() -> Value {
    json!({"anyOf": [{"type": "string", "enum": ["read-only", "workspace-write", "danger-full-access"]}, {"type": "null"}]})
}

fn nullable_integer(minimum: u32, maximum: u32) -> Value {
    json!({"anyOf": [{"type": "integer", "minimum": minimum, "maximum": maximum}, {"type": "null"}]})
}
