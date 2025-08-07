#\!/bin/bash
cd /Users/srini/workspace/mdstect_ws/rust-agent/bedrock-cli-agent
cargo run -- task -p "List files in current directory" 2>&1 | tail -20
