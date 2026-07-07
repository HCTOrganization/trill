# VPS Deployment Quick Reference

Quick commands and configurations for VPS deployment with Nginx.

## Initial Setup (Run as root)

```bash
# 1. Update system
apt update && apt upgrade -y

# 2. Install dependencies
apt install -y python3.11 python3.11-venv nginx certbot python3-certbot-nginx

# 3. Create app user
useradd -m -s /bin/bash trill
su - trill

# 4. Setup application (as trill user)
git clone <repo> trill-signaling-server-python
cd trill-signaling-server-python
python3.11 -m venv venv
source venv/bin/activate
pip install -r requirements.txt

# 5. Create .env
cat > .env << 'EOF'
SERVER_HOST=127.0.0.1
SERVER_PORT=8000
LOG_LEVEL=INFO
EOF
```

## Systemd Service (Run as root)

```bash
sudo tee /etc/systemd/system/trill-signaling.service > /dev/null << 'EOF'
[Unit]
Description=Trill Signaling Server
After=network.target

[Service]
Type=notify
User=trill
Group=www-data
WorkingDirectory=/home/trill/trill-signaling-server-python
Environment="PATH=/home/trill/trill-signaling-server-python/venv/bin"
EnvironmentFile=/home/trill/trill-signaling-server-python/.env
ExecStart=/home/trill/trill-signaling-server-python/venv/bin/gunicorn \
    -w 4 -k uvicorn.workers.UvicornWorker \
    --bind 127.0.0.1:8000 \
    --timeout 120 \
    --access-logfile /var/log/trill-signaling/access.log \
    --error-logfile /var/log/trill-signaling/error.log \
    app:app

Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

sudo mkdir -p /var/log/trill-signaling
sudo chown trill:www-data /var/log/trill-signaling
sudo systemctl daemon-reload
sudo systemctl enable trill-signaling.service
sudo systemctl start trill-signaling.service
```

## Nginx Configuration (Run as root)

```bash
sudo tee /etc/nginx/sites-available/trill-signaling > /dev/null << 'EOF'
upstream trill_backend {
    least_conn;
    server 127.0.0.1:8000 max_fails=3 fail_timeout=30s;
    keepalive 32;
}

server {
    listen 80;
    server_name signaling.example.com;
    location / { return 301 https://$server_name$request_uri; }
    location /.well-known/acme-challenge/ { root /var/www/certbot; }
}

server {
    listen 443 ssl http2;
    server_name signaling.example.com;
    ssl_certificate /etc/letsencrypt/live/signaling.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/signaling.example.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    add_header Strict-Transport-Security "max-age=31536000" always;

    access_log /var/log/nginx/trill-access.log;
    error_log /var/log/nginx/trill-error.log;

    location /ws {
        proxy_pass http://trill_backend;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_connect_timeout 7d;
        proxy_send_timeout 7d;
        proxy_read_timeout 7d;
        proxy_buffering off;
    }

    location / {
        proxy_pass http://trill_backend;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
EOF

sudo ln -sf /etc/nginx/sites-available/trill-signaling /etc/nginx/sites-enabled/
sudo rm -f /etc/nginx/sites-enabled/default
sudo nginx -t
sudo systemctl reload nginx
```

## SSL Certificate (Run as root)

```bash
# Create certificate
sudo certbot certonly --nginx -d signaling.example.com

# Auto-renewal is automatic with systemctl timer
sudo systemctl status certbot.timer
```

## Verification

```bash
# Health check
curl https://signaling.example.com/ok

# Service status
sudo systemctl status trill-signaling.service

# View logs
sudo journalctl -u trill-signaling.service -f

# Monitor
bash /home/trill/trill-signaling-server-python/scripts/monitor.sh
```

## Common Operations

### Restart Service
```bash
sudo systemctl restart trill-signaling.service
```

### View Logs
```bash
# Application logs
sudo journalctl -u trill-signaling.service -f

# Nginx logs
tail -f /var/log/nginx/trill-access.log
tail -f /var/log/nginx/trill-error.log
```

### Update Application
```bash
cd /home/trill/trill-signaling-server-python
sudo git pull
sudo systemctl restart trill-signaling.service
```

### Change Configuration
```bash
# Edit environment
nano /home/trill/trill-signaling-server-python/.env

# Restart to apply changes
sudo systemctl restart trill-signaling.service
```

### Monitor Performance
```bash
# Real-time monitoring
watch -n 1 'ps aux | grep gunicorn'

# Check connections
netstat -an | grep ':8000' | wc -l

# Check memory
ps aux | grep gunicorn | awk '{sum+=$6} END {print sum/1024 "MB"}'
```

## Troubleshooting

### 502 Bad Gateway
```bash
# Check if backend is running
curl http://localhost:8000/ok

# Check service status
sudo systemctl status trill-signaling.service

# View errors
sudo journalctl -u trill-signaling.service -n 20
```

### High Memory Usage
```bash
# Check what's using memory
ps aux | grep gunicorn | sort -k6 -nr

# Restart service
sudo systemctl restart trill-signaling.service
```

### Port Already in Use
```bash
# Find process using port 8000
lsof -i :8000

# Kill it if needed
kill -9 <PID>
```

### Nginx Configuration Error
```bash
# Test configuration
sudo nginx -t

# View detailed errors
sudo nginx -T | grep -A 20 "location /ws"
```

## Scaling Tips

### Increase Workers
Edit `/etc/systemd/system/trill-signaling.service`:
```bash
# Change: -w 4
# To: -w 8  (or 2-4 × CPU cores)
```

### Enable UFW Firewall
```bash
sudo ufw allow 22/tcp
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw enable
```

### Setup Monitoring
```bash
# Run monitoring script
bash /home/trill/trill-signaling-server-python/scripts/monitor.sh

# Or health check
bash /home/trill/trill-signaling-server-python/scripts/health-check.sh signaling.example.com
```

## Files & Locations

```
Application:    /home/trill/trill-signaling-server-python
Configuration:  /home/trill/trill-signaling-server-python/.env
Systemd:        /etc/systemd/system/trill-signaling.service
Nginx:          /etc/nginx/sites-available/trill-signaling
SSL Certs:      /etc/letsencrypt/live/signaling.example.com/
App Logs:       /var/log/trill-signaling/
Nginx Logs:     /var/log/nginx/trill-*.log
```

## Security Checklist

- [ ] SSH key-only authentication (no passwords)
- [ ] UFW firewall enabled
- [ ] SSL/TLS certificate installed
- [ ] HSTS header enabled
- [ ] Application .env not world-readable
- [ ] Log files not world-readable
- [ ] Regular backups configured
- [ ] Security updates automatic (unattended-upgrades)

## Monitoring Setup (Cron)

```bash
# Add to crontab: crontab -e
# Health check every 5 minutes
*/5 * * * * /home/trill/trill-signaling-server-python/scripts/health-check.sh signaling.example.com > /tmp/trill-health.log 2>&1

# Daily monitoring report
0 9 * * * /home/trill/trill-signaling-server-python/scripts/monitor.sh > /tmp/trill-monitor.log 2>&1

# Check for updates weekly
0 2 * * 0 cd /home/trill/trill-signaling-server-python && git fetch && echo "Updates available" || true
```

---

**For complete guide**: See [VPS_DEPLOYMENT.md](VPS_DEPLOYMENT.md)
