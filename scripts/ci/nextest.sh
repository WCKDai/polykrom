#!/usr/bin/env bash
# The tests Polykrom CI runs, in one place: the workflow's jobs call this, and so can anyone who wants
# to run exactly what a pull request will run before pushing it (`scripts/ci/nextest.sh pr all`).
#
# Usage: scripts/ci/nextest.sh <pr|main> <part> [extra nextest arguments, e.g. --partition count:1/4]
#
# Sets: `pr` skips the heavy tests (default-filter in .config/nextest.toml) but still runs the
# robustness goldens, all but the slowest seeds; `main` runs every test.
#
# Parts (together they cover the same tests as `all`, which runs them in one nextest run):
#   engine         polykrom-engine's unit tests (the kernel lives in PolyCore and is tested there)
#   ui             polykrom-ui-egui
#   rest           every other package, but not the action matrix sample
#   action-matrix  the action matrix sample on its own: it takes every core for minutes
#   all            all of the above; on main also the engine's own test binaries (crates/engine/tests)
set -euo pipefail

set_name=${1:?usage: nextest.sh <pr|main> <part> [nextest arguments]}
part=${2:?usage: nextest.sh <pr|main> <part> [nextest arguments]}
shift 2

# The slowest robustness goldens: pull requests leave them to main.
slow_seeds='81|45|20|24|95|02|57|40|18|48|32|46|85|61|77|34|14'
case "$set_name" in
  pr) base="default() | (package(polykrom-engine) & test(/^tests::robustness_goldens::/) & not test(/^tests::robustness_goldens::seed_($slow_seeds)\$/))" ;;
  main) base='all()' ;;
  *) echo "nextest.sh: the set is pr or main, not '$set_name'" >&2; exit 2 ;;
esac

sample='binary(action_matrix) & test(=action_matrix_sample)'
rest=(--workspace --exclude polykrom-engine --exclude polykrom-ui-egui)
case "$part" in
  engine) scope=(-p polykrom-engine --lib); filter="$base" ;;
  ui) scope=(-p polykrom-ui-egui); filter="$base" ;;
  rest) scope=("${rest[@]}"); filter="($base) & not ($sample)" ;;
  action-matrix) scope=("${rest[@]}"); filter="($base) & ($sample)" ;;
  # Pull requests: exactly the parts together, so without the engine's own test binaries
  # (crates/engine/tests, the engine part is --lib). Main runs those too (#327).
  all)
    scope=(--workspace)
    if [ "$set_name" = main ]; then filter="$base"; else filter="($base) & not (package(polykrom-engine) & not kind(lib))"; fi
    ;;
  *) echo "nextest.sh: no part '$part' (engine, ui, rest, action-matrix, all)" >&2; exit 2 ;;
esac

# Pull requests test only what their change can affect: the workflow's `changes` job runs
# scripts/ci/affected.py and passes AFFECTED_FULL and AFFECTED_PACKAGES. Each part narrows to the
# affected packages it covers, and a part with none passes at once (its check must still report,
# or a required check would wait for it). Main always runs everything.
if [ "$set_name" = pr ] && [ "${AFFECTED_FULL:-true}" != true ]; then
  affected=()
  for p in ${AFFECTED_PACKAGES:-}; do
    case "$part:$p" in
      engine:polykrom-engine | ui:polykrom-ui-egui | action-matrix:polykrom-cli | all:*) affected+=("$p") ;;
      rest:polykrom-engine | rest:polykrom-ui-egui) ;;
      rest:*) affected+=("$p") ;;
    esac
  done
  if [ ${#affected[@]} -eq 0 ]; then
    echo "nextest.sh: this change affects no package in the $part part; nothing to run"
    exit 0
  fi
  case "$part" in
    engine | ui) ;;
    *)
      scope=()
      for p in "${affected[@]}"; do scope+=(-p "$p"); done
      ;;
  esac
fi

set -x
exec cargo nextest run --locked --cargo-profile ci --profile ci --no-tests=pass --ignore-default-filter -E "$filter" "${scope[@]}" "$@"
