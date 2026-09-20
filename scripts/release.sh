#!/usr/bin/env bash
# 给当前 Cargo.toml 版本打 annotated tag。不要用 gh release create：
# 推送 v* 之后 package.yml 自测通过才会挂 GitHub Release。
# 用法：
#   ./scripts/release.sh           # 只在本地打 tag
#   ./scripts/release.sh --push    # 打 tag 并推到 origin
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=version.sh
source "$ROOT/scripts/version.sh"
VERSION="$(aida_version "$ROOT")"
TAG="v${VERSION}"
PUSH=0
for arg in "$@"; do
  case "$arg" in
    --push) PUSH=1 ;;
    -h|--help)
      sed -n '2,8p' "$0"
      exit 0
      ;;
    *)
      echo "unknown arg: $arg (want --push)" >&2
      exit 2
      ;;
  esac
done

if [[ -z "$VERSION" ]]; then
  echo "cannot read version from Cargo.toml" >&2
  exit 1
fi

HEAD="$(git rev-parse HEAD)"
if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  TARGET="$(git rev-list -n 1 "refs/tags/${TAG}")"
  if [[ "$TARGET" != "$HEAD" ]]; then
    echo "local tag ${TAG} points at ${TARGET:0:12}, not HEAD ${HEAD:0:12}." >&2
    echo "delete it first: git tag -d ${TAG}" >&2
    exit 1
  fi
  echo "local tag ${TAG} already at HEAD"
else
  git tag -a "$TAG" -m "AIDA Linux ${VERSION}"
  echo "created ${TAG} at $(git rev-parse --short HEAD)"
fi

if [[ "$PUSH" -eq 1 ]]; then
  REMOTE="$(git ls-remote --tags origin "refs/tags/${TAG}^{}" | awk '{print $1}')"
  if [[ -z "$REMOTE" ]]; then
    REMOTE="$(git ls-remote --tags origin "refs/tags/${TAG}" | awk '{print $1}')"
  fi
  if [[ -n "$REMOTE" ]]; then
    if [[ "$REMOTE" != "$HEAD" ]]; then
      echo "origin ${TAG} points at ${REMOTE:0:12}, not HEAD ${HEAD:0:12}; not pushing." >&2
      exit 1
    fi
    echo "origin already has ${TAG} at HEAD; not pushing"
    exit 0
  fi
  git push origin "$TAG"
  echo "pushed ${TAG}. package.yml will smoke then publish the GitHub Release."
else
  echo "not pushed. run: git push origin ${TAG}"
  echo "or: $0 --push"
fi
