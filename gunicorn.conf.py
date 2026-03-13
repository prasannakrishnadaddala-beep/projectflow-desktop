import os

port = os.environ.get("PORT", os.environ.get("RAILWAY_PORT", "8080"))
bind = f"0.0.0.0:{port}"

workers = 1
worker_class = "gthread"
threads = 4
timeout = 120
keepalive = 5

accesslog = "-"
errorlog  = "-"
loglevel  = "info"

preload_app = True
proc_name   = "projectflow"
daemon      = False
