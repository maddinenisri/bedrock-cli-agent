use serde_json::Value;
use std::io::{self, Write};

/// Get a human-readable display name for a tool
pub fn get_tool_display_name(tool_name: &str) -> String {
    match tool_name {
        // File operations
        "fs_read" => "Read File".to_string(),
        "fs_write" => "Write File".to_string(),
        "fs_list" => "List Files".to_string(),
        
        // Search tools
        "grep" => "Grep Search".to_string(),
        "find" => "Find Files".to_string(),
        "rg" => "RipGrep Search".to_string(),
        
        // Execution tools
        "execute_bash" => "Bash".to_string(),
        "execute_cmd" => "Cmd".to_string(),
        
        // Default for unknown tools
        _ => {
            // Convert snake_case to Title Case
            tool_name
                .split('_')
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        }
    }
}

/// Get an appropriate emoji for a tool category
pub fn get_tool_emoji(tool_name: &str) -> &'static str {
    match tool_name {
        // File operations
        "fs_read" | "fs_write" | "fs_list" => "📄",
        
        // Search operations
        "grep" | "find" | "rg" => "🔍",
        
        // Execution tools
        "execute_bash" | "execute_cmd" => "💻",
        
        // Default
        _ => "🔧",
    }
}

/// Format tool execution for display
pub fn format_tool_execution(tool_name: &str, args: &Value) -> String {
    match tool_name {
        "execute_bash" | "execute_cmd" => {
            if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
                // Truncate long commands for display
                let display_cmd = if cmd.len() > 50 {
                    format!("{cmd} | tail -20")
                } else {
                    cmd.to_string()
                };
                format!("Bash({display_cmd})")
            } else {
                "Bash()".to_string()
            }
        }
        "fs_read" => {
            if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                format!("Read({path})")
            } else {
                "Read()".to_string()
            }
        }
        "fs_write" => {
            if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                format!("Write({path})")
            } else {
                "Write()".to_string()
            }
        }
        _ => format!("{}()", get_tool_display_name(tool_name))
    }
}

/// Format tool result for display
pub fn format_tool_result(tool_name: &str, result: &Value) -> String {
    match tool_name {
        "execute_bash" | "execute_cmd" => {
            if let Some(obj) = result.as_object() {
                if let Some(success) = obj.get("success").and_then(|v| v.as_bool()) {
                    if success {
                        if let Some(stdout) = obj.get("stdout").and_then(|v| v.as_str()) {
                            let lines = stdout.lines().count();
                            if lines > 0 {
                                return format!("⏺ Executed successfully ({lines} lines output)");
                            }
                        }
                        return "⏺ Executed successfully".to_string();
                    } else {
                        if let Some(error) = obj.get("error").and_then(|v| v.as_str()) {
                            return format!("❌ Failed: {error}");
                        }
                        return "❌ Command failed".to_string();
                    }
                }
            }
            "⏺ Completed".to_string()
        }
        "fs_read" => {
            if let Some(obj) = result.as_object() {
                if let Some(_path) = obj.get("path").and_then(|v| v.as_str()) {
                    if let Some(content) = obj.get("content").and_then(|v| v.as_str()) {
                        let lines = content.lines().count();
                        return format!("⏺ Read {lines} lines");
                    }
                }
            }
            "⏺ Read complete".to_string()
        }
        "fs_write" => {
            if let Some(obj) = result.as_object() {
                if let Some(_path) = obj.get("path").and_then(|v| v.as_str()) {
                    return "⏺ Updated file".to_string();
                }
            }
            "⏺ Write complete".to_string()
        }
        _ => "⏺ Completed".to_string()
    }
}

/// Display tool execution with proper formatting
pub fn display_tool_execution(tool_name: &str, args: &Value) {
    let emoji = get_tool_emoji(tool_name);
    let formatted = format_tool_execution(tool_name, args);
    println!("{emoji} {formatted}");
    io::stdout().flush().unwrap();
}

/// Display tool result with proper formatting  
pub fn display_tool_result(tool_name: &str, result: &Value) {
    let formatted = format_tool_result(tool_name, result);
    println!("    ⎿  {formatted}");
}