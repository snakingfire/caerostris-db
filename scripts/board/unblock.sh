#!/usr/bin/env bash
# scripts/board/unblock.sh — open the cascade: flip backlog items whose deps are
# ALL done → ready, so the swarm's claim.sh can pull them. Run by the pace-marshal
# each tick. Idempotent. Skips epics (never "worked"); leaves blocked items alone
# (those are conflict-blocked → reland path, not dep-blocked).
#
# Prints the ids it flipped (space-separated). Commits the change if any.
set -euo pipefail

GIT_COMMON=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null) || { echo "not a git repo" >&2; exit 1; }
MAIN=$(dirname "$GIT_COMMON")
BOARD="$MAIN/.project/board/tasks"

field() { grep -m1 "^$2:" "$1" 2>/dev/null | sed "s/^$2:[[:space:]]*//" | tr -d '\r'; }

# 1) collect the set of done ids
declare -A DONE=()
for f in "$BOARD"/*.md; do
  [ -e "$f" ] || continue
  [ "$(field "$f" status)" = "done" ] && DONE["$(field "$f" id)"]=1
done

# 2) flip backlog items whose every dep is done
flipped=()
for f in "$BOARD"/*.md; do
  [ -e "$f" ] || continue
  [ "$(field "$f" status)" = "backlog" ] || continue
  type=$(field "$f" type); [ "$type" = "epic" ] && continue
  id=$(field "$f" id)
  deps=$(field "$f" deps | grep -oE '[A-Z]+-[0-9]+' || true)
  ok=1
  for d in $deps; do [ -n "${DONE[$d]:-}" ] || { ok=0; break; }; done
  if [ "$ok" = "1" ]; then
    perl -i -pe 's/^status: backlog\s*$/status: ready/ if !$done; $done=1 if /^status:/' "$f"
    flipped+=("$id")
  fi
done

if [ "${#flipped[@]}" -gt 0 ]; then
  ( cd "$MAIN" && git add .project/board/tasks/ && \
    git commit -q -m "board: unblock ${flipped[*]} — deps satisfied, opening the cascade

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>" ) || true
fi
echo "${flipped[*]:-}"
