#!/usr/bin/env bash

set -euo pipefail

TARGET_DIR="dist/lena-book"

if [ ! -d "$TARGET_DIR" ]; then
    echo "Error: Directory '$TARGET_DIR' does not exist."
    echo "Hint: Generate the docs first by running your bootstrap script."
    exit 1
fi

echo "==> Preparing to serve '$TARGET_DIR'..."

# 1. Check for Python 3
if command -v python3 >/dev/null 2>&1; then
    echo "==> Using Python 3. Starting server on http://localhost:8080"
    python3 -m http.server 8080 --directory "$TARGET_DIR"

# 2. Check for Node (via npx)
elif command -v npx >/dev/null 2>&1; then
    echo "==> Using npx (Node.js). Starting server..."
    npx serve "$TARGET_DIR"

# 3. Fail gracefully with a hint
else
    echo "Error: Neither 'python3' nor 'npx' (Node.js) is available on your PATH."
    echo "Hint: To view the documentation locally without CORS issues, please install Python 3 or Node.js."
    exit 1
fi
