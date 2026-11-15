#!/usr/bin/env bash
# Generate release notes as markdown bullets over the first-parent history
# between the previous v* tag and the tagged commit: merged PRs as one bullet
# with the PR title, direct pushes as one bullet with the commit subject.
# Usage: scripts/generate-release-notes.sh <output.md>
# Needs GH_TOKEN in the env for `gh pr view`; GITHUB_REPOSITORY and GITHUB_SHA
# are provided by the Actions runner (default to the local repo/HEAD).
set -euo pipefail

cd "$(dirname "$0")/.."

OUT=${1:?usage: generate-release-notes.sh <output.md>}
REPO=${GITHUB_REPOSITORY:-$(git remote get-url origin 2>/dev/null | sed -n 's|.*github\.com[:/]\([^/]*/[^/]*\)\.git$|\1|p' || true)}
if [ -z "$REPO" ]; then
    echo "error: GITHUB_REPOSITORY not set and no github origin remote" >&2
    exit 1
fi
SHA=${GITHUB_SHA:-HEAD}
# Peel annotated tags down to the commit they point at.
SHA=$(git rev-parse "$SHA^{commit}")

prev_tag=$(git describe --tags --abbrev=0 --match 'v*' "$SHA^" 2>/dev/null || true)
if [ -n "$prev_tag" ]; then
    log_range=("$prev_tag..$SHA")
    compare="$prev_tag...$SHA"
else
    # First release: no previous v* tag, list only the tagged commit.
    log_range=(--max-count=1 "$SHA")
    compare=""
fi

# Escape markdown metacharacters so subjects and PR titles can't inject
# links, images or code spans into the public release body.
sanitize() {
    LC_ALL=C sed -e 's/\\/\\\\/g' -e 's/`/\\`/g' -e 's/\[/\\[/g' -e 's/\]/\\]/g'
}

{
    echo "## Changes"
    echo
    # tformat (not format) so the last commit of the range ends with a newline
    # and is not dropped by `read`.
    git log --first-parent --pretty=tformat:'%H%x1f%P%x1f%s' "${log_range[@]}" |
        while IFS=$'\x1f' read -r _hash parents subject; do
            set -- $parents
            if [ "$#" -gt 1 ] && [[ $subject =~ ^Merge\ pull\ request\ \#([0-9]+) ]]; then
                n=${BASH_REMATCH[1]}
                title=$(gh pr view "$n" --json title --jq '.title' 2>/dev/null || true)
                printf -- '- %s (#%s)\n' "$(printf '%s' "${title:-$subject}" | sanitize)" "$n"
            else
                printf -- '- %s\n' "$(printf '%s' "$subject" | sanitize)"
            fi
        done
    echo
    if [ -n "$compare" ]; then
        echo "**Full Changelog**: https://github.com/$REPO/compare/$compare"
    fi
} > "$OUT"
