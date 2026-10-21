#!/usr/bin/env bash
set -euo pipefail

container_id=$(sudo docker-compose ps -q r1)
if [[ -z "$container_id" ]]; then
    echo "No running r1 container found; start the stack first." >&2
    exit 1
fi

sudo docker exec "$container_id" agt-control "$@"
