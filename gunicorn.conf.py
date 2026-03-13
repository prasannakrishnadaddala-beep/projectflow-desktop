import os

# Railway injects PORT env var — must bind to it
port = os.environ.get("PORT", "5000")
bind = f"0.0.0.0:{port}"

# Single worker — SQLite doesn't handle concurrent writes across workers
workers = 1
worker_class = "sync"
threads = 4
timeout = 120
keepalive = 5

# Logging → stdout (Railway captures this)
accesslog = "-"
errorlog  = "-"
loglevel  = "info"
access_log_format = '%(h)s "%(r)s" %(s)s %(b)s %(D)sµs'

preload_app = True
proc_name   = "projectflow"
daemon      = False
