#!/bin/sh
#
# crdt-kit release script
# Automates: checks -> version bump -> commit -> tag -> push -> gh release -> crates.io publish
#
# Usage:
#   ./scripts/release.sh 0.3.0 "Short description of this release"
#
# Prerequisites:
#   - gh CLI authenticated (gh auth login)
#   - cargo login done (for crates.io publish)
#   - Clean working tree (no uncommitted changes)
#

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'
BOLD='\033[1m'

# --- Validate arguments ---
VERSION="$1"
DESCRIPTION="$2"

if [ -z "$VERSION" ] || [ -z "$DESCRIPTION" ]; then
    echo "${RED}Usage: ./scripts/release.sh <version> <description>${NC}"
    echo "  Example: ./scripts/release.sh 0.3.0 \"Add persistence adapters\""
    exit 1
fi

TAG="v${VERSION}"

echo ""
echo "========================================"
echo "  crdt-kit release ${TAG}"
echo "========================================"
echo ""

# --- Step 1: Check clean working tree ---
printf "${CYAN}[1/8]${NC} Checking clean working tree ... "
if [ -n "$(git status --porcelain)" ]; then
    printf "${RED}FAILED${NC}\n"
    echo "  Working tree is dirty. Commit or stash changes first."
    exit 1
fi
printf "${GREEN}OK${NC}\n"

# --- Step 2: Run all CI checks ---
printf "${CYAN}[2/8]${NC} Running CI checks ...\n"
echo ""

cargo fmt --all -- --check
echo "  fmt .............. OK"

cargo clippy --all-targets --all-features -- -D warnings 2>&1
echo "  clippy ........... OK"

cargo check --no-default-features --quiet
echo "  no_std ........... OK"

cargo test --all-features --quiet
echo "  tests ............ OK"

cargo test --doc --quiet
echo "  doctests ......... OK"

cargo doc --no-deps --all-features --quiet 2>&1
echo "  docs ............. OK"

echo ""
printf "${GREEN}  All checks passed!${NC}\n"
echo ""

# --- Step 3: Bump version in Cargo.toml ---
printf "${CYAN}[3/8]${NC} Bumping version to ${VERSION} ... "
sed -i "s/^version = \".*\"/version = \"${VERSION}\"/" Cargo.toml
printf "${GREEN}OK${NC}\n"

# --- Step 4: Update Cargo.lock ---
printf "${CYAN}[4/8]${NC} Updating Cargo.lock ... "
cargo check --quiet 2>/dev/null
printf "${GREEN}OK${NC}\n"

# --- Step 5: Commit ---
printf "${CYAN}[5/8]${NC} Committing ... "
git add Cargo.toml Cargo.lock
git commit -m "release: ${TAG} - ${DESCRIPTION}" --quiet
printf "${GREEN}OK${NC}\n"

# --- Step 6: Tag ---
printf "${CYAN}[6/8]${NC} Creating tag ${TAG} ... "
git tag -a "${TAG}" -m "${TAG} - ${DESCRIPTION}"
printf "${GREEN}OK${NC}\n"

# --- Step 7: Push ---
printf "${CYAN}[7/8]${NC} Pushing to GitHub ... "
git push origin master --quiet
git push origin "${TAG}" --quiet
printf "${GREEN}OK${NC}\n"

# --- Step 8: Create GitHub Release ---
printf "${CYAN}[8/8]${NC} Creating GitHub release ... "
gh release create "${TAG}" \
    --title "${TAG} - ${DESCRIPTION}" \
    --generate-notes \
    --latest
printf "${GREEN}OK${NC}\n"

echo ""
echo "========================================"
printf "  ${GREEN}Release ${TAG} complete!${NC}\n"
echo "========================================"
echo ""
echo "  GitHub:    https://github.com/abdielLopezpy/crdt-kit/releases/tag/${TAG}"
echo "  crates.io: https://crates.io/crates/crdt-kit/${VERSION}"
echo ""
printf "  ${YELLOW}To publish to crates.io, run:${NC}\n"
echo "    cargo publish"
echo ""
