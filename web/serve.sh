#!/bin/bash
# Local development server for the WASM demo

echo "Starting T-SQL Remaker WASM demo..."
echo "The demo will be available at http://localhost:8080"
echo ""
echo "Press Ctrl+C to stop the server"
echo ""

# Use Python's built-in HTTP server
python -m http.server 8080 -d web
