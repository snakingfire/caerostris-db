export const meta = {
  name: 'mainspring',
  description:
    'caerostris-db autonomous build orchestrator — a CONTINUOUS throughput-maximizing lane. Claims a DISJOINT batch of work via scripts/board/claim.sh (atomic, main-worktree, lock-serialized — so N lanes never double-work), then runs each item through implement→review→pre-mortem→LAND independently (no land-at-end barrier; land the instant a PR clears its gates, behind a global land-lock), drives steering ratification of in_review designs, rebases+relands conflict-blocked PRs, releases its batch, and re-pulls newly-unblocked work every round until the board drains or .project/STOP appears.',
  phases: [
    { title: 'Orient' },
    { title: 'Bootstrap', detail: 'steering ratifies intent+rubric; planner decomposes (first run only)' },
    { title: 'Ratify', detail: 'steering signs off in_review design items → done → unblocks the cascade' },
    { title: 'Build', detail: 'implement→adversarial-review→pre-mortem→LAND per item, no cross-item barrier' },
    { title: 'Design', detail: 'specs / spikes / formal models / datasets, committed directly' },
  ],
}

// ---------------------------------------------------------------------------
// caerostris-db mainspring — v3 (continuous / dynamic / multi-lane w/ real claims).
// The script can't touch the FS; agents do. Coordination across lanes is via
// scripts/board/claim.sh, which atomically claims a DISJOINT batch in the MAIN
// worktree under a lock — so 3+ lanes parallelize over distinct work instead of
// triplicating it. Each round: release last batch → claim a fresh disjoint batch →
// ratify in_review designs (unblocks the cascade) + reland conflict-blocked PRs +
// implement→review→premortem→LAND code (land-on-approve, global land-lock) +
// produce design specs → re-orient. Runs until drained/STOP/budget.
// ---------------------------------------------------------------------------

const LANE = (args && args.lane) || 1
const ROUND_CAP = (args && args.roundCap) || 8 // distinct items this lane claims per round
const MAX_ROUNDS = (args && args.maxRounds) || 150
const MIN_BUDGET = (args && args.minBudget) || 120_000

const CANON =
  'Read docs/commanders-intent.md, docs/requirements/master-rubric.md, docs/process/autonomous-operating-model.md, ' +
  'docs/process/task-board-protocol.md and your own agent definition before acting. Follow the open-source guardrails ' +
  '(docs/process/open-source-guardrails.md): public repo, never commit secrets/data, license-clean only. ' +
  'Keep the board honest, watch .project/pace/deadline.md, run ./format_code.sh green before landing, never block the board.'

// Your item is ALREADY CLAIMED for this lane by scripts/board/claim.sh — just do the work.
const LANDLOCK =
  'MAIN-WRITE PROTOCOL (mandatory — prevents the shared-worktree race): do ALL build/test in an ISOLATED worktree, and NEVER `git checkout` a feature/steering branch in the main worktree. ' +
  'To write to main: (1) acquire the lock — `for i in $(seq 1 180); do mkdir .project/.land.lock 2>/dev/null && break || sleep 2; done`; ' +
  '(2) ensure on main — `git symbolic-ref --short HEAD` must be "main" (else `git checkout main`); ' +
  '(3-LAND) merge a branch INTO main while staying on main: `git merge --no-ff <branch>`, resolve additive conflicts in place (union the "pub mod ...;" lines in src/lib.rs + Cargo workspace members; keep both board sides), then `cargo build` sanity — if broken, `git merge --abort`, fix in the worktree, never leave main broken; ' +
  '(3-COMMIT) for board/ADR/decision edits: just `git add` + `git commit` on main (you are already on main; do NOT create or checkout any branch); ' +
  '(4) release — `rmdir .project/.land.lock`. Hold the lock only for steps 2–4 (seconds), NEVER during build/test.'

const CODE_ROLES = ['implementer', 'test-author', 'perf-engineer']
const STEERING = {
  storage: 'steering-storage',
  query: 'steering-query-cypher',
  acid: 'steering-distributed-acid',
  formal: 'steering-formal-methods',
  perf: 'steering-perf-sla',
}

const ORIENT_SCHEMA = {
  type: 'object',
  required: ['stop', 'bootstrapped', 'claimed_ids', 'code_ready', 'design_ready', 'ratify_now', 'reland', 'summary'],
  properties: {
    stop: { type: 'boolean', description: 'true if .project/STOP exists' },
    bootstrapped: { type: 'boolean', description: 'true if .project/.bootstrapped exists' },
    claimed_ids: { type: 'array', items: { type: 'string' }, description: 'EVERY id claim.sh just claimed this round (so the lane can release them next round)' },
    code_ready: {
      type: 'array',
      description: 'claimed items to IMPLEMENT (status ready, code-producing).',
      items: {
        type: 'object',
        required: ['id', 'role', 'title'],
        properties: { id: { type: 'string' }, title: { type: 'string' }, role: { type: 'string', enum: CODE_ROLES }, priority: { type: 'string' } },
      },
    },
    design_ready: {
      type: 'array',
      description: 'claimed items to SPEC/RESEARCH/MODEL (status ready, non-code: researcher|formal-prover|dataset-scout|planner-decomposer|docs-memory-curator|steering-*).',
      items: { type: 'object', required: ['id', 'role', 'title'], properties: { id: { type: 'string' }, title: { type: 'string' }, role: { type: 'string' } } },
    },
    ratify_now: {
      type: 'array',
      description: 'claimed in_review DESIGN items (ADRs/spikes/specs) to steering-ratify → done → unblocks the cascade (HIGHEST leverage).',
      items: {
        type: 'object',
        required: ['id', 'steering_role', 'title'],
        properties: { id: { type: 'string' }, title: { type: 'string' }, steering_role: { type: 'string' } },
      },
    },
    reland: {
      type: 'array',
      description: 'claimed items blocked ONLY by a landing/rebase conflict (signed-off, failed to merge) → integrator rebase+reland.',
      items: { type: 'object', required: ['id'], properties: { id: { type: 'string' }, branch: { type: 'string' } } },
    },
    summary: { type: 'string' },
  },
}

const VERDICT_SCHEMA = {
  type: 'object',
  required: ['verdict', 'rationale'],
  properties: {
    verdict: { type: 'string', enum: ['approve', 'changes_requested', 'reject'] },
    blocking_findings: { type: 'array', items: { type: 'string' } },
    rationale: { type: 'string' },
  },
}

const RESULT_SCHEMA = {
  type: 'object',
  required: ['id', 'status', 'notes'],
  properties: {
    id: { type: 'string' },
    status: { type: 'string', enum: ['pr_opened', 'committed', 'blocked', 'failed', 'done', 'skipped'] },
    branch: { type: 'string' },
    notes: { type: 'string' },
  },
}

// orient = release last batch, atomically claim a fresh DISJOINT batch, classify it.
// Retries on null (a transient API blip on orient must NOT permanently kill a lane).
async function orient(prevClaims) {
  for (let attempt = 1; attempt <= 4; attempt++) {
    const s = await orientOnce(prevClaims)
    if (s) return s
    log(`Lane ${LANE}: orient attempt ${attempt}/4 returned null (transient?) — retrying.`)
    prevClaims = [] // first attempt may have already released; don't double-release
  }
  return null
}

function orientOnce(prevClaims) {
  const rel = prevClaims && prevClaims.length ? `scripts/board/claim.sh release ${prevClaims.join(' ')}` : '(no prior claims to release)'
  return agent(
    `${CANON}\nORIENT + CLAIM (lane ${LANE}). Do EXACTLY this, in order, and report the structured result:\n` +
      `1. Release last round's claims: run \`${rel}\`.\n` +
      `2. Ensure the mock is healthy: run \`scripts/env/up.sh\` (idempotent).\n` +
      `2b. OPEN THE CASCADE: run \`scripts/board/unblock.sh\` — it flips every backlog item whose deps are now ` +
      `done to ready (and commits), so newly-unblocked work is claimable THIS round. This is how ratifying a spike ` +
      `immediately feeds its dependent implementation tasks into the pool.\n` +
      `3. Atomically claim this lane's DISJOINT batch: run \`scripts/board/claim.sh claim ${LANE} ${ROUND_CAP}\`. It prints one TSV ` +
      `row per NEWLY-CLAIMED item: id<TAB>type<TAB>status<TAB>priority<TAB>rubric_refs<TAB>title. These ids are now OWNED by lane ${LANE} — ` +
      `no other lane will touch them. Put every claimed id in claimed_ids.\n` +
      `4. Check .project/STOP (exists? → stop=true) and .project/.bootstrapped (exists? → bootstrapped=true).\n` +
      `5. Classify EACH claimed row into exactly one bucket with the right agent role (read the task file frontmatter/body if unsure):\n` +
      `   • ratify_now — in_review DESIGN item ONLY (type spike, or an ADR/spec with NO code branch) needing sign-off. steering_role by ` +
      `domain from rubric_refs: {3→steering-perf-sla, 11→steering-formal-methods, 1|7→steering-distributed-acid, 2→steering-storage, 4|6→steering-query-cypher}.\n` +
      `   • reland — a built CODE PR that needs landing: status=blocked (failed to merge / rebase conflict) OR status=in_review with an ` +
      `existing work/<id>-<slug> branch or .worktrees/<id> (i.e. a task/bug that was implemented but never landed). Check with ` +
      `\`git branch --list 'work/*'\` and match by id. The integrator rebases onto main + lands it. Include its branch. THIS IS HOW STUCK PRs LAND.\n` +
      `   • code_ready — status=ready, produces code → role implementer (default) | test-author (tests/TCK) | perf-engineer (benches).\n` +
      `   • design_ready — status=ready, produces a spec/model/research → role researcher | formal-prover | steering-* | dataset-scout | planner-decomposer.\n` +
      `Do NOT modify the board beyond the claim/release scripts above. Return the structured state.`,
    { label: `orient:L${LANE}`, phase: 'Orient', model: 'sonnet', schema: ORIENT_SCHEMA }
  )
}

// --- Orient ------------------------------------------------------------------
phase('Orient')
let state = await orient([])
if (!state) {
  log(`Lane ${LANE}: orient failed — aborting; pace-marshal will relaunch.`)
  return { ok: false, lane: LANE }
}
if (state.stop) {
  if (state.claimed_ids && state.claimed_ids.length) await orient(state.claimed_ids) // release before standing down
  log(`Lane ${LANE}: .project/STOP present — standing down.`)
  return { ok: true, lane: LANE, stopped: true }
}

// --- Bootstrap (first run only; lane 1 only) --------------------------------
if (!state.bootstrapped && LANE === 1) {
  phase('Bootstrap')
  log('First run — ratifying foundations and decomposing the board.')
  await parallel(
    Object.values(STEERING).map((s) => () =>
      agent(
        `${CANON}\nRATIFICATION PASS as ${s}: adversarially review docs/commanders-intent.md and docs/requirements/master-rubric.md ` +
          `for anything in YOUR domain that is infeasible/contradictory/under-specified; file findings as decisions/spikes/bugs on the board.`,
        { label: `ratify:${s}`, phase: 'Bootstrap', model: 'opus', agentType: s }
      )
    )
  )
  await agent(
    `${CANON}\nAs planner-decomposer, decompose EVERY epic into ready stories/tasks with deps + rubric_refs, set readiness, then ` +
      `create the .project/.bootstrapped sentinel. Keep design-before-code ordering.`,
    { label: 'decompose', phase: 'Bootstrap', model: 'opus', agentType: 'planner-decomposer' }
  )
  state = await orient(state.claimed_ids || [])
}

// --- Per-item processors (items are PRE-CLAIMED by claim.sh) -----------------
const ratifyItem = (it) =>
  agent(
    `${CANON}\nAs ${it.steering_role || STEERING.formal}, run the design-falsification + sign-off pass on in_review item ${it.id} ("${it.title}"). ` +
      `Try to break it; if it survives, RATIFY: record sign-off in .project/decisions/, set the ADR status accepted, set the board item status ` +
      `done. ${LANDLOCK} — commit these edits to main via the COMMIT path (do NOT create/checkout a branch). This unblocks dependent implementation. ` +
      `If it fails, send back with specific blocking findings (status blocked).`,
    { label: `ratify:${it.id}`, phase: 'Ratify', model: 'opus', agentType: it.steering_role || STEERING.formal, schema: RESULT_SCHEMA }
  )

const relandItem = (it) =>
  agent(
    `${CANON}\nAs the integrator, RELAND ${it.id}${it.branch ? ` (branch ${it.branch})` : ''}: in an ISOLATED worktree, rebase/merge latest main into the branch, resolve the ` +
      `additive conflict (typically the "pub mod ...;" ordering in src/lib.rs — keep BOTH modules, sorted), re-run ./format_code.sh + cargo test ` +
      `in that worktree. ${LANDLOCK} — land it via the LAND path (git merge --no-ff the branch into main), then commit the board item done with the landing sha (board:). ` +
      `Do NOT use scripts/pr/land.sh (it checks out in main). On a genuine test/review failure, file a BUG and leave blocked.`,
    { label: `reland:${it.id}`, phase: 'Build', model: 'sonnet', agentType: 'integrator', schema: RESULT_SCHEMA }
  )

const designItem = (it) =>
  agent(
    `${CANON}\nAs ${it.role}, do ${it.id} ("${it.title}"): produce the spec/ADR/model/research (write files freely — these are docs, not code branches), set the board item to in_review with a ` +
      `steering sign-off request in .project/decisions/ so it is ratified next round. ${LANDLOCK} — commit your files to main via the COMMIT path (do NOT create/checkout a branch). Resolve bug/spike findings folded into your scope inline.`,
    { label: `design:${it.id}`, phase: 'Design', model: 'opus', agentType: it.role, schema: RESULT_SCHEMA }
  )

// Code item: implement → review → pre-mortem → LAND, all per-item (no cross-item barrier).
function buildAndLand(items) {
  return pipeline(
    items,
    (it) =>
      agent(
        `${CANON}\nAs ${it.role}, implement board item ${it.id} ("${it.title}") TDD-first in an isolated worktree (branch work/${it.id}-<slug>; ` +
          `base it on the LATEST main). Run scripts/env/up.sh + scripts/env/bucket.sh for integration tests. Open a simulated PR (scripts/pr/open.sh), ` +
          `./format_code.sh green, set status in_review. Return status pr_opened + branch.`,
        { label: `impl:${it.id}`, phase: 'Build', model: 'opus', agentType: it.role, isolation: 'worktree', schema: RESULT_SCHEMA }
      ).then((impl) => ({ it, impl })),
    (prev) => {
      if (!prev.impl || prev.impl.status !== 'pr_opened') return { ...prev, skip: true }
      return agent(
        `${CANON}\nAdversarially review the PR for ${prev.it.id} on branch ${prev.impl.branch}. Try to break it (correctness, ACID, latency theorem, ` +
          `security, simplicity). 'looks fine' is never a sign-off. Tick the reviewer box in PR.md only on approve.`,
        { label: `review:${prev.it.id}`, phase: 'Build', model: 'opus', agentType: 'adversarial-reviewer', schema: VERDICT_SCHEMA }
      ).then((review) => ({ ...prev, review }))
    },
    (prev) => {
      if (prev.skip || !prev.review || prev.review.verdict !== 'approve') return prev
      return agent(
        `${CANON}\nPre-mortem the PR for ${prev.it.id}: assume it shipped and caused an incident — enumerate failure modes; gate on mitigations. ` +
          `Tick the pre-mortem box in PR.md only on approve.`,
        { label: `premortem:${prev.it.id}`, phase: 'Build', model: 'opus', agentType: 'premortem-analyst', schema: VERDICT_SCHEMA }
      ).then((premortem) => ({ ...prev, premortem }))
    },
    (prev) => {
      const approved = !prev.skip && prev.review && prev.review.verdict === 'approve' && prev.premortem && prev.premortem.verdict === 'approve'
      if (!approved) return { id: prev.it.id, status: 'in_review', notes: 'not approved this round' }
      return agent(
        `${CANON}\nAs the integrator, LAND ${prev.it.id} (branch ${prev.impl.branch}). Verify both sign-offs are ticked in PR.md. The branch was already ` +
          `built+tested in its isolated worktree by the implement step. ${LANDLOCK} — land it via the LAND path (git merge --no-ff ${prev.impl.branch} into main). ` +
          `Then commit the board item done with the landing sha (board:). Do NOT run scripts/pr/land.sh (it checks out in the main worktree). On a real ` +
          `build/test failure after merge, git merge --abort, file a BUG, leave the item in_review (do NOT leave main broken).`,
        { label: `land:${prev.it.id}`, phase: 'Build', model: 'sonnet', agentType: 'integrator', schema: RESULT_SCHEMA }
      )
    }
  )
}

// --- Continuous loop ---------------------------------------------------------
const landed = []
const ratified = []
const designed = []
let idle = 0
let round = 0

while (round < MAX_ROUNDS) {
  round++
  if (budget.total && budget.remaining() < MIN_BUDGET) {
    log(`Lane ${LANE}: budget floor reached (${Math.round(budget.remaining() / 1000)}k left) — releasing claims, stopping.`)
    if (state.claimed_ids && state.claimed_ids.length) await orient(state.claimed_ids)
    break
  }

  const ratifyNow = state.ratify_now || []
  const reland = state.reland || []
  const code = state.code_ready || []
  const design = state.design_ready || []
  const total = ratifyNow.length + reland.length + code.length + design.length

  if (total === 0) {
    idle++
    log(`Lane ${LANE} round ${round}: nothing claimable (board blocked or drained). idle=${idle}.`)
    if (idle >= 3) break
    state = await orient(state.claimed_ids || [])
    if (!state || state.stop) break
    continue
  }
  idle = 0
  log(`Lane ${LANE} round ${round}: ratify=${ratifyNow.length} reland=${reland.length} code=${code.length} design=${design.length}`)

  const [rRes, reRes, cRes, dRes] = await Promise.all([
    ratifyNow.length ? parallel(ratifyNow.map((it) => () => ratifyItem(it))) : Promise.resolve([]),
    reland.length ? parallel(reland.map((it) => () => relandItem(it))) : Promise.resolve([]),
    code.length ? buildAndLand(code) : Promise.resolve([]),
    design.length ? parallel(design.map((it) => () => designItem(it))) : Promise.resolve([]),
  ])

  ratified.push(...rRes.filter(Boolean).filter((r) => r.status === 'done').map((r) => r.id))
  landed.push(...[...reRes, ...cRes].filter(Boolean).filter((r) => r.status === 'done').map((r) => r.id))
  designed.push(...dRes.filter(Boolean).filter((r) => r.status && r.status !== 'skipped').map((r) => r.id))

  // Release this round's claims + re-pull the now-unblocked frontier.
  state = await orient(state.claimed_ids || [])
  if (!state || state.stop) break
}

log(`Lane ${LANE} done after ${round} rounds: landed ${landed.length}, ratified ${ratified.length}, design ${designed.length}.`)
return { ok: true, lane: LANE, rounds: round, landed, ratified, designed }
