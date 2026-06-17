#!/bin/bash

# Tango Signaling Server - Update Script
# Updates the application with zero downtime

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

APP_DIR="/home/tango/tango-signaling-server-python"
SERVICE_NAME="tango-signaling"
BACKUP_DIR="/var/backups/tango-signaling"

echo -e "${YELLOW}=== Tango Signaling Server Update ===${NC}\n"

# Check if running as root
if [[ $EUID -ne 0 ]]; then
   echo -e "${RED}This script must be run as root${NC}"
   exit 1
fi

# Create backup directory
mkdir -p $BACKUP_DIR

# Backup current version
echo -e "${YELLOW}[1/5] Creating backup...${NC}"
BACKUP_FILE="$BACKUP_DIR/backup-$(date +%Y%m%d-%H%M%S).tar.gz"
tar -czf $BACKUP_FILE \
    --exclude='venv' \
    --exclude='__pycache__' \
    --exclude='.git' \
    -C $(dirname $APP_DIR) $(basename $APP_DIR)
echo -e "${GREEN}Backup created: $BACKUP_FILE${NC}"

# Pull latest code
echo -e "${YELLOW}[2/5] Pulling latest code...${NC}"
cd $APP_DIR
git pull origin main || {
    echo -e "${RED}Failed to pull latest code${NC}"
    exit 1
}
echo -e "${GREEN}Code updated${NC}"

# Update Python dependencies
echo -e "${YELLOW}[3/5] Updating dependencies...${NC}"
sudo -u tango bash << EOF
cd $APP_DIR
source venv/bin/activate
pip install --upgrade pip
pip install -r requirements.txt
EOF
echo -e "${GREEN}Dependencies updated${NC}"

# Restart service
echo -e "${YELLOW}[4/5] Restarting service...${NC}"
systemctl restart $SERVICE_NAME

# Wait for service to start
sleep 3

# Verify service is running
echo -e "${YELLOW}[5/5] Verifying service...${NC}"
if systemctl is-active --quiet $SERVICE_NAME; then
    echo -e "${GREEN}Service is running${NC}"
else
    echo -e "${RED}Service failed to start!${NC}"
    echo "Rolling back from backup: $BACKUP_FILE"
    
    # Rollback procedure
    cd /
    tar -xzf $BACKUP_FILE
    systemctl restart $SERVICE_NAME
    
    exit 1
fi

# Health check
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8000/ok" 2>/dev/null)
if [ "$RESPONSE" == "200" ]; then
    echo -e "${GREEN}Health check passed (HTTP $RESPONSE)${NC}"
else
    echo -e "${RED}Health check failed (HTTP $RESPONSE)${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}=== Update Complete ===${NC}"
echo ""
echo "Summary:"
echo "  Code branch: $(git rev-parse --abbrev-ref HEAD)"
echo "  Latest commit: $(git log -1 --oneline)"
echo "  Service status: $(systemctl is-active $SERVICE_NAME)"
echo "  Backup location: $BACKUP_FILE"
echo ""
echo "View logs:"
echo "  journalctl -u $SERVICE_NAME -f"
echo ""
