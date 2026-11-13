# Resolve the relate-flow binary. Source this; do not execute it.
# RELATE_FLOW_BIN overrides the default, which is the host release build
# at <repo>/target/release/relate-flow. The gosonic build stage writes a
# Linux binary to target/sonic/release/relate-flow and does not replace this one.
examples_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$examples_dir/.." && pwd)"
: "${RELATE_FLOW_BIN:=$repo/target/release/relate-flow}"
if [[ ! -x "$RELATE_FLOW_BIN" ]]; then
  echo "relate-flow binary is not executable at $RELATE_FLOW_BIN" >&2
  echo "build it with: cargo build --release -p relate-flow" >&2
  exit 1
fi

# A plan script executed on its own, or a run script that did not get a store
# from run-scenario.sh, uses <scenario>/.relate-flow and starts empty.
# A caller that set RELATE_FLOW_DATABASE keeps that store.
example_store() {
  if [[ -n "${RELATE_FLOW_DATABASE:-}" ]]; then
    return 0
  fi
  local plan="${BASH_SOURCE[1]}"
  local dir
  dir="$(cd "$(dirname "$plan")" && pwd)"
  export RELATE_FLOW_DATABASE="$dir/.relate-flow"
  rm -rf "$RELATE_FLOW_DATABASE"
  echo "store: $RELATE_FLOW_DATABASE" >&2
}

# Call relate-flow on $PLAN. PLAN is read when rf runs, so begin_run can retarget it.
# RELATE_FLOW_VERBOSE=1 prints the command on stderr before the output.
# RELATE_FLOW_OUTPUT=text adds --output text to next and show.
rf() {
  local args=("$@")
  if [[ "${RELATE_FLOW_OUTPUT:-}" == "text" && ( "$1" == "next" || "$1" == "show" ) ]]; then
    args+=(--output text)
  fi
  if [[ -n "${RELATE_FLOW_VERBOSE:-}" ]]; then
    printf '+' >&2
    printf ' %q' "$RELATE_FLOW_BIN" "$PLAN" "${args[@]}" >&2
    printf '\n' >&2
  fi
  "$RELATE_FLOW_BIN" "$PLAN" "${args[@]}"
}

# Copy the committed template into a named execution and point later rf calls at it.
# Usage: begin_run NAME [--input KEY=VALUE]...
begin_run() {
  local name="$1"
  shift
  rf execute "$name" "$@"
  PLAN="$name"
}
