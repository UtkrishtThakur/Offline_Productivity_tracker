#!/usr/bin/env bash

set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$PROJECT_ROOT/py-analyzer"

case "$1" in
    start)
        echo "[*] Starting analyzer..."
        docker compose up -d --build
        ;;

    stop)
        echo "[*] Stopping analyzer..."
        docker compose down
        ;;

    restart)
        echo "[*] Restarting analyzer..."
        docker compose down
        docker compose up -d --build
        ;;

    logs)
        docker compose logs -f
        ;;

    status)
        docker ps | grep tracker-ai || true
        ;;

    shell)
        docker exec -it tracker-ai sh
        ;;

    rebuild)
        echo "[*] Rebuilding analyzer..."
        docker compose build --no-cache
        ;;

    *)
        echo ""
        echo "Usage:"
        echo "  ./scripts/docker.sh start"
        echo "  ./scripts/docker.sh stop"
        echo "  ./scripts/docker.sh restart"
        echo "  ./scripts/docker.sh logs"
        echo "  ./scripts/docker.sh status"
        echo "  ./scripts/docker.sh shell"
        echo "  ./scripts/docker.sh rebuild"
        echo ""
        ;;
esac