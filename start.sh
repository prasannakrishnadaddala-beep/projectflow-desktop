#!/bin/bash
set -e
export PORT="${PORT:-8080}"
echo "=== ProjectFlow starting on 0.0.0.0:$PORT ==="
exec /opt/venv/bin/python -m gunicorn app:app \
  --bind "0.0.0.0:${PORT}" \
  --workers 1 \
  --worker-class sync \
  --threads 4 \
  --timeout 120 \
  --access-logfile - \
  --error-logfile - \
  --log-level info
