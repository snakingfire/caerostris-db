#!/usr/bin/env bash
# scripts/board/claim.sh — atomic cross-lane work claiming for the continuous swarm.
#
# THE coordination primitive: runs in the MAIN worktree (resolved via git-common-dir)
# so every lane and every isolated work-worktree shares ONE claim set. A global
# mkdir-lock serialises the claim pass, so concurrent lanes receive DISJOINT batches
# of work — no two lanes work the same item.
#
# Usage:
#   claim.sh claim  <lane> [max]   # atomically claim up to <max> claimable items for <lane>;
#                                   # prints one TSV line per NEWLY-CLAIMED item:
#                                   #   id<TAB>type<TAB>status<TAB>priority<TAB>rubric_refs<TAB>title
#   claim.sh release <id> [id...]  # release claims (rmdir) so bounced items can be re-claimed
#   claim.sh list                  # list currently-claimed ids
#
# Claimable = status in {ready, in_review, blocked} AND not already claimed.
# Claims are per-round: the orchestrator releases its batch at round end, so a
# bounced item (still ready/in_review) is re-claimable and a done item (status=done)
# simply stops being claimable. No stale-claim buildup.
set -euo pipefail

GIT_COMMON=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null) || {
  echo "ERROR: not in a git repo" >&2
  exit 1
}
MAIN_ROOT=$(dirname "$GIT_COMMON")
BOARD="$MAIN_ROOT/.project/board/tasks"
CLAIMS="$MAIN_ROOT/.project/board/.claims"
LOCK="$CLAIMS/.lock"
mkdir -p "$CLAIMS"

field() { grep -m1 "^$2:" "$1" 2>/dev/null | sed "s/^$2:[[:space:]]*//" | tr -d '\r'; }

cmd="${1:-claim}"

case "$cmd" in
  release)
    shift
    for id in "$@"; do rmdir "$CLAIMS/$id" 2>/dev/null || rm -rf "$CLAIMS/$id" 2>/dev/null || true; done
    exit 0
    ;;
  list)
    ls "$CLAIMS" 2>/dev/null | grep -vE '^\.lock$' || true
    exit 0
    ;;
  gc)
    # Free stale claims from dead lanes (held longer than ${2:-30} min). Healthy
    # rounds release within minutes, so anything older is an abandoned claim.
    find "$CLAIMS" -mindepth 1 -maxdepth 1 -type d -not -name '.lock' -mmin "+${2:-30}" -exec rm -rf {} + 2>/dev/null || true
    exit 0
    ;;
  claim) : ;;
  *) echo "usage: claim.sh claim <lane> [max] | release <id...> | list | gc [min]" >&2; exit 2 ;;
esac

LANE="${2:?usage: claim.sh claim <lane> [max]}"
MAX="${3:-12}"

# Acquire the claim lock (atomic mkdir), up to ~20s, then proceed regardless (lock is advisory).
for _ in $(seq 1 40); do mkdir "$LOCK" 2>/dev/null && break || sleep 0.5; done
trap 'rmdir "$LOCK" 2>/dev/null || true' EXIT

# Self-heal: free claims abandoned by a dead lane (held > 30 min) before claiming.
find "$CLAIMS" -mindepth 1 -maxdepth 1 -type d -not -name '.lock' -mmin +30 -exec rm -rf {} + 2>/dev/null || true

claimed=0
# Highest priority first so lanes grab the most valuable work.
for prio in P0 P1 P2 P3; do
  [ "$claimed" -ge "$MAX" ] && break
  for f in "$BOARD"/*.md; do
    [ -e "$f" ] || continue
    [ "$claimed" -ge "$MAX" ] && break
    p=$(field "$f" priority); [ "$p" = "$prio" ] || continue
    status=$(field "$f" status)
    case "$status" in ready | in_review | blocked) ;; *) continue ;; esac
    id=$(field "$f" id); [ -n "$id" ] || continue
    [ -d "$CLAIMS/$id" ] && continue        # already claimed by another lane
    mkdir "$CLAIMS/$id" 2>/dev/null || continue
    echo "$LANE" >"$CLAIMS/$id/lane"
    type=$(field "$f" type); refs=$(field "$f" rubric_refs); title=$(field "$f" title)
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$id" "$type" "$status" "$prio" "$refs" "$title"
    claimed=$((claimed + 1))
  done
done
