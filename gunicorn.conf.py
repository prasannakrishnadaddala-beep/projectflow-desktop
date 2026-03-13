# gunicorn.conf.py — Production server config for ProjectFlow
import multiprocessing
import os

# ── Binding ───────────────────────────────────────────────────────────────────
bind = "127.0.0.1:5000"          # Nginx will reverse-proxy to this
backlog = 64

# ── Workers ───────────────────────────────────────────────────────────────────
# For ProjectFlow (SQLite + SSE polling) 1 worker avoids DB lock contention
workers = 1
worker_class = "sync"
threads = 4
worker_connections = 100
timeout = 120
keepalive = 5

# ── Logging ───────────────────────────────────────────────────────────────────
accesslog = "-"          # stdout → journald picks it up
errorlog  = "-"
loglevel  = "info"
access_log_format = '%(h)s "%(r)s" %(s)s %(b)s %(D)sµs'

# ── Process ───────────────────────────────────────────────────────────────────
proc_name = "projectflow"
preload_app = True
daemon = False            # systemd manages the process

# ── Environment ───────────────────────────────────────────────────────────────
raw_env = [
    "FLASK_ENV=production",
]
