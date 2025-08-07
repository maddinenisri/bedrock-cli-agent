#!/bin/bash

# Test script for Bedrock CLI Agent

set -e

echo "🚀 Testing Bedrock CLI Agent"
echo "=============================="
echo ""

# Build the project
echo "📦 Building project..."
cargo build --release
echo "✅ Build successful"
echo ""

# Test help command
echo "📖 Testing help..."
./target/release/bedrock-agent --help
echo ""

# Test configuration display
echo "⚙️ Testing config display..."
./target/release/bedrock-agent config
echo ""

# Test tool listing
echo "🔧 Testing tool listing..."
./target/release/bedrock-agent tools
echo ""

# Test simple task without tools
echo "🤖 Testing simple task (no tools)..."
./target/release/bedrock-agent task -p "Say hello and introduce yourself"
echo ""

# Test task with tools (if AWS credentials are configured)
if aws sts get-caller-identity > /dev/null 2>&1; then
    echo "🔨 Testing task with tools..."
    ./target/release/bedrock-agent task -p "What programming languages are used in this project?" -t
else
    echo "⚠️ AWS credentials not configured, skipping tool test"
fi

echo ""
echo "✅ All tests completed!"