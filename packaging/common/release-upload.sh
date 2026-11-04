#!/usr/bin/env bash
# Attach build artifacts to an existing GitHub release (needs GH_TOKEN).
#
# The release must already exist (release-candidate.yml or a published
# release creates it), so a mistyped tag fails instead of creating a new
# release at the default branch. The tag must match the app version, with an
# optional `v` prefix and `-rc.N` suffix. --clobber keeps re-runs and
# concurrent uploads from the other packaging jobs safe.
set -euo pipefail

TAG="${1:?usage: release-upload.sh <tag> <file>...}"
shift
if [ "$#" -eq 0 ]; then
  echo "release-upload.sh: no files to upload" >&2
  exit 1
fi

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERSION="$("${HERE}/version.sh")"
case "${TAG#v}" in
  "${VERSION}" | "${VERSION}"-rc.[1-9]*) ;;
  *)
    echo "release-upload.sh: tag ${TAG} does not match app version ${VERSION}" >&2
    exit 1
    ;;
esac

if ! gh release view "${TAG}" >/dev/null; then
  echo "release-upload.sh: release ${TAG} does not exist" >&2
  exit 1
fi
gh release upload "${TAG}" "$@" --clobber
