#!/usr/bin/env bash
# ==============================================================================
# Shared cluster lifecycle helpers for Batata dev/test scripts.
#
# Source this file to get the canonical 3-node cluster layout (ports, member
# list) plus small helpers used by startup.sh (cluster subcommand),
# test.sh (sdk --matrix / all stages). Centralizing
# these avoids the same magic numbers being copy-pasted (and drifting) across
# launchers.
#
# Usage (in a sourcing script):
#   source "$(dirname "$0")/cluster_lib.sh"
# ==============================================================================

# Node / console / consul / apollo port layout (shared across all cluster launchers).
# Node i  : main port 8848 + 1000, console 8081 + 1000, consul 8500 + 1000,
#           apollo 18080 + 100.
CLUSTER_NODE_PORTS=(8848 8858 8868)
CLUSTER_CONSOLE_PORTS=(8081 8082 8083)
CLUSTER_CONSUL_PORTS=(8500 8510 8520)
CLUSTER_APOLLO_PORTS=(18080 18090 18100)

# Comma-separated member list used for --batata.member.list.
CLUSTER_MEMBER_LIST="127.0.0.1:8848,127.0.0.1:8858,127.0.0.1:8868"

# Write the standard 3-node cluster.conf into <project_root>/conf/cluster.conf.
# Usage: cluster_write_conf <project_root>
cluster_write_conf() {
    local root="$1"
    cat > "${root}/conf/cluster.conf" <<EOF
127.0.0.1:8848
127.0.0.1:8858
127.0.0.1:8868
EOF
}

# Best-effort kill of anything listening on the given ports (used for cleanup).
# Usage: cluster_kill_ports <port> [port...]
cluster_kill_ports() {
    for port in "$@"; do
        lsof -ti:"${port}" 2>/dev/null | xargs kill -9 2>/dev/null || true
    done
}
