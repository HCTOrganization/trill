#!/bin/bash

# Deployment script for tango-signaling-server on VPS with workerd
# Usage: ./deploy-workerd.sh [dev|prod] [docker|native]

set -e

DEPLOY_ENV=${1:-prod}
DEPLOY_METHOD=${2:-native}
VPS_USER=${VPS_USER:-tango}
VPS_HOST=${VPS_HOST:-signaling.yourdomain.com}
INSTALL_PATH="/opt/trill5/tango-signaling-server"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    if [ "$DEPLOY_METHOD" = "native" ]; then
        command -v node >/dev/null 2>&1 || { log_error "Node.js not found. Please install Node.js 18+"; exit 1; }
        command -v npm >/dev/null 2>&1 || { log_error "npm not found"; exit 1; }
        command -v workerd >/dev/null 2>&1 || { log_error "workerd not found. Install from: https://github.com/cloudflare/workerd"; exit 1; }
    fi
    
    if [ "$DEPLOY_METHOD" = "docker" ]; then
        command -v docker >/dev/null 2>&1 || { log_error "Docker not found"; exit 1; }
        command -v docker-compose >/dev/null 2>&1 || { log_error "Docker Compose not found"; exit 1; }
    fi
    
    log_info "Prerequisites check passed"
}

# Deploy locally on VPS (native)
deploy_native() {
    log_info "Deploying with native workerd..."
    
    # Install dependencies
    log_info "Installing dependencies..."
    npm ci
    
    # Build protocol buffers
    log_info "Building protocol buffers..."
    npm run proto
    
    # Type checking
    log_info "Type checking..."
    npm run typecheck || true
    
    # Setup systemd service
    log_info "Setting up systemd service..."
    
    # Create service file
    sudo tee /etc/systemd/system/tango-signaling-server.service > /dev/null <<EOF
[Unit]
Description=Tango Signaling Server (workerd)
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=$VPS_USER
WorkingDirectory=$INSTALL_PATH
ExecStart=/usr/local/bin/workerd --config workerd.toml --env $DEPLOY_ENV
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=tango-signaling
EnvironmentFile=-$INSTALL_PATH/.env

LimitNOFILE=65536
LimitNPROC=65536
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF

    # Reload systemd
    sudo systemctl daemon-reload
    sudo systemctl enable tango-signaling-server
    
    log_info "Starting service..."
    sudo systemctl restart tango-signaling-server
    
    sleep 2
    
    if sudo systemctl is-active --quiet tango-signaling-server; then
        log_info "Service started successfully"
    else
        log_error "Service failed to start"
        sudo journalctl -u tango-signaling-server -n 20
        exit 1
    fi
}

# Deploy with Docker
deploy_docker() {
    log_info "Deploying with Docker..."
    
    # Build images
    log_info "Building Docker images..."
    docker-compose -f docker-compose.workerd.yml build
    
    # Create SSL directory
    mkdir -p ssl
    
    log_info "Starting containers..."
    docker-compose -f docker-compose.workerd.yml up -d
    
    sleep 3
    
    # Check if workerd is running
    if docker-compose -f docker-compose.workerd.yml ps workerd | grep -q "Up"; then
        log_info "Containers started successfully"
    else
        log_error "Container failed to start"
        docker-compose -f docker-compose.workerd.yml logs
        exit 1
    fi
}

# Setup nginx reverse proxy
setup_nginx() {
    log_info "Setting up nginx reverse proxy..."
    
    if [ "$DEPLOY_METHOD" = "native" ]; then
        sudo cp nginx.conf /etc/nginx/sites-available/matchmakingcn
        sudo ln -sf /etc/nginx/sites-available/matchmakingcn /etc/nginx/sites-enabled/
        
        # Test nginx config
        sudo nginx -t || { log_error "nginx config test failed"; exit 1; }
        
        sudo systemctl restart nginx
        log_info "nginx configured and restarted"
    fi
}

# Setup Let's Encrypt SSL
setup_ssl() {
    log_info "Setting up Let's Encrypt SSL..."
    
    if command -v certbot >/dev/null 2>&1; then
        sudo certbot certonly --nginx -d "$VPS_HOST" --non-interactive --agree-tos -m admin@yourdomain.com || \
            log_warn "SSL setup failed or already exists"
        
        sudo systemctl enable certbot.timer
    else
        log_warn "certbot not found. Install with: sudo apt-get install certbot python3-certbot-nginx"
    fi
}

# Health check
health_check() {
    log_info "Running health check..."
    
    sleep 2
    
    local max_attempts=5
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if curl -s http://localhost:8787/ok > /dev/null; then
            log_info "Health check passed"
            return 0
        fi
        
        attempt=$((attempt + 1))
        log_warn "Health check attempt $attempt/$max_attempts failed, retrying..."
        sleep 2
    done
    
    log_error "Health check failed"
    return 1
}

# Show logs
show_logs() {
    if [ "$DEPLOY_METHOD" = "native" ]; then
        log_info "Recent logs:"
        sudo journalctl -u tango-signaling-server -n 20
    else
        log_info "Docker logs:"
        docker-compose -f docker-compose.workerd.yml logs --tail=20
    fi
}

# Main deployment
main() {
    log_info "Starting deployment for $DEPLOY_ENV environment..."
    log_info "Deploy method: $DEPLOY_METHOD"
    
    check_prerequisites
    
    if [ "$DEPLOY_METHOD" = "native" ]; then
        deploy_native
        setup_nginx
        setup_ssl
    elif [ "$DEPLOY_METHOD" = "docker" ]; then
        deploy_docker
    else
        log_error "Unknown deploy method: $DEPLOY_METHOD"
        exit 1
    fi
    
    health_check || exit 1
    
    show_logs
    
    log_info "Deployment completed successfully!"
    log_info "Service is running and ready to accept connections"
}

# Run main
main
