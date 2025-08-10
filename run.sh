#!/bin/bash

# Exit immediately if a command exits with a non-zero status.
set -e

# --- Configuration ---
# Path to the cargo executable.
CARGO_PATH="$HOME/.cargo/bin/cargo"
LOG_FILE="python_api.log"

# --- Python API ---
echo "Starting Python API in the background..."
(
  cd python_api
  # Check if venv exists
  if [ ! -d "venv" ]; then
    echo "Python virtual environment not found. Please run 'python3 -m venv venv' in the python_api directory."
    exit 1
  fi
  source venv/bin/activate
  # Check if requirements are installed
  if ! pip freeze | grep -q -f requirements.txt; then
    echo "Installing Python dependencies..."
    pip install -r requirements.txt
  fi
  # Start server and redirect logs
  uvicorn app:app --reload --port 8000 > ../$LOG_FILE 2>&1
) &
PYTHON_PID=$!
echo "Python API started with PID: $PYTHON_PID. Logs are in $LOG_FILE"

# --- Rust CLI ---
echo "Starting Rust CLI in the foreground..."
(
  cd rust_cli
  if [ ! -f "Cargo.toml" ]; then
    echo "Cargo.toml not found in rust_cli directory."
    exit 1
  fi
  $CARGO_PATH run
)

# --- Cleanup ---
echo "Rust CLI exited. Terminating Python API..."
kill $PYTHON_PID
