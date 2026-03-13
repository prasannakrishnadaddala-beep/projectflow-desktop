#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# ProjectFlow — Server Bootstrap Script
# Run this ONCE on your fresh Ubuntu/Debian VPS to prepare it for deployments.
#
# Usage:
#   curl -fsSL https://your-gitlab.com/project/-/raw/main/deploy/bootstrap.sh | sudo bash
#   — OR —
#   sudo bash deploy/bootstrap.sh
#
# What it does:
#   1. Installs Python 3, pip, nginx, certbot
#   2. Creates /opt/projectflow with correct permissions
#   3. Sets up a deploy user with sudo access for systemd/nginx only
#   4. Adds your GitLab deploy key to authorized_keys
#   5. Configures nginx with your domain
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

# ── Config — edit these ───────────────────────────────────────────────────────
DOMAIN="${DOMAIN:-projectflow.example.com}"
DEPLOY_USER="${DEPLOY_USER:-projectflow}"
DEPLOY_PATH="${DEPLOY_PATH:-/opt/projectflow}"
# ─────────────────────────────────────────────────────────────────────────────

RED='\033[0;31m'; GRN='\033[0;32m'; YLW='\033[1;33m'; NC='\033[0m'
log()  { echo -e "${GRN}[bootstrap]${NC} $*"; }
warn() { echo -e "${YLW}[bootstrap]${NC} $*"; }
err()  { echo -e "${RED}[bootstrap]${NC} $*" >&2; exit 1; }

[[ $EUID -ne 0 ]] && err "Run as root: sudo bash deploy/bootstrap.sh"
[[ "$DOMAIN" == "projectflow.example.com" ]] && warn "DOMAIN is not set — edit the script or set: DOMAIN=yourdomain.com"

log "━━━ ProjectFlow Server Bootstrap ━━━"
log "Domain      : $DOMAIN"
log "Deploy path : $DEPLOY_PATH"
log "Deploy user : $DEPLOY_USER"

# ── 1. System updates & packages ──────────────────────────────────────────────
log "Installing system packages..."
apt-get update -qq
apt-get install -y -qq \
  python3 python3-pip python3-venv \
  nginx certbot python3-certbot-nginx \
  git curl wget rsync \
  ufw fail2ban

# ── 2. Create deploy user ─────────────────────────────────────────────────────
log "Creating deploy user: $DEPLOY_USER"
id "$DEPLOY_USER" &>/dev/null || useradd -r -s /bin/bash -m -d /home/"$DEPLOY_USER" "$DEPLOY_USER"

# Allow deploy user to manage only the projectflow service + nginx (no full sudo)
cat > /etc/sudoers.d/projectflow << SUDOERS
$DEPLOY_USER ALL=(ALL) NOPASSWD: \
  /bin/systemctl daemon-reload, \
  /bin/systemctl enable projectflow, \
  /bin/systemctl disable projectflow, \
  /bin/systemctl start projectflow, \
  /bin/systemctl stop projectflow, \
  /bin/systemctl restart projectflow, \
  /bin/systemctl reload projectflow, \
  /bin/systemctl is-active projectflow, \
  /bin/journalctl -u projectflow, \
  /bin/cp $DEPLOY_PATH/projectflow.service /etc/systemd/system/projectflow.service, \
  /bin/sed -i * /etc/systemd/system/projectflow.service, \
  /bin/mkdir -p $DEPLOY_PATH, \
  /bin/tar -xzf /tmp/projectflow-*.tar.gz -C $DEPLOY_PATH *, \
  /bin/chown -R $DEPLOY_USER $DEPLOY_PATH, \
  /bin/tee $DEPLOY_PATH/.env
SUDOERS
chmod 440 /etc/sudoers.d/projectflow

# ── 3. Deploy directory ───────────────────────────────────────────────────────
log "Creating deploy directory..."
mkdir -p "$DEPLOY_PATH/data/pf_uploads"
chown -R "$DEPLOY_USER":"$DEPLOY_USER" "$DEPLOY_PATH"
chmod 755 "$DEPLOY_PATH"

# ── 4. SSH authorized_keys for CI runner ──────────────────────────────────────
log "Setting up SSH authorized_keys..."
SSH_DIR="/home/$DEPLOY_USER/.ssh"
mkdir -p "$SSH_DIR"
touch "$SSH_DIR/authorized_keys"
chmod 700 "$SSH_DIR"
chmod 600 "$SSH_DIR/authorized_keys"
chown -R "$DEPLOY_USER":"$DEPLOY_USER" "$SSH_DIR"
echo ""
warn "━━━ ACTION REQUIRED ━━━"
warn "Add your GitLab CI deploy public key to:"
warn "  $SSH_DIR/authorized_keys"
warn ""
warn "Generate a key pair with:"
warn "  ssh-keygen -t ed25519 -C 'gitlab-ci-projectflow' -f ~/.ssh/projectflow_deploy"
warn "Then:"
warn "  Add the PUBLIC key  (projectflow_deploy.pub) → $SSH_DIR/authorized_keys"
warn "  Add the PRIVATE key (projectflow_deploy)     → GitLab CI/CD Variable: SSH_PRIVATE_KEY"
warn "━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# ── 5. Nginx config ───────────────────────────────────────────────────────────
log "Installing nginx config..."
NGINX_CONF="/etc/nginx/sites-available/projectflow"
cp /dev/stdin "$NGINX_CONF" << NGINXEOF
server {
    listen 80;
    server_name $DOMAIN;
    return 301 https://\$host\$request_uri;
}
server {
    listen 443 ssl http2;
    server_name $DOMAIN;
    ssl_certificate     /etc/letsencrypt/live/$DOMAIN/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/$DOMAIN/privkey.pem;
    include             /etc/letsencrypt/options-ssl-nginx.conf;
    ssl_dhparam         /etc/letsencrypt/ssl-dhparams.pem;

    client_max_body_size 155M;
    access_log /var/log/nginx/projectflow.log;
    error_log  /var/log/nginx/projectflow-err.log;

    location / {
        proxy_pass         http://127.0.0.1:5000;
        proxy_http_version 1.1;
        proxy_set_header   Host              \$host;
        proxy_set_header   X-Real-IP         \$remote_addr;
        proxy_set_header   X-Forwarded-For   \$proxy_add_x_forwarded_for;
        proxy_set_header   X-Forwarded-Proto \$scheme;
        proxy_set_header   Upgrade           \$http_upgrade;
        proxy_set_header   Connection        "upgrade";
        proxy_buffering    off;
        proxy_read_timeout 120s;
    }
}
NGINXEOF

ln -sf "$NGINX_CONF" /etc/nginx/sites-enabled/projectflow
rm -f /etc/nginx/sites-enabled/default
nginx -t && systemctl reload nginx

# ── 6. SSL certificate ────────────────────────────────────────────────────────
if [[ "$DOMAIN" != "projectflow.example.com" ]]; then
  log "Obtaining SSL certificate for $DOMAIN..."
  certbot --nginx -d "$DOMAIN" --non-interactive --agree-tos -m "admin@$DOMAIN" || \
    warn "Certbot failed — run manually: sudo certbot --nginx -d $DOMAIN"
else
  warn "Skipping SSL (domain not configured)"
fi

# ── 7. Firewall ───────────────────────────────────────────────────────────────
log "Configuring firewall..."
ufw --force enable
ufw allow 22/tcp   # SSH
ufw allow 80/tcp   # HTTP
ufw allow 443/tcp  # HTTPS
ufw deny  5000/tcp # Block direct Flask access (nginx only)
ufw status

# ── Done ──────────────────────────────────────────────────────────────────────
log ""
log "━━━ Bootstrap complete! ━━━"
log "Next steps:"
log "  1. Add your deploy SSH public key to: $SSH_DIR/authorized_keys"
log "  2. Set GitLab CI/CD variables (Settings → CI/CD → Variables):"
log "     SSH_PRIVATE_KEY  = <private key content>"
log "     SERVER_HOST      = $(curl -s ifconfig.me 2>/dev/null || echo 'your-server-ip')"
log "     SERVER_USER      = $DEPLOY_USER"
log "     DEPLOY_PATH      = $DEPLOY_PATH"
log "     APP_URL          = https://$DOMAIN"
log "     SECRET_KEY       = $(python3 -c 'import secrets; print(secrets.token_hex(32))')"
log "  3. Push to main/master to trigger your first deploy"
log ""
