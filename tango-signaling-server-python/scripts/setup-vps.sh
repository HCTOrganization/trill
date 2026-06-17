#!/bin/bash

# Tango Signaling Server - VPS Setup Script
# This script automates the setup process on a fresh Ubuntu VPS

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
DOMAIN="${1:-signaling.example.com}"
APP_USER="tango"
APP_HOME="/home/$APP_USER"
APP_DIR="$APP_HOME/tango-signaling-server-python"

echo -e "${YELLOW}=== Tango Signaling Server VPS Setup ===${NC}"
echo "Domain: $DOMAIN"
echo "App user: $APP_USER"
echo "App dir: $APP_DIR"
echo ""

# Check if running as root
if [[ $EUID -ne 0 ]]; then
   echo -e "${RED}This script must be run as root${NC}"
   exit 1
fi

# Step 1: Update system
echo -e "${YELLOW}[1/8] Updating system...${NC}"
apt update
apt upgrade -y

# Step 2: Install dependencies
echo -e "${YELLOW}[2/8] Installing dependencies...${NC}"
apt install -y \
    python3.11 python3.11-venv python3.11-dev \
    nginx supervisor curl wget git \
    build-essential libssl-dev libffi-dev \
    certbot python3-certbot-nginx \
    htop tmux vim ufw

# Step 3: Create application user
echo -e "${YELLOW}[3/8] Creating application user...${NC}"
if id "$APP_USER" &>/dev/null; then
    echo -e "${YELLOW}User $APP_USER already exists${NC}"
else
    useradd -m -s /bin/bash $APP_USER
    usermod -aG sudo $APP_USER
    echo -e "${GREEN}Created user $APP_USER${NC}"
fi

# Step 4: Setup application directory
echo -e "${YELLOW}[4/8] Setting up application directory...${NC}"
if [ -d "$APP_DIR" ]; then
    echo -e "${YELLOW}Directory already exists, skipping clone${NC}"
else
    # If this script is in the repo, use the current directory
    if [ -f "./app.py" ]; then
        mkdir -p $APP_DIR
        cp -r ./* $APP_DIR/
        chown -R $APP_USER:$APP_USER $APP_DIR
    else
        echo -e "${RED}Please clone the repository first or copy files to $APP_DIR${NC}"
        exit 1
    fi
fi

# Step 5: Setup Python environment
echo -e "${YELLOW}[5/8] Setting up Python virtual environment...${NC}"
sudo -u $APP_USER bash << EOF
cd $APP_DIR
python3.11 -m venv venv
source venv/bin/activate
pip install --upgrade pip setuptools wheel
pip install -r requirements.txt
EOF

# Step 6: Create .env file if not exists
echo -e "${YELLOW}[6/8] Creating .env file...${NC}"
if [ -f "$APP_DIR/.env" ]; then
    echo -e "${YELLOW}.env already exists, skipping${NC}"
else
    cat > "$APP_DIR/.env" << 'ENVEOF'
# Server Configuration
SERVER_HOST=127.0.0.1
SERVER_PORT=8000
LOG_LEVEL=INFO

# TURN Server (configure as needed)
# TURN_ADDR=turn.example.com:3478
# TURN_USER=username
# TURN_CREDENTIAL=password

# Or use Cloudflare TURN:
# CLOUDFLARE_TURN_SERVICE_ID=your-service-id
# CLOUDFLARE_TURN_SERVICE_API_TOKEN=your-token
ENVEOF
    chown $APP_USER:$APP_USER "$APP_DIR/.env"
    chmod 600 "$APP_DIR/.env"
    echo -e "${GREEN}Created .env file (edit with your settings)${NC}"
fi

# Step 7: Create systemd service
echo -e "${YELLOW}[7/8] Creating systemd service...${NC}"
mkdir -p /var/log/tango-signaling
chown $APP_USER:www-data /var/log/tango-signaling
chmod 750 /var/log/tango-signaling

cat > /etc/systemd/system/tango-signaling.service << SERVICEEOF
[Unit]
Description=Tango Signaling Server
After=network.target

[Service]
Type=notify
User=$APP_USER
Group=www-data
WorkingDirectory=$APP_DIR
Environment="PATH=$APP_DIR/venv/bin"
EnvironmentFile=$APP_DIR/.env
ExecStart=$APP_DIR/venv/bin/gunicorn \
    -w 4 \
    -k uvicorn.workers.UvicornWorker \
    --bind 127.0.0.1:8000 \
    --timeout 120 \
    --access-logfile /var/log/tango-signaling/access.log \
    --error-logfile /var/log/tango-signaling/error.log \
    --log-level info \
    app:app

Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=tango-signaling

[Install]
WantedBy=multi-user.target
SERVICEEOF

systemctl daemon-reload
systemctl enable tango-signaling.service
echo -e "${GREEN}Created systemd service${NC}"

# Step 8: Configure Nginx
echo -e "${YELLOW}[8/8] Configuring Nginx...${NC}"
cat > /etc/nginx/sites-available/tango-signaling << NGINXEOF
upstream tango_backend {
    least_conn;
    server 127.0.0.1:8000 max_fails=3 fail_timeout=30s;
    keepalive 32;
}

server {
    listen 80;
    listen [::]:80;
    server_name $DOMAIN;

    location /.well-known/acme-challenge/ {
        root /var/www/certbot;
    }

    location / {
        return 301 https://\$server_name\$request_uri;
    }
}

server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name $DOMAIN;

    ssl_certificate /etc/letsencrypt/live/$DOMAIN/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/$DOMAIN/privkey.pem;
    
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    ssl_prefer_server_ciphers on;
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 10m;

    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;

    access_log /var/log/nginx/tango-signaling-access.log combined;
    error_log /var/log/nginx/tango-signaling-error.log warn;

    location /ok {
        proxy_pass http://tango_backend;
        access_log off;
    }

    location /ws {
        proxy_pass http://tango_backend;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
        proxy_connect_timeout 7d;
        proxy_send_timeout 7d;
        proxy_read_timeout 7d;
        proxy_buffering off;
    }

    location / {
        proxy_pass http://tango_backend;
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }

    location ~ /\. {
        deny all;
        access_log off;
        log_not_found off;
    }
}
NGINXEOF

rm -f /etc/nginx/sites-enabled/default
ln -sf /etc/nginx/sites-available/tango-signaling /etc/nginx/sites-enabled/
nginx -t
systemctl reload nginx
echo -e "${GREEN}Configured Nginx${NC}"

# Summary
echo ""
echo -e "${GREEN}=== Setup Complete ===${NC}"
echo ""
echo "Next steps:"
echo "1. Edit configuration:"
echo "   nano $APP_DIR/.env"
echo ""
echo "2. Start the application:"
echo "   systemctl start tango-signaling.service"
echo ""
echo "3. Get SSL certificate:"
echo "   certbot certonly --nginx -d $DOMAIN"
echo ""
echo "4. Verify service is running:"
echo "   systemctl status tango-signaling.service"
echo "   curl http://localhost:8000/ok"
echo ""
echo "5. Monitor:"
echo "   journalctl -u tango-signaling.service -f"
echo "   tail -f /var/log/nginx/tango-signaling-access.log"
echo ""
