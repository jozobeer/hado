#!/usr/bin/env bash
# tier: quick
# round: ci
# CI（.github/workflows/ci.yml と scripts/ci-run.sh）の判定器。network を使わない。
#
# 測るのは 3 つ。
#   a. 層の選択。event と ref から層が決まり、PR は core 以上、main への push と
#      workflow_dispatch の既定は full。層の中身は scripts/lib/gate-tiers.sh から
#      計算する（門番の一覧をここに書かない）。
#   b. 入口の振る舞い。層と文書層が走り、赤は隠さない。知らない event・ref・層は
#      回さずに落ちる。
#   c. ワークフローの配線。YAML として読み、入口を呼び、依存の取得・libgc-dev・
#      キャッシュ・打ち切り・失敗ログを持つ。層と門番の一覧をワークフローに
#      書かない。
#
# 判定 a・b は合成木で測る。scripts/ci-run.sh と scripts/lib/gate-tiers.sh だけを
# 写し、run-gates と lint-docs を「受けた引数を記録する stub」に置き換える。
# 期待値は stub が受けた引数そのものである。
set -uo pipefail
cd "$(dirname "$0")/.."

passed=0
total=0
check() {
    local label=$1
    shift
    total=$((total + 1))
    if "$@"; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label"
    fi
}

workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT
record="$workdir/record"
tree="$workdir/tree"
out="$workdir/out"
stdout="$workdir/stdout"
stderr="$workdir/stderr"

# --- 合成木 ---
mkdir -p "$tree/scripts/lib"
cp scripts/ci-run.sh "$tree/scripts/"
cp scripts/lib/gate-tiers.sh "$tree/scripts/lib/"
cat >"$tree/scripts/run-gates.sh" <<'EOS'
#!/usr/bin/env bash
printf 'run-gates %s\n' "$*" >>"$HADO_CI_TEST_RECORD"
exit "${HADO_CI_STUB_RUN_GATES_STATUS:-0}"
EOS
cat >"$tree/scripts/lint-docs.sh" <<'EOS'
#!/usr/bin/env bash
printf 'lint-docs\n' >>"$HADO_CI_TEST_RECORD"
exit "${HADO_CI_STUB_LINT_DOCS_STATUS:-0}"
EOS

# --- 層は scripts/lib/gate-tiers.sh から取る（一覧を持たない） ---
gates_of() {
    ( . scripts/lib/gate-tiers.sh; hado_tier_gates "$1" | LC_ALL=C sort )
}
all_gates() {
    ( . scripts/lib/gate-tiers.sh; hado_tier_all_gates | LC_ALL=C sort )
}
tiers() {
    ( . scripts/lib/gate-tiers.sh; hado_tiers )
}
tier_covers() {
    local tier=$1 want=$2
    [ -z "$(comm -23 <(gates_of "$want") <(gates_of "$tier"))" ]
}
tier_is_full() {
    diff -q <(gates_of "$1") <(all_gates) >/dev/null
}

# 合成木の入口を 1 回走らせ、stub が受けた層を返す。記録は $record に残る。
ci_tier_of() {
    local event=$1 ref=$2 requested=${3:-}
    : >"$record"
    if ! (
        cd "$tree" || exit 1
        HADO_CI_TEST_RECORD="$record" HADO_CI_JOBS=1 HADO_CI_OUT="$out" \
            bash scripts/ci-run.sh "$event" "$ref" "$requested"
    ) >"$stdout" 2>"$stderr"; then
        return 1
    fi
    awk '$1 == "run-gates" { for (i = 1; i <= NF; i++) if ($i == "--tier") print $(i + 1) }' "$record"
}

# --- a: 層の選択 ---
pull_request_at_least_core() {
    local tier
    tier=$(ci_tier_of pull_request refs/pull/1/merge "") || return 1
    [ -n "$tier" ] && tier_covers "$tier" core
}

main_push_is_full() {
    local tier
    tier=$(ci_tier_of push refs/heads/main "") || return 1
    [ -n "$tier" ] && tier_is_full "$tier"
}

dispatch_default_is_full() {
    local tier
    tier=$(ci_tier_of workflow_dispatch refs/heads/topic "") || return 1
    [ -n "$tier" ] && tier_is_full "$tier"
}

dispatch_uses_requested() {
    local tier got
    while IFS= read -r tier; do
        got=$(ci_tier_of workflow_dispatch refs/heads/topic "$tier") || return 1
        [ "$got" = "$tier" ] || return 1
    done < <(tiers)
}

# --- b: 入口の振る舞い ---
doc_layer_always_runs() {
    local event ref
    for event in pull_request push workflow_dispatch; do
        case "$event" in
            pull_request) ref=refs/pull/1/merge ;;
            *) ref=refs/heads/main ;;
        esac
        : >"$record"
        if ! (
            cd "$tree" || exit 1
            HADO_CI_TEST_RECORD="$record" HADO_CI_JOBS=1 HADO_CI_OUT="$out" \
                bash scripts/ci-run.sh "$event" "$ref" ""
        ) >"$stdout" 2>"$stderr"; then
            return 1
        fi
        grep -q '^lint-docs$' "$record" || return 1
    done
}

refuses_unknown_before_running() {
    local spec event ref requested status
    for spec in 'schedule refs/heads/main' 'push refs/heads/topic' 'workflow_dispatch refs/heads/topic nope'; do
        set -- $spec
        event=$1
        ref=$2
        requested=${3:-}
        : >"$record"
        (
            cd "$tree" || exit 1
            HADO_CI_TEST_RECORD="$record" HADO_CI_JOBS=1 HADO_CI_OUT="$out" \
                bash scripts/ci-run.sh "$event" "$ref" "$requested"
        ) >"$stdout" 2>"$stderr"
        status=$?
        # 知らない入力は 1 本も実走せずに落ちる（fail closed）。
        [ "$status" -ne 0 ] || return 1
        ! grep -q '^run-gates' "$record" || return 1
    done
}

red_is_not_hidden() {
    local status
    : >"$record"
    (
        cd "$tree" || exit 1
        HADO_CI_TEST_RECORD="$record" HADO_CI_JOBS=1 HADO_CI_OUT="$out" \
            HADO_CI_STUB_RUN_GATES_STATUS=1 \
            bash scripts/ci-run.sh pull_request refs/pull/1/merge ""
    ) >"$stdout" 2>"$stderr"
    status=$?
    # 門番が赤なら赤。文書層はそれでも走る。
    [ "$status" -ne 0 ] || return 1
    grep -q '^lint-docs$' "$record" || return 1
    : >"$record"
    (
        cd "$tree" || exit 1
        HADO_CI_TEST_RECORD="$record" HADO_CI_JOBS=1 HADO_CI_OUT="$out" \
            HADO_CI_STUB_LINT_DOCS_STATUS=1 \
            bash scripts/ci-run.sh pull_request refs/pull/1/merge ""
    ) >"$stdout" 2>"$stderr"
    # 文書層が赤でも赤。
    [ "$?" -ne 0 ]
}

# --- c: ワークフローの配線 ---
# ワークフローを YAML として読む python。runner の python3 が toolcache のものでも
# apt の python3-yaml（/usr/bin/python3）を使えるよう、yaml を import できる方を選ぶ。
yaml_python() {
    local candidate
    for candidate in python3 /usr/bin/python3; do
        if command -v "$candidate" >/dev/null 2>&1 && "$candidate" -c 'import yaml' >/dev/null 2>&1; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    return 1
}

workflow_case() {
    local python
    if ! python=$(yaml_python); then
        echo 'no python3 with yaml (install python3-yaml)' >&2
        return 1
    fi
    "$python" - <<'PY'
from pathlib import Path

import yaml


def fail(message):
    raise SystemExit(message)


path = Path('.github/workflows/ci.yml')
if not path.is_file():
    fail('missing .github/workflows/ci.yml')
try:
    doc = yaml.safe_load(path.read_text())
except yaml.YAMLError as exc:
    fail(f'not YAML: {exc}')
if not isinstance(doc, dict):
    fail('the workflow is not a mapping')

# `on:` は YAML 1.1 の真偽値として読まれる。どちらでも受ける。
triggers = doc.get('on')
if triggers is None:
    triggers = doc.get(True)
if not isinstance(triggers, dict):
    fail('on: is not a mapping')
if 'pull_request' not in triggers:
    fail('pull_request is not a trigger')
push = triggers.get('push')
if not isinstance(push, dict) or push.get('branches') != ['main']:
    fail(f'push is not limited to main: {push!r}')
if 'workflow_dispatch' not in triggers:
    fail('workflow_dispatch is not a trigger')

concurrency = doc.get('concurrency')
if not isinstance(concurrency, dict):
    fail('no concurrency')
if concurrency.get('cancel-in-progress') != "${{ github.event_name == 'pull_request' }}":
    fail('cancel-in-progress does not spare the main runs')

env = doc.get('env')
if not isinstance(env, dict) or str(env.get('CARGO_NET_OFFLINE')).lower() != 'true':
    fail('CARGO_NET_OFFLINE is not the default')

permissions = doc.get('permissions')
if not isinstance(permissions, dict) or permissions.get('contents') != 'read':
    fail('permissions do not default to contents: read')

jobs = doc.get('jobs')
if not isinstance(jobs, dict) or len(jobs) != 1:
    fail('jobs must be exactly one')
job = next(iter(jobs.values()))
if job.get('runs-on') != 'ubuntu-latest':
    fail(f'runs-on: {job.get("runs-on")!r} (the gates need Linux x86_64)')
if not isinstance(job.get('timeout-minutes'), int):
    fail('no timeout-minutes')


def run_text(step):
    return step.get('run') or ''


steps = job.get('steps')
if not isinstance(steps, list):
    fail('steps is not a list')

entry = [i for i, step in enumerate(steps) if 'scripts/ci-run.sh' in run_text(step)]
if len(entry) != 1:
    fail(f'scripts/ci-run.sh must be called from exactly one step: {entry!r}')
entry_index = entry[0]
entry_line = run_text(steps[entry_index]).strip()
if not entry_line.startswith('bash scripts/ci-run.sh '):
    fail(f'the entry is not a bash invocation: {entry_line!r}')
for value in ('github.event_name', 'github.ref', 'inputs.tier'):
    if value not in entry_line:
        fail(f'the entry does not pass {value}: {entry_line!r}')

fetch = [i for i, step in enumerate(steps) if 'cargo fetch --locked' in run_text(step)]
if len(fetch) != 1 or fetch[0] >= entry_index:
    fail('cargo fetch --locked must run exactly once, before the entry')
fetch_env = steps[fetch[0]].get('env')
if not isinstance(fetch_env, dict) or str(fetch_env.get('CARGO_NET_OFFLINE')).lower() != 'false':
    fail('the fetch step does not open the network')

if not any('libgc-dev' in run_text(step) for step in steps):
    fail('libgc-dev is not installed')
if not any(str(step.get('uses', '')).startswith('actions/cache@') for step in steps):
    fail('the cargo registry is not cached')
uploads = [step for step in steps if str(step.get('uses', '')).startswith('actions/upload-artifact@')]
if not any(step.get('if') == 'failure()' for step in uploads):
    fail('the gate logs are not uploaded on failure')

uses = [str(step['uses']) for step in steps if 'uses' in step]
if not uses or any(not value.startswith('actions/') for value in uses):
    fail(f'only first-party actions are allowed: {uses!r}')

for step in steps:
    text = run_text(step)
    if '--tier' in text:
        fail('the workflow must not choose a tier (scripts/ci-run.sh does)')
    if 'verify-' in text:
        fail('the workflow must not name gates (scripts/lib/gate-tiers.sh does)')
PY
}

check 'a pull_request runs at least the core tier' pull_request_at_least_core
check 'a push to main runs the full tier' main_push_is_full
check 'workflow_dispatch defaults to the full tier' dispatch_default_is_full
check 'workflow_dispatch runs the requested tier' dispatch_uses_requested
check 'every entry runs the document layer' doc_layer_always_runs
check 'an unknown event, ref, or tier is refused before any gate' refuses_unknown_before_running
check 'a red gate or a red document layer is not hidden' red_is_not_hidden
check 'the workflow parses and is wired for the decisions' workflow_case

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
