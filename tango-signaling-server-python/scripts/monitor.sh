#!/bin/bash

# Tango Signaling Server - Monitoring Script
# Provides real-time monitoring of service health and performance

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
SERVICE_NAME="tango-signaling"
LOG_DIR="/var/log/tango-signaling"
BACKEND_URL="http://localhost:8000"

# Functions
print_header() {
    echo -e "\n${BLUE}=== $1 ===${NC}\n"
}

print_status() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}✓${NC} $2"
    else
        echo -e "${RED}✗${NC} $2"
    fi
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

# Clear screen
clear

# Header
echo -e "${BLUE}"
echo "╔═══════════════════════════════════════════════════════╗"
echo "║   Tango Signaling Server - Monitoring Dashboard       ║"
echo "║   Time: $(date '+%Y-%m-%d %H:%M:%S')                    ║"
echo "╚═══════════════════════════════════════════════════════╝"
echo -e "${NC}"

# System Info
print_header "System Information"
echo "Hostname: $(hostname)"
echo "Uptime: $(uptime -p)"
echo "Load Average: $(cat /proc/loadavg | awk '{print $1, $2, $3}')"

# Service Status
print_header "Service Status"
if systemctl is-active --quiet $SERVICE_NAME; then
    print_status 0 "Tango Signaling Server is RUNNING"
    PID=$(systemctl show -p MainPID --value $SERVICE_NAME)
    echo "  PID: $PID"
    echo "  Started: $(systemctl show -p ActiveEnterTimestamp --value $SERVICE_NAME)"
else
    print_status 1 "Tango Signaling Server is STOPPED"
    echo "  To start: sudo systemctl start $SERVICE_NAME"
fi

# Nginx Status
print_header "Nginx Status"
if systemctl is-active --quiet nginx; then
    print_status 0 "Nginx is RUNNING"
else
    print_status 1 "Nginx is STOPPED"
    echo "  To start: sudo systemctl start nginx"
fi

# Health Checks
print_header "Health Checks"

# Backend health
if curl -s "$BACKEND_URL/ok" | grep -q "ok"; then
    print_status 0 "Backend HTTP endpoint responding"
else
    print_status 1 "Backend HTTP endpoint not responding"
fi

# Backend JSON health
if curl -s "$BACKEND_URL/health" | grep -q "status"; then
    print_status 0 "Backend JSON endpoint responding"
else
    print_status 1 "Backend JSON endpoint not responding"
fi

# Resource Usage
print_header "Resource Usage"

# CPU Usage
CPU_USAGE=$(ps aux | grep "gunicorn" | grep -v grep | awk '{sum+=$3} END {print sum}')
echo "Gunicorn CPU: ${CPU_USAGE}%"

# Memory Usage
MEM_USAGE=$(ps aux | grep "gunicorn" | grep -v grep | awk '{sum+=$6} END {print sum/1024 "MB"}')
echo "Gunicorn Memory: $MEM_USAGE"

# Disk Usage
DISK_USAGE=$(df -h / | tail -1 | awk '{print $5 " (" $3 "/" $2 ")"}')
echo "Disk Usage: $DISK_USAGE"

# Disk Usage of app directory
if [ -d "/home/tango/tango-signaling-server-python" ]; then
    APP_DISK=$(du -sh /home/tango/tango-signaling-server-python | awk '{print $1}')
    echo "App Directory: $APP_DISK"
fi

# Memory Available
MEM_FREE=$(free -h | grep "^Mem" | awk '{print $7}')
echo "Memory Available: $MEM_FREE"

# Network Connections
print_header "Network Connections"

# Established connections
EST_COUNT=$(netstat -an 2>/dev/null | grep ESTABLISHED | wc -l)
echo "Established Connections: $EST_COUNT"

# WebSocket connections (port 8000)
WS_COUNT=$(netstat -an 2>/dev/null | grep ':8000' | grep ESTABLISHED | wc -l)
echo "Backend Connections: $WS_COUNT"

# Listening ports
echo -e "\n${YELLOW}Listening Ports:${NC}"
netstat -tuln 2>/dev/null | grep LISTEN | grep -E ':(80|443|8000)' | while read line; do
    echo "  $line"
done

# Recent Logs
print_header "Recent Application Logs (Last 5 Errors)"

if [ -f "$LOG_DIR/error.log" ]; then
    ERROR_COUNT=$(tail -50 "$LOG_DIR/error.log" | grep -c "ERROR\|error" || true)
    if [ $ERROR_COUNT -gt 0 ]; then
        print_warning "Found $ERROR_COUNT errors in recent logs"
        echo ""
        tail -5 "$LOG_DIR/error.log" | while read line; do
            echo "  $line"
        done
    else
        print_status 0 "No errors in recent logs"
    fi
else
    echo "Error log file not found: $LOG_DIR/error.log"
fi

# Systemd Logs
print_header "Recent Systemd Logs (Last 3 entries)"
journalctl -u $SERVICE_NAME -n 3 --no-pager 2>/dev/null || echo "No systemd logs available"

# Nginx Status
print_header "Nginx Status"
systemctl status nginx --no-pager 2>/dev/null | head -5

# Database Connections
print_header "Process Information"
echo "Gunicorn Processes:"
ps aux | grep "gunicorn" | grep -v grep | awk '{print "  PID " $2 " - Memory: " $6/1024 "MB - CPU: " $3 "%"}'

# Configuration Status
print_header "Configuration Status"

if [ -f "/etc/nginx/sites-enabled/tango-signaling" ]; then
    print_status 0 "Nginx configuration exists"
else
    print_status 1 "Nginx configuration not found"
fi

if [ -f "/etc/systemd/system/tango-signaling.service" ]; then
    print_status 0 "Systemd service configuration exists"
else
    print_status 1 "Systemd service configuration not found"
fi

if [ -f "/home/tango/tango-signaling-server-python/.env" ]; then
    print_status 0 "Application .env file exists"
else
    print_status 1 "Application .env file not found"
fi

# SSL Certificate Status
print_header "SSL Certificate Status"
if certbot certificates 2>/dev/null | grep -q "tango-signaling"; then
    CERT_INFO=$(certbot certificates 2>/dev/null | grep -A 2 "tango-signaling" | grep "Expiry Date")
    if [ -n "$CERT_INFO" ]; then
        echo "$CERT_INFO"
        # Calculate days until expiry
        EXPIRY_DATE=$(echo "$CERT_INFO" | grep -oP '\d{4}-\d{2}-\d{2}')
        if [ -n "$EXPIRY_DATE" ]; then
            EXPIRY_EPOCH=$(date -d "$EXPIRY_DATE" +%s)
            NOW_EPOCH=$(date +%s)
            DAYS_LEFT=$(( ($EXPIRY_EPOCH - $NOW_EPOCH) / 86400 ))
            if [ $DAYS_LEFT -lt 30 ]; then
                print_warning "Certificate expires in $DAYS_LEFT days"
            else
                print_status 0 "Certificate valid for $DAYS_LEFT more days"
            fi
        fi
    fi
else
    echo "No SSL certificate found"
fi

# Footer
echo ""
echo -e "${BLUE}═══════════════════════════════════════════════════════${NC}"
echo -e "Last updated: $(date '+%Y-%m-%d %H:%M:%S')"
echo ""
echo "Quick Commands:"
echo "  View logs:        journalctl -u tango-signaling.service -f"
echo "  Restart service:  sudo systemctl restart tango-signaling.service"
echo "  Reload Nginx:     sudo systemctl reload nginx"
echo "  Test backend:     curl http://localhost:8000/ok"
echo "  View processes:   ps aux | grep gunicorn"
echo ""
