# workerd Deployment Quick Reference

Deploy your signaling server on a VPS with Cloudflare's open-source Workers runtime.

## Quick Start (5 minutes)

### Option 1: Native Deployment (Recommended for VPS)

On your VPS:

```bash
# 1. SSH into VPS
ssh user@your-vps-ip

# 2. Install prerequisites
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs
sudo apt-get install -y nginx certbot python3-certbot-nginx

# 3. Install workerd
wget https://github.com/cloudflare/workerd/releases/download/v1.20241203.0/workerd-linux-64 -O workerd
chmod +x workerd
sudo mv workerd /usr/local/bin/

# 4. Clone and setup
cd /opt
git clone https://github.com/your-repo/trill5.git
cd trill5/tango-signaling-server
npm install

# 5. Deploy
./deploy-workerd.sh prod native
```

Done! Your server is now running on `https://matchmakingcn.yourdomain.com`

### Option 2: Docker Deployment

```bash
# 1. Install Docker and Docker Compose
curl -fsSL https://get.docker.com -o get-docker.sh | sudo sh

# 2. Deploy
./deploy-workerd.sh prod docker
```

## Available Commands

```bash
# View logs (native)
sudo journalctl -u tango-signaling-server -f

# View logs (Docker)
docker-compose -f docker-compose.workerd.yml logs -f

# Restart service (native)
sudo systemctl restart tango-signaling-server

# Restart service (Docker)
docker-compose -f docker-compose.workerd.yml restart workerd

# Health check
curl https://matchmakingcn.yourdomain.com/ok

# Stop (Docker)
docker-compose -f docker-compose.workerd.yml down
```

## Configuration

### Set Environment Variables

Create `.env` file:

```bash
# Self-hosted TURN
TURN_ADDR=turn.yourdomain.com:3478
TURN_USER=username
TURN_CREDENTIAL=password

# Or Cloudflare TURN
CLOUDFLARE_TURN_SERVICE_ID=xxx
CLOUDFLARE_TURN_SERVICE_API_TOKEN=xxx
```

### Update workerd.toml

```toml
[env.production.server]
ip = "0.0.0.0"
port = 8787
https = false  # Use nginx for HTTPS
```

### Update nginx.conf

Change:
```
server_name matchmakingcn.yourdomain.com;
```

## Monitoring

```bash
# Check service status
sudo systemctl status tango-signaling-server

# Monitor connections
ss -tlnp | grep 8787

# Monitor CPU/Memory
top -p $(pgrep -f workerd)

# Check recent errors
sudo journalctl -u tango-signaling-server --since "1 hour ago" | grep ERROR
```

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Port 8787 in use | `sudo lsof -i :8787` then kill process or change port |
| WebSocket fails | Check nginx `Upgrade` header config |
| Geolocation not working | workerd doesn't have geo headers; modify Worker code |
| High memory | Durable Object state accumulation; restart service |
| Service won't start | `sudo journalctl -u tango-signaling-server -n 50` |

## Performance

- **Connections**: 1000+ concurrent WebSockets per instance
- **Latency**: <10ms p95 (on same region)
- **CPU**: ~5-10% for moderate load
- **Memory**: 100-200MB baseline

## Updating

```bash
cd /opt/trill5/tango-signaling-server
git pull
npm install
npm run proto
sudo systemctl restart tango-signaling-server
```

## More Info

- [Full Deployment Guide](./WORKERD_DEPLOYMENT.md)
- [workerd GitHub](https://github.com/cloudflare/workerd)
- [Cloudflare Workers Docs](https://developers.cloudflare.com/workers/)
