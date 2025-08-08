#!/bin/bash

echo "=== Full MCP Integration Test with Figma Server ==="
echo

# Set the API key
# export FIGMA_API_KEY="your-figma-api-key"  # Set your actual Figma API key

# Run the agent with MCP configuration
echo "Testing MCP with Figma server..."
echo "Query: 'What Figma tools do you have?'"
echo

cargo run -- --config examples/mcp-stdio-test.yaml task -p "What Figma tools do you have? List them with descriptions." 2>&1 | head -100

echo
echo "=== Test Complete ==="