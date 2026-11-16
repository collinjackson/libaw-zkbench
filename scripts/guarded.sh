#!/bin/sh
# Runs a command and kills it if its resident memory passes a cap (default 24 GB), so a zkVM
# executor that keeps its trace in RAM can't swap the machine to a halt. The memory counted is
# the command's and all its descendants' (RISC Zero's host runs the zkVM in an r0vm child).
#   CAP_GB=16 scripts/guarded.sh ./target/release/referee-nexus-host ../../workloads/move.json
CAP_KB=$(( ${CAP_GB:-24} * 1024 * 1024 ))
"$@" & pid=$!
tree() {
  echo "$1"
  for c in $(pgrep -P "$1" 2>/dev/null); do tree "$c"; done
}
peak=0
while kill -0 "$pid" 2>/dev/null; do
  pids=$(tree "$pid" | paste -sd, -)
  rss=$(ps -o rss= -p "$pids" 2>/dev/null | awk '{s += $1} END {print s + 0}')
  if [ "$rss" -gt 0 ]; then
    [ "$rss" -gt "$peak" ] && peak=$rss
    if [ "$rss" -gt "$CAP_KB" ]; then
      for p in $(tree "$pid"); do kill -9 "$p" 2>/dev/null; done
      echo "guarded: killed at $((rss / 1024)) MB (cap ${CAP_GB:-24} GB)" >&2
      exit 137
    fi
  fi
  sleep 2
done
wait "$pid"; code=$?
echo "guarded: peak $((peak / 1024)) MB" >&2
exit $code
