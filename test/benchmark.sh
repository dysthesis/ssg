#!/usr/bin/env bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if hyperfine is installed
if ! command -v hyperfine &>/dev/null; then
  echo -e "${RED}Error: hyperfine is not installed${NC}"
  echo "Install with: cargo install hyperfine"
  echo "Or on most systems: brew install hyperfine / apt install hyperfine"
  exit 1
fi

# Build the release binary
echo -e "${YELLOW}Building release binary...${NC}"
cd "$(dirname "$0")/.."
cargo build --release

SSG_BIN="./target/release/ssg"

if [ ! -f "$SSG_BIN" ]; then
  echo -e "${RED}Error: Binary not found at $SSG_BIN${NC}"
  exit 1
fi

# Clean previous results
rm -rf result/

echo -e "${GREEN}Starting benchmarks...${NC}"
echo

# Run benchmarks with different scenarios
hyperfine \
  --warmup 5 \
  --min-runs 100 \
  --export-markdown benchmark-results.md \
  --export-json benchmark-results.json \
  --prepare 'rm -rf result/' \
  --command-name "Full build" \
  "cd test && ../$SSG_BIN"

echo
echo -e "${GREEN}Benchmark complete!${NC}"
echo "Results saved to:"
echo "  - benchmark-results.md (Markdown table)"
echo "  - benchmark-results.json (JSON data)"
echo
echo "Output files generated in test/result/"
