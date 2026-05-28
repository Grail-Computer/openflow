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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn workflow_schema_is_strict_output_compatible() {
        assert_strict_objects_require_all_properties(&workflow_plan_schema(), "$");
    }

    #[test]
    fn verifier_schema_is_strict_output_compatible() {
        assert_strict_objects_require_all_properties(&verifier_schema(), "$");
    }

    #[test]
    fn workflow_override_fields_are_nullable() {
        let schema = workflow_plan_schema();
        assert_nullable(&schema["properties"]["defaults"]["properties"]["agent"]);
        assert_nullable(&schema["properties"]["defaults"]["properties"]["agentBin"]);
        assert_nullable(&schema["properties"]["defaults"]["properties"]["model"]);
        assert_nullable(&schema["properties"]["defaults"]["properties"]["sandbox"]);
        assert_nullable(&schema["properties"]["tasks"]["items"]["properties"]["model"]);
        assert_nullable(&schema["properties"]["tasks"]["items"]["properties"]["maxRetries"]);
        assert_nullable(&schema["properties"]["tasks"]["items"]["properties"]["verifierModel"]);
    }

    fn assert_strict_objects_require_all_properties(schema: &Value, path: &str) {
        if schema.get("type").and_then(Value::as_str) == Some("object")
            && schema.get("additionalProperties").and_then(Value::as_bool) == Some(false)
        {
            let properties = schema
                .get("properties")
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{path} is strict but has no properties object"));
            let required = schema
                .get("required")
                .and_then(Value::as_array)
                .unwrap_or_else(|| panic!("{path} is strict but has no required array"))
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .unwrap_or_else(|| panic!("{path} has a non-string required entry"))
                        .to_string()
                })
                .collect::<BTreeSet<_>>();

            for key in properties.keys() {
                assert!(
                    required.contains(key),
                    "{path} property {key:?} must be listed in required"
                );
            }
        }

        match schema {
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    assert_strict_objects_require_all_properties(item, &format!("{path}[{index}]"));
                }
            }
            Value::Object(entries) => {
                for (key, value) in entries {
                    assert_strict_objects_require_all_properties(value, &format!("{path}.{key}"));
                }
            }
            _ => {}
        }
    }

    fn assert_nullable(schema: &Value) {
        let any_of = schema
            .get("anyOf")
            .and_then(Value::as_array)
            .expect("nullable field should use anyOf");
        assert!(
            any_of
                .iter()
                .any(|entry| entry.get("type").and_then(Value::as_str) == Some("null")),
            "nullable field should accept null: {schema}"
        );
    }
}
