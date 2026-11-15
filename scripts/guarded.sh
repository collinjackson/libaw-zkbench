#!/bin/sh
# Runs a command and kills it if its resident memory passes a cap (default 24 GB), so a zkVM
# executor that keeps its trace in RAM can't swap the machine to a halt.
#   CAP_GB=16 scripts/guarded.sh ./target/release/referee-nexus-host ../../workloads/move.json
CAP_KB=$(( ${CAP_GB:-24} * 1024 * 1024 ))
"$@" & pid=$!
peak=0
while kill -0 "$pid" 2>/dev/null; do
  rss=$(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ')
  if [ -n "$rss" ]; then
    [ "$rss" -gt "$peak" ] && peak=$rss
    if [ "$rss" -gt "$CAP_KB" ]; then
      kill -9 "$pid"
      echo "guarded: killed at $((rss / 1024)) MB (cap ${CAP_GB:-24} GB)" >&2
      exit 137
    fi
  fi
  sleep 2
done
wait "$pid"; code=$?
echo "guarded: peak $((peak / 1024)) MB" >&2
exit $code
