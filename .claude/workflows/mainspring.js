export const meta = {
  name: 'mainspring',
  description:
    'caerostris-db autonomous build orchestrator — a CONTINUOUS throughput-maximizing lane: pull all ready work, run each item through implement→review→pre-mortem→LAND independently (no land-at-end barrier), land+unblock the instant a PR clears its gates, drive design ratification + conflict-relands, and re-pull newly-unblocked work every round until the board drains or .project/STOP appears. Lane-aware: multiple instances run concurrently, coordinating via atomic mkdir claims + a global land-lock, to exceed the per-workflow agent cap.',
  phases: [
    { title: 'Orient' },
    { title: 'Bootstrap', detail: 'steering ratifies intent+rubric; planner decomposes (first run only)' },
    { title: 'Ratify', detail: 'steering signs off in_review design items → done → unblocks the cascade' },
    { title: 'Build', detail: 'implement→adversarial-review→pre-mortem→LAND per item, no cross-item barrier' },
    { title: 'Design', detail: 'specs / spikes / formal models / datasets, committed directly' },
  ],
}

// ---------------------------------------------------------------------------
// caerostris-db mainspring — v2 (continuous / dynamic / multi-lane).
// EPIC-010 / T-0004. The script cannot touch the FS or wallclock; it dispatches
// agents who do. Throughput rules (commander's directive, T+1:10):
//   * Nothing that CAN be worked waits on orchestrator policy — only on the
//     harness agent cap. Pull ALL ready work; excess queues and runs as slots free.
//   * Work LANDS the instant it clears its gates (per-item land stage + global
//     land-lock), unblocking dependents immediately — never blocked behind an
//     unrelated task at a synchronization point.
//   * Re-pull every round so newly-unblocked work enters with zero epoch delay.
//   * Lane-aware: run N instances concurrently (args.lane); they claim tasks
//     atomically (mkdir .project/board/.claims/<ID>) so no two lanes double-work.
// ---------------------------------------------------------------------------

const LANE = (args && args.lane) || 1
const ROUND_CAP = (args && args.roundCap) || 12 // items a lane pulls per round (harness caps real concurrency ~14)
const MAX_ROUNDS = (args && args.maxRounds) || 40 // hard backstop against runaway loops
const MIN_BUDGET = (args && args.minBudget) || 120_000 // stop pulling new work below this many remaining tokens

const CANON =
  'Read docs/commanders-intent.md, docs/requirements/master-rubric.md, docs/process/autonomous-operating-model.md, ' +
  'docs/process/task-board-protocol.md and your own agent definition before acting. Follow the open-source guardrails ' +
  '(docs/process/open-source-guardrails.md): public repo, never commit secrets/data, license-clean only. ' +
  'Keep the board honest, watch .project/pace/deadline.md, run ./format_code.sh green before landing, never block the board.'

// Every work agent runs this FIRST so concurrent lanes never double-work an item.
const CLAIM = (id) =>
  `CLAIM-FIRST (lane ${LANE}): your very first action is exactly ` +
  `\`mkdir -p .project/board/.claims && mkdir .project/board/.claims/${id} 2>/dev/null && echo CLAIM_OK || echo CLAIM_LOST\`. ` +
  `If it prints CLAIM_LOST, another lane owns ${id}: do NOTHING else and return status "skipped". Otherwise proceed.`

// The integrator wraps the merge in a global lock so lands serialize across all lanes.
const LANDLOCK =
  'LAND-LOCK: before merging, acquire the global land-lock — ' +
  '`for i in $(seq 1 90); do mkdir .project/.land.lock 2>/dev/null && break || sleep 2; done`. ' +
  'Hold it ONLY across the merge, then release with `rmdir .project/.land.lock`. ' +
  'Before merging, rebase/merge latest main into the branch and resolve trivial additive conflicts ' +
  '(e.g. `pub mod ...;` lines in src/lib.rs) so parallel branches land cleanly.'

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
  required: ['stop', 'bootstrapped', 'code_ready', 'design_ready', 'ratify_now', 'reland', 'summary'],
  properties: {
    stop: { type: 'boolean', description: 'true if .project/STOP exists' },
    bootstrapped: { type: 'boolean', description: 'true if .project/.bootstrapped exists' },
    behind_pace: { type: 'boolean' },
    code_ready: {
      type: 'array',
      description:
        'READY items whose role produces CODE (implementer|test-author|perf-engineer), deps all done, ' +
        'NOT already claimed (no .project/board/.claims/<id> dir), highest rubric-weight/priority first.',
      items: {
        type: 'object',
        required: ['id', 'role', 'title'],
        properties: {
          id: { type: 'string' },
          title: { type: 'string' },
          role: { type: 'string', enum: CODE_ROLES },
          priority: { type: 'string' },
        },
      },
    },
    design_ready: {
      type: 'array',
      description:
        'READY non-code items (researcher|formal-prover|dataset-scout|planner-decomposer|docs-memory-curator|steering-*), ' +
        'deps done, unclaimed. Produce a spec/ADR/model and leave in_review for ratification.',
      items: {
        type: 'object',
        required: ['id', 'role', 'title'],
        properties: { id: { type: 'string' }, title: { type: 'string' }, role: { type: 'string' } },
      },
    },
    ratify_now: {
      type: 'array',
      description:
        'in_review DESIGN items (ADRs/spikes/specs) awaiting steering sign-off. Ratifying these → done → unblocks the ' +
        'implementation cascade, so they are the HIGHEST-leverage work. Tag the steering role that owns each.',
      items: {
        type: 'object',
        required: ['id', 'steering_role', 'title'],
        properties: {
          id: { type: 'string' },
          title: { type: 'string' },
          steering_role: { type: 'string', description: 'steering-storage|steering-query-cypher|steering-distributed-acid|steering-formal-methods|steering-perf-sla' },
        },
      },
    },
    reland: {
      type: 'array',
      description:
        'items blocked ONLY by a landing/rebase conflict (signed-off but failed to merge). Need rebase-onto-main + reland.',
      items: {
        type: 'object',
        required: ['id'],
        properties: { id: { type: 'string' }, branch: { type: 'string' } },
      },
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

function orient(note) {
  return agent(
    `${CANON}\nORIENT (lane ${LANE}). Read .project/STOP, .project/.bootstrapped, the board (scripts/board/ls.sh + frontmatter), ` +
      `the latest .project/reports/ rubric report, and .project/pace/deadline.md. Return structured state per the schema: ` +
      `code_ready, design_ready, ratify_now (in_review design items needing steering sign-off — HIGHEST leverage, they unblock ` +
      `the cascade), and reland (signed-off items blocked only by a merge/rebase conflict). EXCLUDE any item that already has a ` +
      `claim dir at .project/board/.claims/<id> (another lane owns it). Order by rubric weight then priority. ${note || ''} ` +
      `Do not modify the repo or board beyond reading.`,
    { label: `orient:L${LANE}`, phase: 'Orient', model: 'sonnet', schema: ORIENT_SCHEMA }
  )
}

// --- Phase: Orient -----------------------------------------------------------
phase('Orient')
let state = await orient('First orient of this lane.')
if (!state) {
  log(`Lane ${LANE}: orient failed — aborting; pace-marshal will relaunch.`)
  return { ok: false, lane: LANE }
}
if (state.stop) {
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
  state = await orient('Re-orient after bootstrap.')
}

// --- Per-item processors -----------------------------------------------------
const ratifyItem = (it) =>
  agent(
    `${CANON}\n${CLAIM(it.id)}\nAs ${it.steering_role || STEERING.formal}, run the design-falsification + sign-off pass on the in_review ` +
      `design item ${it.id} ("${it.title}"). Try to break it; if it survives, RATIFY: record the sign-off in .project/decisions/, set ` +
      `the ADR status to accepted, set the board item status to done, and commit (board:). This unblocks dependent implementation. ` +
      `If it fails, send back with specific blocking findings (status blocked/changes).`,
    { label: `ratify:${it.id}`, phase: 'Ratify', model: 'opus', agentType: it.steering_role || STEERING.formal, schema: RESULT_SCHEMA }
  )

const relandItem = (it) =>
  agent(
    `${CANON}\n${CLAIM(it.id)}\nAs the integrator, RELAND ${it.id}${it.branch ? ` (branch ${it.branch})` : ''}: rebase/merge latest main into the ` +
      `branch, resolve the additive conflict (typically the "pub mod ...;" ordering in src/lib.rs — keep BOTH modules, sorted), re-run ` +
      `./format_code.sh + cargo test in the worktree, then ${LANDLOCK} run scripts/pr/land.sh ${it.id}, release the lock, set the board item ` +
      `done with the landing sha (board:). On a genuine test/review failure, file a BUG and leave blocked.`,
    { label: `reland:${it.id}`, phase: 'Build', model: 'sonnet', agentType: 'integrator', schema: RESULT_SCHEMA }
  )

const designItem = (it) =>
  agent(
    `${CANON}\n${CLAIM(it.id)}\nAs ${it.role}, do ${it.id} ("${it.title}"): produce the spec/ADR/model/research, commit it (board:), and set ` +
      `the board item to in_review with a steering sign-off request in .project/decisions/ so it is ratified next round. Resolve any ` +
      `bug/spike findings folded into your scope inline.`,
    { label: `design:${it.id}`, phase: 'Design', model: 'opus', agentType: it.role, schema: RESULT_SCHEMA }
  )

// Code item: implement → review → pre-mortem → LAND, all per-item (no cross-item barrier).
function buildAndLand(items) {
  return pipeline(
    items,
    // Stage 1 — implement TDD-first in an isolated worktree, open a simulated PR.
    (it) =>
      agent(
        `${CANON}\n${CLAIM(it.id)}\nAs ${it.role}, implement board item ${it.id} ("${it.title}") TDD-first in an isolated worktree ` +
          `(branch work/${it.id}-<slug>; base it on the LATEST main). Run scripts/env/up.sh + bucket.sh for integration tests. Open a ` +
          `simulated PR (scripts/pr/open.sh), ./format_code.sh green, set status in_review. Return status pr_opened + branch (or skipped).`,
        { label: `impl:${it.id}`, phase: 'Build', model: 'opus', agentType: it.role, isolation: 'worktree', schema: RESULT_SCHEMA }
      ).then((impl) => ({ it, impl })),
    // Stage 2 — adversarial review.
    (prev) => {
      if (!prev.impl || prev.impl.status !== 'pr_opened') return { ...prev, skip: true }
      return agent(
        `${CANON}\nAdversarially review the PR for ${prev.it.id} on branch ${prev.impl.branch}. Try to break it (correctness, ACID, ` +
          `latency theorem, security, simplicity). 'looks fine' is never a sign-off. Tick the reviewer box in PR.md only on approve.`,
        { label: `review:${prev.it.id}`, phase: 'Build', model: 'opus', agentType: 'adversarial-reviewer', schema: VERDICT_SCHEMA }
      ).then((review) => ({ ...prev, review }))
    },
    // Stage 3 — pre-mortem.
    (prev) => {
      if (prev.skip || !prev.review || prev.review.verdict !== 'approve') return prev
      return agent(
        `${CANON}\nPre-mortem the PR for ${prev.it.id}: assume it shipped and caused an incident — enumerate failure modes; gate on ` +
          `mitigations. Tick the pre-mortem box in PR.md only on approve.`,
        { label: `premortem:${prev.it.id}`, phase: 'Build', model: 'opus', agentType: 'premortem-analyst', schema: VERDICT_SCHEMA }
      ).then((premortem) => ({ ...prev, premortem }))
    },
    // Stage 4 — LAND immediately if approved (serialized by the global land-lock). No waiting on sibling items.
    (prev) => {
      const approved = !prev.skip && prev.review && prev.review.verdict === 'approve' && prev.premortem && prev.premortem.verdict === 'approve'
      if (!approved) return { id: prev.it.id, status: 'in_review', notes: 'not approved this round' }
      return agent(
        `${CANON}\nAs the integrator, LAND ${prev.it.id} (branch ${prev.impl.branch}). Verify both sign-offs are ticked in PR.md. ` +
          `${LANDLOCK} Then run scripts/pr/land.sh ${prev.it.id} (runs ./format_code.sh + cargo test, merges to main, cleans the worktree), ` +
          `release the lock, set the board item done with the landing sha (board:). On failure file a BUG and leave blocked.`,
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
    log(`Lane ${LANE}: budget floor reached (${Math.round(budget.remaining() / 1000)}k left) — stopping new work.`)
    break
  }

  const ratifyNow = (state.ratify_now || []).slice(0, ROUND_CAP)
  const reland = (state.reland || []).slice(0, ROUND_CAP)
  const code = (state.code_ready || []).slice(0, ROUND_CAP)
  const design = (state.design_ready || []).slice(0, ROUND_CAP)
  const total = ratifyNow.length + reland.length + code.length + design.length

  if (total === 0) {
    idle++
    log(`Lane ${LANE} round ${round}: no claimable work (board blocked or drained). idle=${idle}.`)
    if (idle >= 2) break
    state = await orient('Re-orient — nothing was claimable last round; check for newly-unblocked work.')
    if (!state || state.stop) break
    continue
  }
  idle = 0
  log(`Lane ${LANE} round ${round}: ratify=${ratifyNow.length} reland=${reland.length} code=${code.length} design=${design.length}`)

  // Everything in a round runs concurrently; the per-item land/sign-off needs no cross-item barrier.
  const [rRes, reRes, cRes, dRes] = await Promise.all([
    ratifyNow.length ? parallel(ratifyNow.map((it) => () => ratifyItem(it))) : Promise.resolve([]),
    reland.length ? parallel(reland.map((it) => () => relandItem(it))) : Promise.resolve([]),
    code.length ? buildAndLand(code) : Promise.resolve([]),
    design.length ? parallel(design.map((it) => () => designItem(it))) : Promise.resolve([]),
  ])

  ratified.push(...rRes.filter(Boolean).filter((r) => r.status === 'done').map((r) => r.id))
  landed.push(...[...reRes, ...cRes].filter(Boolean).filter((r) => r.status === 'done').map((r) => r.id))
  designed.push(...dRes.filter(Boolean).filter((r) => r.status && r.status !== 'skipped').map((r) => r.id))

  // Re-pull: ratified design items have unblocked dependents; landed code may unblock more.
  state = await orient('Re-orient — pull work unblocked by this round’s lands + ratifications.')
  if (!state || state.stop) break
}

log(`Lane ${LANE} done after ${round} rounds: landed ${landed.length}, ratified ${ratified.length}, design ${designed.length}.`)
return { ok: true, lane: LANE, rounds: round, landed, ratified, designed }
