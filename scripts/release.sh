#!/usr/bin/env bash
# zelper release script — test / build / package / publish (idempotent)
# Requires: run on x86_64 Linux (musl target build + staged binary execution check).
set -euo pipefail

die() { echo "release: ERROR: $*" >&2; exit 1; }
log() { echo "==> $*"; }

# ---- Phase 0: preflight ----
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || die "not inside a git work tree"
cd "$REPO_ROOT"
for cmd in git cargo jq rustup gh tar sha256sum; do
  command -v "$cmd" >/dev/null 2>&1 || die "required command not found: $cmd"
done
# Note: `gh auth status` succeeding does not guarantee write permission on the
# repo; an insufficient scope/role only surfaces at publish time (Phase 8).
gh auth status >/dev/null 2>&1 || die "gh is not authenticated (run: gh auth login)"
git remote get-url origin >/dev/null 2>&1 || die "git remote 'origin' is not configured"

# ---- Phase 1: determine version ----
VERSION="$(cargo metadata --no-deps --format-version 1 \
  | jq -r '.packages[] | select(.name == "zelper") | .version')" \
  || die "cargo metadata | jq failed while determining zelper version"
# '+' build metadata is rejected: unused in this repo's versioning
# (hyphens inside the prerelease part are allowed).
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]] \
  || die "could not determine zelper version from cargo metadata (got: '${VERSION}')"
TAG="v${VERSION}"
log "releasing ${TAG}"

# ---- Phase 2: clean work tree, tag consistency (tag creation/push happens just before publish) ----
[ -z "$(git status --porcelain)" ] || die "working tree is not clean (git status --porcelain is non-empty)"
HEAD_SHA="$(git rev-parse HEAD)"
git fetch origin --quiet || die "git fetch origin failed"
[ -n "$(git branch -r --contains "$HEAD_SHA")" ] \
  || die "HEAD ${HEAD_SHA} is not contained in any origin/* branch (push first)"
LOCAL_TAG_SHA="$(git rev-parse -q --verify "refs/tags/${TAG}^{commit}" || true)"
REMOTE_TAG_SHA="$(git ls-remote --tags origin "refs/tags/${TAG}^{}" | cut -f1)" \
  || die "git ls-remote of remote tag ${TAG} failed"
[ -n "$REMOTE_TAG_SHA" ] || REMOTE_TAG_SHA="$(git ls-remote --tags origin "refs/tags/${TAG}" | cut -f1)" \
  || die "git ls-remote of remote tag ${TAG} failed"
[ -z "$LOCAL_TAG_SHA" ] || [ "$LOCAL_TAG_SHA" = "$HEAD_SHA" ] \
  || die "local tag ${TAG} -> ${LOCAL_TAG_SHA} != HEAD ${HEAD_SHA} (tag move is not allowed; investigate)"
[ -z "$REMOTE_TAG_SHA" ] || [ "$REMOTE_TAG_SHA" = "$HEAD_SHA" ] \
  || die "remote tag ${TAG} -> ${REMOTE_TAG_SHA} != HEAD ${HEAD_SHA} (tag move is not allowed; investigate)"
if [ -z "$LOCAL_TAG_SHA" ] && [ -n "$REMOTE_TAG_SHA" ]; then
  git fetch origin "refs/tags/${TAG}:refs/tags/${TAG}" --quiet \
    || die "git fetch of remote tag ${TAG} failed"
  log "fetched existing remote tag ${TAG}"
fi

# ---- Phase 3: tests (plugin wasm target is required by build.rs) ----
rustup target add wasm32-wasip1 x86_64-unknown-linux-musl
log "running cargo test"
cargo test
# cargo test must not modify tracked files; a dirty tree here usually means
# Cargo.lock is out of sync with the bumped Cargo.toml version.
[ -z "$(git status --porcelain)" ] \
  || die "working tree became dirty after cargo test (Cargo.lock out of sync with Cargo.toml? commit the lock update and re-run; non-ignored CARGO_TARGET_DIR output can also trigger this)"

# ---- Phase 4: release build (musl static) ----
log "building x86_64-unknown-linux-musl (release)"
cargo build --release --target x86_64-unknown-linux-musl
BIN="${CARGO_TARGET_DIR:-target}/x86_64-unknown-linux-musl/release/zelper"
[ -f "$BIN" ] || die "built binary not found: ${BIN}"

# ---- Phase 5: stage & package (v0.1.0-compatible assets) ----
STAGE="tmp/release/${TAG}"
rm -rf "$STAGE"
mkdir -p "$STAGE"
cp "$BIN" LICENSE-MIT LICENSE-APACHE "$STAGE/"
ASSET_TAR="${STAGE}/zelper-x86_64-unknown-linux-musl.tar.gz"
ASSET_SUM="${STAGE}/sha256sums.txt"
tar -C "$STAGE" -czf "$ASSET_TAR" zelper LICENSE-MIT LICENSE-APACHE
( cd "$STAGE" && sha256sum zelper-x86_64-unknown-linux-musl.tar.gz > sha256sums.txt )

# ---- Phase 6: binary version check (exact match) ----
ACTUAL="$(cd "$STAGE" && ./zelper --version)" \
  || die "staged binary failed to run or report its version"
[ "$ACTUAL" = "zelper ${VERSION}" ] \
  || die "built binary reports '${ACTUAL}', expected 'zelper ${VERSION}'"

# ---- Phase 7: release notes (commit list since previous tag; HEAD^-based because the tag is not created yet) ----
PREV_TAG="$(git describe --tags --match 'v*' --abbrev=0 "HEAD^" 2>/dev/null || true)"
if [ -n "$PREV_TAG" ]; then RANGE="${PREV_TAG}..HEAD"; else RANGE="HEAD"; fi
NOTES="${STAGE}/release-notes.md"
{ echo "Changes since ${PREV_TAG:-<initial commit>}:"
  echo
  git log --no-merges --format='- %h %s' "$RANGE"; } > "$NOTES"

# ---- ensure tag: create + push only after version/package/notes all passed ----
# Annotated form is enforced only for tags this script creates; the type of an
# existing tag (local or fetched) is not re-checked.
if ! git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  git tag -a "${TAG}" -m "zelper ${VERSION}"
  log "created annotated tag ${TAG} at HEAD"
fi
if [ -z "$REMOTE_TAG_SHA" ]; then
  git push origin "refs/tags/${TAG}" || die "git push of tag ${TAG} failed"
  log "pushed tag ${TAG} to origin"
fi
[ "$(git rev-parse "${TAG}^{commit}")" = "$HEAD_SHA" ] || die "tag sanity check failed for ${TAG}"

# ---- Phase 8: publish (idempotent) ----
if gh release view "$TAG" >/dev/null 2>&1; then
  log "release ${TAG} already exists; overwriting assets and updating notes"
  gh release upload "$TAG" "$ASSET_TAR" "$ASSET_SUM" --clobber
  gh release edit "$TAG" --title "$TAG" --notes-file "$NOTES"
  RELEASE_URL="$(gh release view "$TAG" --json url --jq .url)" \
    || die "gh release view (url) failed for ${TAG}"
else
  log "creating release ${TAG}"
  gh release create "$TAG" --verify-tag --title "$TAG" --notes-file "$NOTES" \
    "$ASSET_TAR" "$ASSET_SUM"
  RELEASE_URL="$(gh release view "$TAG" --json url --jq .url)" \
    || die "gh release view (url) failed for ${TAG}"
fi
log "done: ${RELEASE_URL}"
