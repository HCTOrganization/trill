# VPS Deployment with Nginx Reverse Proxy

Complete guide for deploying Tango Signaling Server on a VPS with Nginx as reverse proxy.

## Architecture Overview

```
Internet
    ↓
Nginx (Port 80/443)
    ↓ (Port 8000)
Gunicorn + Uvicorn Workers
    ↓
Tango Signaling Server
    ↓
WebSocket Connections (Browser Clients)
```

## Prerequisites

- VPS with Linux (Ubuntu 20.04+ recommended)
- SSH access to VPS
- Domain name (for SSL certificates)
- Sudo privileges

## Step 1: VPS Initial Setup

### 1.1 Update System

```bash
sudo apt update
sudo apt upgrade -y
```

### 1.2 Install Required Packages

```bash
# Python and system dependencies
sudo apt install -y python3.11 python3.11-venv python3.11-dev
sudo apt install -y nginx supervisor curl wget git
sudo apt install -y build-essential libssl-dev libffi-dev
sudo apt install -y certbot python3-certbot-nginx

# Optional but recommended
sudo apt install -y htop tmux vim
```

### 1.3 Create Application User

```bash
# Create a non-root user for the application
sudo useradd -m -s /bin/bash tango
sudo usermod -aG sudo tango

# Switch to the new user
sudo su - tango
```

## Step 2: Application Setup

### 2.1 Clone Repository

```bash
# As tango user
cd ~
git clone <repo-url> tango-signaling-server-python
cd tango-signaling-server-python
```

Or if uploading manually:

```bash
mkdir -p ~/tango-signaling-server-python
# Upload files via sftp or scp
cd ~/tango-signaling-server-python
```

### 2.2 Create Python Virtual Environment

```bash
# Create venv
python3.11 -m venv venv

# Activate venv
source venv/bin/activate

# Upgrade pip
pip install --upgrade pip setuptools wheel

# Install dependencies
pip install -r requirements.txt
```

### 2.3 Create .env File

```bash
cat > .env << 'EOF'
# Server Configuration
SERVER_HOST=127.0.0.1
SERVER_PORT=8000
LOG_LEVEL=INFO

# TURN Server (choose one option)

# Option 1: Self-hosted TURN server
# TURN_ADDR=turn.example.com:3478
# TURN_USER=username
# TURN_CREDENTIAL=password

# Option 2: Cloudflare TURN service
# CLOUDFLARE_TURN_SERVICE_ID=your-service-id
# CLOUDFLARE_TURN_SERVICE_API_TOKEN=your-token

# Option 3: Use Google STUN servers (default)
# No additional configuration needed
EOF
```

Edit `.env` with your configuration:

```bash
nano .env
```

### 2.4 Test Local Execution

```bash
# With venv activated
python -m uvicorn app:app --host 127.0.0.1 --port 8000

# Should output:
# INFO:     Uvicorn running on http://127.0.0.1:8000 (Press CTRL+C to quit)

# In another terminal, test:
curl http://localhost:8000/ok
# Should return: ok
```

Press `Ctrl+C` to stop.

## Step 3: Systemd Service Setup

### 3.1 Create Systemd Service File

```bash
sudo nano /etc/systemd/system/tango-signaling.service
```

Paste the following:

```ini
[Unit]
Description=Tango Signaling Server
After=network.target

[Service]
Type=notify
User=tango
Group=www-data
WorkingDirectory=/home/tango/tango-signaling-server-python
Environment="PATH=/home/tango/tango-signaling-server-python/venv/bin"
EnvironmentFile=/home/tango/tango-signaling-server-python/.env
ExecStart=/home/tango/tango-signaling-server-python/venv/bin/gunicorn \
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
```

### 3.2 Create Log Directory

```bash
sudo mkdir -p /var/log/tango-signaling
sudo chown tango:www-data /var/log/tango-signaling
sudo chmod 750 /var/log/tango-signaling
```

### 3.3 Enable and Start Service

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable on boot
sudo systemctl enable tango-signaling.service

# Start the service
sudo systemctl start tango-signaling.service

# Check status
sudo systemctl status tango-signaling.service

# View logs
sudo journalctl -u tango-signaling.service -f
```

Verify it's running:

```bash
curl http://localhost:8000/ok
# Should return: ok
```

## Step 4: Nginx Configuration

### 4.1 Create Nginx Configuration

```bash
sudo nano /etc/nginx/sites-available/tango-signaling
```

Paste the following (replace `signaling.example.com` with your domain):

```nginx
upstream tango_backend {
    least_conn;
    server 127.0.0.1:8000 max_fails=3 fail_timeout=30s;
    keepalive 32;
}

# Redirect HTTP to HTTPS
server {
    listen 80;
    listen [::]:80;
    server_name signaling.example.com;

    location /.well-known/acme-challenge/ {
        root /var/www/certbot;
    }

    location / {
        return 301 https://$server_name$request_uri;
    }
}

# Main HTTPS server
server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name signaling.example.com;

    # SSL Configuration (will be updated by certbot)
    ssl_certificate /etc/letsencrypt/live/signaling.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/signaling.example.com/privkey.pem;
    
    # Modern SSL configuration
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    ssl_prefer_server_ciphers on;
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 10m;

    # HSTS (only after confirming SSL works)
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;

    # Logging
    access_log /var/log/nginx/tango-signaling-access.log combined;
    error_log /var/log/nginx/tango-signaling-error.log warn;

    # Health check endpoint
    location /ok {
        proxy_pass http://tango_backend;
        proxy_http_version 1.1;
        access_log off;
    }

    # WebSocket endpoint
    location /ws {
        proxy_pass http://tango_backend;
        proxy_http_version 1.1;
        
        # WebSocket headers
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        
        # Proxy headers
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-Host $server_name;
        
        # Timeouts for WebSocket
        proxy_connect_timeout 7d;
        proxy_send_timeout 7d;
        proxy_read_timeout 7d;
        
        # Buffering
        proxy_buffering off;
        
        # Keep-alive
        proxy_set_header Connection "";
    }

    # API endpoints
    location / {
        proxy_pass http://tango_backend;
        proxy_http_version 1.1;
        
        # Proxy headers
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # Timeouts
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
    }

    # Deny direct access to sensitive files
    location ~ /\. {
        deny all;
        access_log off;
        log_not_found off;
    }
}
```

### 4.2 Enable Nginx Configuration

```bash
# Test configuration
sudo nginx -t

# Create symbolic link
sudo ln -s /etc/nginx/sites-available/tango-signaling /etc/nginx/sites-enabled/

# Disable default site (optional)
sudo rm -f /etc/nginx/sites-enabled/default

# Reload Nginx
sudo systemctl reload nginx
```

## Step 5: SSL Certificate Setup

### 5.1 Create Certbot Certificate

```bash
sudo certbot certonly --nginx -d signaling.example.com
# Follow prompts to create certificate
```

### 5.2 Auto-Renewal Setup

```bash
# Check certbot timer
sudo systemctl status certbot.timer

# Test renewal (dry-run)
sudo certbot renew --dry-run
```

## Step 6: Verification

### 6.1 Test Health Endpoint

```bash
# Via Nginx
curl https://signaling.example.com/ok
# Should return: ok

# Or from VPS
curl http://localhost:8000/ok
# Should return: ok
```

### 6.2 Test WebSocket Connection

```python
# Create test script: test_ws.py
import asyncio
import websockets
import json

async def test():
    uri = "wss://signaling.example.com/ws?session_id=test-123"
    async with websockets.connect(uri) as websocket:
        print("Connected!")
        msg = await websocket.recv()
        print("Received:", msg)

asyncio.run(test())
```

Run it:

```bash
pip install websockets
python test_ws.py
```

### 6.3 Check Service Status

```bash
# Application service
sudo systemctl status tango-signaling.service

# Nginx
sudo systemctl status nginx

# View recent logs
sudo journalctl -u tango-signaling.service -n 50

# View Nginx logs
tail -f /var/log/nginx/tango-signaling-access.log
tail -f /var/log/nginx/tango-signaling-error.log
```

## Step 7: Monitoring & Management

### 7.1 Monitor Service Health

```bash
# Create monitoring script: monitor.sh
#!/bin/bash

echo "=== Service Status ==="
sudo systemctl status tango-signaling.service

echo -e "\n=== Recent Errors ==="
sudo journalctl -u tango-signaling.service -n 10 --no-pager

echo -e "\n=== Nginx Status ==="
sudo systemctl status nginx

echo -e "\n=== Disk Usage ==="
df -h

echo -e "\n=== Memory Usage ==="
free -h

echo -e "\n=== Health Check ==="
curl -s http://localhost:8000/ok || echo "FAILED"
```

Make executable:

```bash
chmod +x monitor.sh
./monitor.sh
```

### 7.2 View Live Logs

```bash
# Application logs
sudo journalctl -u tango-signaling.service -f

# Nginx access
tail -f /var/log/nginx/tango-signaling-access.log

# Nginx errors
tail -f /var/log/nginx/tango-signaling-error.log

# All system logs
sudo journalctl -f
```

### 7.3 Restart Service

```bash
# Restart application
sudo systemctl restart tango-signaling.service

# Reload Nginx (no downtime)
sudo systemctl reload nginx

# Reload both
sudo systemctl restart tango-signaling.service && sudo systemctl reload nginx
```

## Step 8: Performance Tuning

### 8.1 Adjust Worker Count

Edit `/etc/systemd/system/tango-signaling.service`:

```ini
ExecStart=/home/tango/tango-signaling-server-python/venv/bin/gunicorn \
    -w 8 \  # Increase workers (typically 2-4 × CPU cores)
    -k uvicorn.workers.UvicornWorker \
    ...
```

Then restart:

```bash
sudo systemctl daemon-reload
sudo systemctl restart tango-signaling.service
```

### 8.2 Optimize Nginx

Add to `/etc/nginx/nginx.conf` in `http` block:

```nginx
# Connection pooling
upstream tango_backend {
    least_conn;
    server 127.0.0.1:8000 max_fails=3 fail_timeout=30s;
    keepalive 64;  # Increase from 32
}

# Gzip compression
gzip on;
gzip_types text/plain application/json application/javascript;
gzip_min_length 1000;

# File descriptors
worker_connections 2048;
```

### 8.3 Monitor Performance

```bash
# Check connection count
netstat -an | grep ESTABLISHED | wc -l

# Check process memory
ps aux | grep gunicorn

# Monitor in real-time
watch -n 1 'ps aux | grep gunicorn'

# Check Nginx connections
curl http://localhost:8000/health
```

## Step 9: Backup & Recovery

### 9.1 Backup Configuration

```bash
# Backup application
tar -czf tango-signaling-backup.tar.gz \
    ~/tango-signaling-server-python/.env \
    ~/tango-signaling-server-python/app.py \
    ~/tango-signaling-server-python/config.py

# Backup Nginx
sudo tar -czf nginx-config-backup.tar.gz /etc/nginx/

# Backup SSL certificates
sudo tar -czf ssl-backup.tar.gz /etc/letsencrypt/

# Move to safe location
sudo mv *.tar.gz /var/backups/
sudo chown root:root /var/backups/*.tar.gz
```

### 9.2 Recovery Procedure

```bash
# Restore application
cd ~
tar -xzf /var/backups/tango-signaling-backup.tar.gz

# Restore and test
sudo systemctl restart tango-signaling.service
curl http://localhost:8000/ok
```

## Step 10: Production Checklist

- [ ] Python 3.10+ installed
- [ ] Virtual environment created and dependencies installed
- [ ] `.env` file configured with TURN settings
- [ ] Systemd service created and enabled
- [ ] Service starts on boot (`systemctl status tango-signaling.service`)
- [ ] Nginx configured as reverse proxy
- [ ] SSL certificate installed and auto-renewal enabled
- [ ] Health check working (`curl https://signaling.example.com/ok`)
- [ ] WebSocket connection working
- [ ] Logs are being written and rotated
- [ ] Firewall allows ports 80, 443, and 22
- [ ] Performance monitoring in place
- [ ] Backup strategy implemented
- [ ] On-call documentation prepared

## Common Issues & Solutions

### Issue: "Permission denied" errors

```bash
# Fix permissions
sudo chown -R tango:tango /home/tango/tango-signaling-server-python
sudo chmod -R u+rwX /home/tango/tango-signaling-server-python
```

### Issue: Service won't start

```bash
# Check for errors
sudo systemctl status tango-signaling.service
sudo journalctl -u tango-signaling.service -n 50

# Try running manually
cd ~/tango-signaling-server-python
source venv/bin/activate
python app.py
```

### Issue: Nginx 502 Bad Gateway

```bash
# Check backend is running
curl http://localhost:8000/ok

# Check Nginx logs
tail -f /var/log/nginx/tango-signaling-error.log

# Restart both services
sudo systemctl restart tango-signaling.service
sudo systemctl reload nginx
```

### Issue: WebSocket connection fails

```bash
# Check Nginx WebSocket configuration
sudo nginx -T | grep -A 20 "location /ws"

# Test with curl
curl -i -N -H "Connection: Upgrade" \
  -H "Upgrade: websocket" \
  https://signaling.example.com/ws
```

### Issue: High memory usage

```bash
# Check process
ps aux | grep gunicorn

# Reduce worker count in service file
# Or increase swap
sudo fallocate -l 4G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
```

### Issue: SSL certificate renewal failing

```bash
# Manual renewal
sudo certbot renew --force-renewal

# Check logs
sudo journalctl -u certbot.service -f
```

## Security Hardening

### 7.1 Firewall Setup (UFW)

```bash
# Enable UFW
sudo ufw enable

# Allow SSH
sudo ufw allow 22/tcp

# Allow HTTP
sudo ufw allow 80/tcp

# Allow HTTPS
sudo ufw allow 443/tcp

# Check status
sudo ufw status
```

### 7.2 Fail2ban Setup (Optional)

```bash
# Install
sudo apt install -y fail2ban

# Create config
sudo cp /etc/fail2ban/jail.conf /etc/fail2ban/jail.local

# Configure and start
sudo systemctl enable fail2ban
sudo systemctl start fail2ban
```

### 7.3 SSH Hardening

```bash
# Disable password auth, use keys only
sudo nano /etc/ssh/sshd_config
# Set: PasswordAuthentication no
# Set: PubkeyAuthentication yes

# Restart SSH
sudo systemctl restart sshd
```

## Scaling for Higher Load

If you need to handle more connections:

### Option 1: Increase Worker Processes

```bash
# Edit service file
# Change: -w 4 → -w 8 or -w 16
# Depends on CPU cores (typically 2-4 × CPU count)
```

### Option 2: Use Load Balancer (Multiple VPS)

```nginx
upstream tango_backend {
    least_conn;
    server 192.168.1.100:8000 max_fails=3 fail_timeout=30s;
    server 192.168.1.101:8000 max_fails=3 fail_timeout=30s;
    server 192.168.1.102:8000 max_fails=3 fail_timeout=30s;
    keepalive 32;
}
```

Then use session affinity:

```nginx
hash $http_x_session_id consistent;
```

### Option 3: Add Redis for Distributed State

```bash
# Install Redis
sudo apt install -y redis-server

# Update app.py to use Redis hub
# (See IMPLEMENTATION.md for details)
```

## Troubleshooting & Debugging

### Enable Debug Mode

```bash
# Edit .env
echo "LOG_LEVEL=DEBUG" >> ~/.env

# Restart service
sudo systemctl restart tango-signaling.service

# View debug logs
sudo journalctl -u tango-signaling.service -f
```

### Test Backend Directly

```bash
# Connect to backend directly
curl -v http://localhost:8000/ok
curl -v http://localhost:8000/health
```

### Check Nginx Proxy Headers

```bash
# Test with verbose headers
curl -I -H "Upgrade: websocket" \
  -H "Connection: Upgrade" \
  https://signaling.example.com/ws?session_id=test
```

## Next Steps

1. **Monitor**: Set up monitoring alerts for service health
2. **Scale**: Add more workers or instances as needed
3. **Backup**: Automate configuration backups
4. **Upgrade**: Plan regular system and dependency updates
5. **Enhance**: Add authentication, metrics, or caching as needed

## Support Resources

- Nginx docs: https://nginx.org/en/docs/
- Gunicorn docs: https://docs.gunicorn.org/
- Certbot docs: https://certbot.eff.org/docs/
- FastAPI docs: https://fastapi.tiangolo.com/
