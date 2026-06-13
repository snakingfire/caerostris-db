export const meta = {
  name: 'mainspring',
  description:
    'caerostris-db autonomous build orchestrator — one bounded epoch: orient, (bootstrap if first run), then a wave of design/research/implement→review→pre-mortem→land work pulled from the file board. Relaunched repeatedly by the pace-marshal cron until the .project/STOP sentinel appears.',
  phases: [
    { title: 'Orient' },
    { title: 'Bootstrap', detail: 'steering ratifies intent+rubric; planner decomposes the board (first epoch only)' },
    { title: 'Design & Research', detail: 'specs, spikes, formal models, datasets — committed directly, design gets steering sign-off' },
    { title: 'Build', detail: 'implement→adversarial-review→pre-mortem in worktrees (parallel)' },
    { title: 'Land', detail: 'integrator lands approved PRs serially onto main (single-writer)' },
  ],
}

// ---------------------------------------------------------------------------
// caerostris-db mainspring — v1 floor. EPIC-010 / T-0004 hardens this.
// The script cannot read the filesystem or wallclock directly, so it dispatches
// an "orient" agent to read board/pace/sentinels and returns structured state.
// The pace-marshal cron relaunches the next epoch and writes .project/STOP at the
// deadline. Keep this correct and simple; let the swarm improve it.
// ---------------------------------------------------------------------------

const CANON =
  'Read docs/commanders-intent.md, docs/requirements/master-rubric.md, docs/process/autonomous-operating-model.md, ' +
  'docs/process/task-board-protocol.md and your own agent definition before acting. Follow the open-source guardrails ' +
  '(docs/process/open-source-guardrails.md): public repo, never commit secrets/data, license-clean only. ' +
  'Keep the board honest, watch .project/pace/deadline.md, run ./format_code.sh green before landing, never block the board.'

const MAX_CODE = (args && args.maxCode) || 8 // code tasks per epoch (each ⇒ implement+review+premortem)
const MAX_NONCODE = (args && args.maxNonCode) || 6 // design/research/formal/dataset tasks per epoch

const CODE_ROLES = ['implementer', 'test-author', 'perf-engineer']

const ORIENT_SCHEMA = {
  type: 'object',
  required: ['stop', 'bootstrapped', 'ready', 'in_review', 'behind_pace', 'summary'],
  properties: {
    stop: { type: 'boolean', description: 'true if .project/STOP exists' },
    bootstrapped: { type: 'boolean', description: 'true if .project/.bootstrapped exists' },
    behind_pace: { type: 'boolean', description: 'true if actual rubric score is behind the .project/pace/deadline.md marker' },
    ready: {
      type: 'array',
      description: 'READY board items, highest priority first, capped at ~30',
      items: {
        type: 'object',
        required: ['id', 'role', 'priority', 'title'],
        properties: {
          id: { type: 'string' },
          title: { type: 'string' },
          priority: { type: 'string', enum: ['P0', 'P1', 'P2', 'P3'] },
          role: {
            type: 'string',
            description:
              'agentType to handle this item: implementer | test-author | perf-engineer (these produce code ⇒ PR pipeline) | researcher | dataset-scout | formal-prover | planner-decomposer | docs-memory-curator | steering-storage | steering-query-cypher | steering-distributed-acid | steering-formal-methods | steering-perf-sla',
          },
          needs_steering_signoff: { type: 'boolean', description: 'true for design/spec/ADR items that must be ratified' },
        },
      },
    },
    in_review: { type: 'array', items: { type: 'string' }, description: 'ids of items left in_review from a prior epoch' },
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
    status: { type: 'string', enum: ['pr_opened', 'committed', 'blocked', 'failed', 'done'] },
    branch: { type: 'string', description: 'work/<ID>-<slug> if a PR was opened' },
    notes: { type: 'string' },
  },
}

// --- Phase: Orient -----------------------------------------------------------
phase('Orient')
const state = await agent(
  `${CANON}\nORIENT the swarm. Read .project/STOP (exists?), .project/.bootstrapped (exists?), the board under ` +
    `.project/board/tasks/ (use scripts/board/ls.sh or rg over frontmatter), the latest report in .project/reports/, ` +
    `and .project/pace/deadline.md. Return: whether to stop, whether bootstrap has happened, the READY items (highest ` +
    `priority first, cap ~30) each tagged with the best agentType 'role' to handle it and whether it needs steering sign-off, ` +
    `any items stuck in_review, whether we are behind pace, and a one-line summary. Do not modify anything.`,
  { label: 'orient', phase: 'Orient', model: 'sonnet', agentType: 'pace-marshal', schema: ORIENT_SCHEMA }
)

if (!state) {
  log('Orient failed — aborting this epoch; marshal will relaunch.')
  return { ok: false, reason: 'orient_failed' }
}
if (state.stop) {
  log('STOP sentinel present — standing down. Run complete.')
  return { ok: true, stopped: true }
}
log(`Oriented: bootstrapped=${state.bootstrapped} ready=${state.ready.length} behind_pace=${state.behind_pace} — ${state.summary}`)

// --- Phase: Bootstrap (first epoch only) ------------------------------------
if (!state.bootstrapped) {
  phase('Bootstrap')
  log('First epoch — ratifying foundations and decomposing the board.')

  // Steering ratifies intent + rubric + the latency-envelope framing in parallel.
  // This is a fast adversarial sanity pass; findings are filed but do NOT hard-block the run.
  const STEERING = [
    'steering-storage',
    'steering-query-cypher',
    'steering-distributed-acid',
    'steering-formal-methods',
    'steering-perf-sla',
  ]
  const ratify = await parallel(
    STEERING.map((s) => () =>
      agent(
        `${CANON}\nRATIFICATION PASS. As ${s}, adversarially review docs/commanders-intent.md and ` +
          `docs/requirements/master-rubric.md for anything in YOUR domain that is infeasible, contradictory, or under-specified ` +
          `— especially the latency selectivity-envelope theorem and the GATE categories you own. If you find a blocking issue, ` +
          `file it as a P0 board item (scripts/board/new-task.sh) and record an entry in .project/decisions/. Then APPROVE so the ` +
          `run proceeds (we do not hard-block the launch; we surface and track). Return your verdict.`,
        { label: `ratify:${s}`, phase: 'Bootstrap', model: 'opus', agentType: s, schema: VERDICT_SCHEMA }
      )
    )
  )
  const objections = ratify.filter(Boolean).filter((v) => v.verdict !== 'approve').length
  log(`Steering ratification: ${ratify.filter(Boolean).length}/5 responded, ${objections} raised blocking findings (filed to board).`)

  // Planner decomposes every epic into ready stories/tasks (design-before-code ordering).
  await agent(
    `${CANON}\nAs the planner, execute T-0003: decompose EVERY epic in .project/board/tasks/ into ready stories/tasks with ` +
      `deps, rubric_refs, priority and acceptance criteria. Enforce design-before-code: implementation tasks that depend on an ` +
      `unratified design (SPIKE-0001 latency envelope, SPIKE-0002 commit protocol, SPIKE-0003 storage format) stay backlog until ` +
      `those are done. Prefer many small tasks. Commit your board edits (board: ...). Then create .project/.bootstrapped to mark ` +
      `bootstrap complete. Return a one-line summary of how many items now exist.`,
    { label: 'plan:decompose', phase: 'Bootstrap', model: 'opus', agentType: 'planner-decomposer' }
  )

  // Re-orient now that the board is populated.
  const reoriented = await agent(
    `${CANON}\nRe-ORIENT after planning: return the current READY items (priority order, cap ~30) tagged with role + ` +
      `needs_steering_signoff, plus stop/bootstrapped/behind_pace and a one-line summary.`,
    { label: 'orient:post-plan', phase: 'Orient', model: 'sonnet', agentType: 'pace-marshal', schema: ORIENT_SCHEMA }
  )
  if (reoriented && reoriented.ready) state.ready = reoriented.ready
}

// --- Partition this epoch's ready work --------------------------------------
const ready = (state.ready || []).filter(Boolean)
const codeTasks = ready.filter((t) => CODE_ROLES.includes(t.role)).slice(0, MAX_CODE)
const nonCodeTasks = ready.filter((t) => !CODE_ROLES.includes(t.role)).slice(0, MAX_NONCODE)

if (codeTasks.length === 0 && nonCodeTasks.length === 0) {
  log('No ready work this epoch (board may be blocked on design or fully drained). Marshal will re-groom and relaunch.')
  return { ok: true, idle: true, in_review: state.in_review || [] }
}
log(`Epoch plan: ${codeTasks.length} code tasks, ${nonCodeTasks.length} design/research tasks.`)

// --- Phase: Design & Research (non-code, runs concurrently with Build) -------
const nonCodePromise = parallel(
  nonCodeTasks.map((t) => () =>
    agent(
      `${CANON}\nExecute board item ${t.id} ("${t.title}"). Claim it on the board, do the work per your role, commit your ` +
        `output (spec/ADR/model/research note/dataset plan) with a clear message, and update the board item.` +
        (t.needs_steering_signoff
          ? ' This is a DESIGN item: after committing, it requires steering sign-off via the design-falsification loop before ' +
            'dependent code becomes ready — record the sign-off request in .project/decisions/ and leave status in_review.'
          : '') +
        ' Return your result.',
      { label: `${t.role}:${t.id}`, phase: 'Design & Research', model: t.role.startsWith('steering') || t.role === 'formal-prover' ? 'opus' : 'sonnet', agentType: t.role, schema: RESULT_SCHEMA }
    )
  )
)

// Steering sign-off for the design items produced above (after they're committed).
const signoffPromise = (async () => {
  const designItems = nonCodeTasks.filter((t) => t.needs_steering_signoff)
  if (designItems.length === 0) return []
  await nonCodePromise // designs must be committed before they can be ratified
  return parallel(
    designItems.map((t) => () =>
      agent(
        `${CANON}\nAs the responsible steering member, run the design-falsification loop on board item ${t.id}: try to REFUTE ` +
          `the committed design (does it break the latency theorem, ACID, or an interface contract?). If it survives, APPROVE, ` +
          `record an ADR/decision, and flip the item + its dependent implementation tasks to ready. Otherwise request changes. Return your verdict.`,
        { label: `signoff:${t.id}`, phase: 'Design & Research', model: 'opus', agentType: 'steering-formal-methods', schema: VERDICT_SCHEMA }
      )
    )
  )
})()

// --- Phase: Build — implement → review → pre-mortem (parallel pipeline) -------
phase('Build')
const reviewed = await pipeline(
  codeTasks,
  // Stage 1: implement TDD-first in an isolated worktree and open a simulated PR.
  (t) =>
    agent(
      `${CANON}\nExecute board item ${t.id} ("${t.title}"). Claim it, create a worktree + PR via scripts/pr/open.sh ${t.id}, ` +
        `work TDD-first (failing test → implement → green), keep the diff small, fill PR.md (summary + test evidence), and leave ` +
        `status in_review. Return your result (status pr_opened + branch).`,
      { label: `build:${t.id}`, phase: 'Build', model: 'opus', agentType: t.role, isolation: 'worktree', schema: RESULT_SCHEMA }
    ).then((r) => ({ task: t, impl: r })),
  // Stage 2: adversarial code review of the diff.
  (prev) =>
    agent(
      `${CANON}\nAdversarially review the PR for board item ${prev.task.id} on branch ${prev.impl && prev.impl.branch}. Try to ` +
        `BREAK it: correctness, ACID, the latency theorem, security, simplicity. Check the acceptance criteria. Emit a verdict; ` +
        `'looks fine' is never a sign-off. Tick the reviewer sign-off box in PR.md only on approve.`,
      { label: `review:${prev.task.id}`, phase: 'Build', model: 'opus', agentType: 'adversarial-reviewer', schema: VERDICT_SCHEMA }
    ).then((review) => ({ ...prev, review })),
  // Stage 3: pre-mortem.
  (prev) =>
    agent(
      `${CANON}\nPre-mortem the PR for board item ${prev.task.id}: assume it shipped and caused an incident — enumerate failure ` +
        `modes and gate on mitigations. Tick the pre-mortem sign-off box in PR.md only on approve. Emit a verdict.`,
      { label: `premortem:${prev.task.id}`, phase: 'Build', model: 'opus', agentType: 'premortem-analyst', schema: VERDICT_SCHEMA }
    ).then((premortem) => ({
      ...prev,
      premortem,
      approved: prev.review && prev.review.verdict === 'approve' && premortem && premortem.verdict === 'approve',
    }))
)

const built = reviewed.filter(Boolean)
const approved = built.filter((r) => r.approved)
const sentBack = built.filter((r) => !r.approved)
log(`Build wave: ${built.length} reviewed, ${approved.length} approved, ${sentBack.length} sent back for changes.`)

// --- Phase: Land — integrator merges approved PRs serially (single-writer) ---
phase('Land')
const landed = []
for (const r of approved) {
  const res = await agent(
    `${CANON}\nAs the integrator, LAND board item ${r.task.id} (branch ${r.impl && r.impl.branch}). Verify reviewer + pre-mortem ` +
      `sign-offs are ticked in PR.md, run scripts/pr/land.sh ${r.task.id} (it runs ./format_code.sh + cargo test, then merges to ` +
      `main serially and cleans the worktree). On success set the board item done with the landing sha; on failure file a BUG and ` +
      `leave the item blocked. Return your result.`,
    { label: `land:${r.task.id}`, phase: 'Land', model: 'sonnet', agentType: 'integrator', schema: RESULT_SCHEMA }
  )
  if (res) landed.push(res)
}

const nonCode = (await nonCodePromise).filter(Boolean)
const signoffs = (await signoffPromise).filter(Boolean)

log(`Epoch complete: landed ${landed.length} PRs, ${nonCode.length} design/research items, ${signoffs.length} steering sign-offs.`)
return {
  ok: true,
  landed: landed.map((r) => r.id),
  sent_back: sentBack.map((r) => r.task.id),
  design_items: nonCode.map((r) => r.id),
  in_review: built.filter((r) => !r.approved).map((r) => r.task.id),
}
