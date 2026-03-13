#!/bin/bash
set -e
PORT="${PORT:-8080}"
echo "Starting ProjectFlow on 0.0.0.0:$PORT"
exec /opt/venv/bin/gunicorn app:app \
  --bind "0.0.0.0:$PORT" \
  --workers 1 \
  --worker-class gthread \
  --threads 4 \
  --timeout 120 \
  --access-logfile - \
  --error-logfile -
