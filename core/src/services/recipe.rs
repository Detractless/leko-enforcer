use std::io::{Read, Write};
use std::process::Command;

use serde::Deserialize;
use serde_json::Value;

use crate::error::{CoreError, CoreResult};

#[derive(Clone, Debug, Deserialize)]
pub struct Recipe {
    pub kind: String,
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub expected_code: i32,
    #[serde(default)]
    pub expected_token: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub field: String,
    #[serde(default)]
    pub threshold: f64,
    #[serde(default)]
    pub op: String,
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub expected: String,
}

pub fn parse(text: &str) -> CoreResult<Recipe> {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        if value.is_object() {
            return serde_json::from_value(value)
                .map_err(|error| CoreError::Parse(error.to_string()));
        }
    }
    toml::from_str(text).map_err(|error| CoreError::Parse(error.to_string()))
}

fn nested<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

fn command_check(recipe: &Recipe) -> CoreResult<bool> {
    let program = recipe.command.first()
        .ok_or_else(|| CoreError::InvalidInput("a command recipe needs a command".to_string()))?;
    let output = Command::new(program)
        .args(&recipe.command[1..])
        .output()
        .map_err(|error| CoreError::InvalidInput(error.to_string()))?;
    let code = output.status.code().unwrap_or(-1);
    let token_ok = recipe.expected_token.is_empty()
        || String::from_utf8_lossy(&output.stdout).contains(&recipe.expected_token);
    Ok(code == recipe.expected_code && token_ok)
}

fn local_only(url: &str) -> bool {
    url.starts_with("http://127.0.0.1:") || url.starts_with("http://localhost:")
}

fn http_check(recipe: &Recipe) -> CoreResult<bool> {
    if !local_only(&recipe.url) {
        return Err(CoreError::InvalidInput("only loopback URLs are allowed".to_string()));
    }
    let address = recipe.url.trim_start_matches("http://").to_string();
    let mut stream = std::net::TcpStream::connect(address)
        .map_err(|error| CoreError::InvalidInput(error.to_string()))?;
    let request = format!("GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes())
        .map_err(|error| CoreError::InvalidInput(error.to_string()))?;
    let mut response = String::new();
    stream.read_to_string(&mut response)
        .map_err(|error| CoreError::InvalidInput(error.to_string()))?;
    let body = response.split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .ok_or_else(|| CoreError::InvalidInput("no HTTP body".to_string()))?;
    let value: Value = serde_json::from_str(body)
        .map_err(|error| CoreError::InvalidInput(error.to_string()))?;
    let found = nested(&value, &recipe.field)
        .and_then(Value::as_f64)
        .ok_or_else(|| CoreError::InvalidInput("the field was missing".to_string()))?;
    Ok(match recipe.op.as_str() {
        "gte" => found >= recipe.threshold,
        "lte" => found <= recipe.threshold,
        "eq" => found == recipe.threshold,
        _ => return Err(CoreError::InvalidInput("unknown comparison".to_string())),
    })
}

fn prompt_check(recipe: &Recipe, config: &Value) -> CoreResult<bool> {
    if recipe.question.is_empty() {
        return Err(CoreError::InvalidInput("a prompt recipe needs a question".to_string()));
    }
    let answer = config.get("answer").and_then(Value::as_str)
        .ok_or_else(|| CoreError::InvalidInput("no answer was supplied".to_string()))?;
    Ok(answer == recipe.expected)
}

pub fn evaluate(recipe: &Recipe, config: &Value) -> CoreResult<bool> {
    match recipe.kind.as_str() {
        "command" => command_check(recipe),
        "http_json" => http_check(recipe),
        "prompt" => prompt_check(recipe, config),
        _ => Err(CoreError::InvalidInput("unknown recipe kind".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn all_three_builtin_recipes_run_without_code() {
        let json = r#"{"kind":"command","command":["/bin/sh","-c","echo ok"],"expected_code":0,"expected_token":"ok"}"#;
        let command = parse(json).unwrap();
        assert!(evaluate(&command, &serde_json::json!({})).unwrap());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 1024];
            let _ = stream.read(&mut buffer);
            let body = r#"{"score":7}"#;
            let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
            let _ = stream.write_all(response.as_bytes());
        });
        let toml_text = format!(
            "kind = \"http_json\"\nurl = \"http://127.0.0.1:{}\"\nfield = \"score\"\nthreshold = 5.0\nop = \"gte\"\n",
            address.port()
        );
        let http = parse(&toml_text).unwrap();
        assert!(evaluate(&http, &serde_json::json!({})).unwrap());
        server.join().unwrap();

        let prompt = parse(r#"{"kind":"prompt","question":"Type READY","expected":"READY"}"#).unwrap();
        assert!(evaluate(&prompt, &serde_json::json!({"answer": "READY"})).unwrap());
        assert!(!evaluate(&prompt, &serde_json::json!({"answer": "NO"})).unwrap());
    }
}
