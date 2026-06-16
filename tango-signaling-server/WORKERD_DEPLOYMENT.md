# Deploying tango-signaling-server with workerd on VPS

This guide covers running the Cloudflare Workers signaling server locally on a VPS using `workerd`.

## Prerequisites

- **Node.js 18+** and npm
- **workerd** binary (Cloudflare's open-source Workers runtime)
- **nginx** or **caddy** for reverse proxy (optional, for HTTPS)
- **Linux VPS** with systemd support

## Installation

### 1. Install Node.js

```bash
# Using nvm (recommended)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
nvm install 20
nvm use 20

# Or using apt (Ubuntu/Debian)
sudo apt-get update
sudo apt-get install nodejs npm
```

### 2. Install workerd

```bash
# Option A: Download pre-built binary (easiest)
wget https://github.com/cloudflare/workerd/releases/download/v1.20241203.0/workerd-linux-64
chmod +x workerd-linux-64
sudo mv workerd-linux-64 /usr/local/bin/workerd

# Option B: Build from source (if needed)
git clone https://github.com/cloudflare/workerd.git
cd workerd
# Follow build instructions in their repo
```

Verify installation:
```bash
workerd --version
```

### 3. Clone and Setup

```bash
cd /opt
git clone https://github.com/your-repo/trill5.git
cd trill5/tango-signaling-server
npm install
```

### 4. Build Protocol Buffers

```bash
npm run proto
```

## Running Locally

### Development Mode

```bash
# Build and run with workerd
npm run proto
npm run typecheck
workerd --config workerd.toml --env development
```

Server will run on `http://localhost:8787`

### Production Mode

```bash
# Run production configuration
workerd --config workerd.toml --env production
```

Server will run on `http://0.0.0.0:8787`

## Systemd Service Setup

Create `/etc/systemd/system/tango-signaling-server.service`:

```ini
[Unit]
Description=Tango Signaling Server (workerd)
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=tango
WorkingDirectory=/opt/trill5/tango-signaling-server
ExecStart=/usr/local/bin/workerd --config workerd.toml --env production
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=tango-signaling

# Resource limits
LimitNOFILE=65536
LimitNPROC=65536

# Hardening
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
```

### Enable and Start Service

```bash
# Create dedicated user
sudo useradd -r -s /bin/false tango || true

# Set permissions
sudo chown -R tango:tango /opt/trill5/tango-signaling-server

# Enable service
sudo systemctl daemon-reload
sudo systemctl enable tango-signaling-server
sudo systemctl start tango-signaling-server

# Check status
sudo systemctl status tango-signaling-server

# View logs
sudo journalctl -u tango-signaling-server -f
```

## Reverse Proxy Setup with nginx

Since workerd listens on HTTP, use nginx for HTTPS and geolocation blocking:

Create `/etc/nginx/sites-available/matchmakingcn`:

```nginx
upstream workerd {
    server 127.0.0.1:8787;
    keepalive 64;
}

server {
    listen 80;
    listen [::]:80;
    server_name matchmakingcn.yourdomain.com;
    
    # Redirect to HTTPS
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name matchmakingcn.yourdomain.com;

    # SSL certificates (use Let's Encrypt)
    ssl_certificate /etc/letsencrypt/live/matchmakingcn.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/matchmakingcn.yourdomain.com/privkey.pem;
    
    # SSL configuration
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    ssl_prefer_server_ciphers on;

    location / {
        # Geolocation check - restrict to China mainland
        if ($http_cf_ipcountry != "CN") {
            return 403 "Access denied: Service available in China mainland only";
        }

        proxy_pass http://workerd;
        proxy_http_version 1.1;
        
        # WebSocket support
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        
        # Headers
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # Timeouts for long-lived connections
        proxy_read_timeout 86400s;
        proxy_send_timeout 86400s;
        proxy_connect_timeout 60s;
        
        # Buffering
        proxy_buffering off;
    }

    location /ok {
        proxy_pass http://workerd;
        access_log off;
    }
}
```

Enable nginx config:

```bash
sudo ln -s /etc/nginx/sites-available/matchmakingcn /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

### Setup Let's Encrypt SSL

```bash
sudo apt-get install certbot python3-certbot-nginx
sudo certbot certonly --nginx -d matchmakingcn.yourdomain.com

# Auto-renewal
sudo systemctl enable certbot.timer
```

## Environment Variables

Create `/opt/trill5/tango-signaling-server/.env`:

```bash
# TURN server credentials
TURN_ADDR=turn.yourdomain.com:3478
TURN_USER=username
TURN_CREDENTIAL=password

# Or use Cloudflare TURN service
CLOUDFLARE_TURN_SERVICE_ID=your_id
CLOUDFLARE_TURN_SERVICE_API_TOKEN=your_token
```

Load in systemd service by adding to `[Service]`:

```ini
EnvironmentFile=/opt/trill5/tango-signaling-server/.env
```

## Monitoring and Logs

### Check Health

```bash
curl http://localhost:8787/ok
```

### View Real-time Logs

```bash
# Systemd logs
sudo journalctl -u tango-signaling-server -f

# Or follow nginx logs
sudo tail -f /var/log/nginx/error.log
```

### Monitor Performance

```bash
# CPU/Memory usage
top -p $(pgrep -f workerd)

# Network connections
ss -tlnp | grep 8787
```

## Scaling

### Load Balancing with Multiple Instances

If you need multiple workerd instances:

```ini
upstream workerd {
    server 127.0.0.1:8787;
    server 127.0.0.1:8788;
    server 127.0.0.1:8789;
    keepalive 64;
}
```

Run multiple services with different ports:

```bash
# In systemd service override
ExecStart=/usr/local/bin/workerd --config workerd.toml --env production --port 8787
```

### Docker Deployment (Alternative)

See `Dockerfile.workerd` for containerized deployment.

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Port 8787 already in use | Change port in workerd.toml or kill existing process |
| WebSocket connection fails | Check `Upgrade` and `Connection` headers in nginx config |
| Geolocation not working | Cloudflare headers not available outside CF; modify Worker to accept custom headers |
| Service crashes | Check `journalctl -u tango-signaling-server -n 50` for errors |
| High memory usage | Durable Objects may accumulate state; implement cleanup logic |

## Updating

```bash
cd /opt/trill5/tango-signaling-server
git pull
npm install
npm run proto
sudo systemctl restart tango-signaling-server
```

## Backup and Recovery

```bash
# Backup durable object data (stored in workerd's storage)
sudo cp -r ~/.workerd /backup/workerd-backup-$(date +%s)

# Restore
sudo cp -r /backup/workerd-backup-* ~/.workerd
sudo systemctl restart tango-signaling-server
```

## References

- [workerd GitHub](https://github.com/cloudflare/workerd)
- [Cloudflare Workers Documentation](https://developers.cloudflare.com/workers/)
- [Durable Objects Guide](https://developers.cloudflare.com/workers/runtime-apis/durable-objects/)
