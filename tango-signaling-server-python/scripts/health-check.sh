#!/bin/bash

# Tango Signaling Server - Health Check Script
# Simple health check script for monitoring systems (Nagios, Zabbix, etc.)

# Usage: ./health-check.sh [domain]
# Exit codes:
#   0 = OK (all checks pass)
#   1 = WARNING (some checks failed)
#   2 = CRITICAL (service down)

set -e

DOMAIN="${1:-localhost}"
BACKEND_URL="http://localhost:8000"
HTTPS_URL="https://$DOMAIN"
SERVICE_NAME="tango-signaling"
THRESHOLD_RESPONSE_TIME=1000  # milliseconds

# Colors (for terminal output)
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# Counters
CHECKS_TOTAL=0
CHECKS_PASSED=0
CHECKS_FAILED=0

# Helper function
check_status() {
    CHECKS_TOTAL=$((CHECKS_TOTAL + 1))
    if [ $1 -eq 0 ]; then
        CHECKS_PASSED=$((CHECKS_PASSED + 1))
        echo -e "${GREEN}✓${NC} $2"
        return 0
    else
        CHECKS_FAILED=$((CHECKS_FAILED + 1))
        echo -e "${RED}✗${NC} $2"
        return 1
    fi
}

warn() {
    echo -e "${YELLOW}⚠${NC} $1"
}

# Check if running as root (for service checks)
if [[ $EUID -eq 0 ]]; then
    # Check systemd service
    if systemctl is-active --quiet $SERVICE_NAME; then
        check_status 0 "Systemd service is active"
    else
        check_status 1 "Systemd service is NOT active"
    fi

    if systemctl is-active --quiet nginx; then
        check_status 0 "Nginx is active"
    else
        check_status 1 "Nginx is NOT active"
    fi
else
    warn "Not running as root - skipping service checks"
fi

# Check backend HTTP endpoint
echo -e "\n${GREEN}Backend Checks:${NC}"

# Test /ok endpoint
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" "$BACKEND_URL/ok" 2>/dev/null)
if [ "$RESPONSE" == "200" ]; then
    check_status 0 "Backend /ok endpoint returns 200"
else
    check_status 1 "Backend /ok endpoint returns $RESPONSE (expected 200)"
fi

# Test /health endpoint
RESPONSE=$(curl -s -w "\n%{http_code}" "$BACKEND_URL/health" 2>/dev/null | tail -1)
if [ "$RESPONSE" == "200" ]; then
    check_status 0 "Backend /health endpoint returns 200"
else
    check_status 1 "Backend /health endpoint returns $RESPONSE (expected 200)"
fi

# Test response time
if command -v curl &> /dev/null; then
    RESPONSE_TIME=$(curl -s -o /dev/null -w "%{time_total}" "$BACKEND_URL/ok" 2>/dev/null | awk '{print $1 * 1000}')
    if (( $(echo "$RESPONSE_TIME < $THRESHOLD_RESPONSE_TIME" | bc -l) )); then
        check_status 0 "Backend response time: ${RESPONSE_TIME}ms (< ${THRESHOLD_RESPONSE_TIME}ms)"
    else
        check_status 1 "Backend response time: ${RESPONSE_TIME}ms (> ${THRESHOLD_RESPONSE_TIME}ms)"
    fi
fi

# Check HTTPS endpoint (if domain provided)
if [ "$DOMAIN" != "localhost" ]; then
    echo -e "\n${GREEN}HTTPS Checks:${NC}"
    
    RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" -I "$HTTPS_URL/ok" 2>/dev/null || echo "000")
    if [ "$RESPONSE" == "200" ]; then
        check_status 0 "HTTPS /ok endpoint returns 200"
    else
        check_status 1 "HTTPS /ok endpoint returns $RESPONSE (expected 200)"
    fi
    
    # Check SSL certificate
    if command -v openssl &> /dev/null; then
        CERT_EXPIRY=$(echo | openssl s_client -servername $DOMAIN -connect $DOMAIN:443 2>/dev/null | openssl x509 -noout -dates 2>/dev/null | grep notAfter | cut -d= -f2)
        if [ -n "$CERT_EXPIRY" ]; then
            EXPIRY_EPOCH=$(date -d "$CERT_EXPIRY" +%s 2>/dev/null || echo "0")
            NOW_EPOCH=$(date +%s)
            DAYS_LEFT=$(( ($EXPIRY_EPOCH - $NOW_EPOCH) / 86400 ))
            
            if [ $DAYS_LEFT -gt 30 ]; then
                check_status 0 "SSL certificate valid for $DAYS_LEFT days"
            elif [ $DAYS_LEFT -gt 0 ]; then
                check_status 1 "SSL certificate expires in $DAYS_LEFT days"
            else
                check_status 1 "SSL certificate has EXPIRED"
            fi
        fi
    fi
fi

# Check disk space
echo -e "\n${GREEN}System Checks:${NC}"

DISK_USAGE=$(df / | tail -1 | awk '{print $5}' | sed 's/%//')
if [ "$DISK_USAGE" -lt 80 ]; then
    check_status 0 "Disk usage: ${DISK_USAGE}% (< 80%)"
else
    check_status 1 "Disk usage: ${DISK_USAGE}% (>= 80%)"
fi

# Check memory
MEMORY_USAGE=$(free | grep Mem | awk '{printf("%d", $3/$2 * 100)}')
if [ "$MEMORY_USAGE" -lt 90 ]; then
    check_status 0 "Memory usage: ${MEMORY_USAGE}% (< 90%)"
else
    check_status 1 "Memory usage: ${MEMORY_USAGE}% (>= 90%)"
fi

# Check process count
if [ "$DOMAIN" = "localhost" ] || [ "$DOMAIN" = "" ]; then
    GUNICORN_PROCS=$(ps aux | grep "gunicorn" | grep -v grep | wc -l)
    if [ "$GUNICORN_PROCS" -gt 0 ]; then
        check_status 0 "Gunicorn processes running: $GUNICORN_PROCS"
    else
        check_status 1 "No Gunicorn processes running"
    fi
fi

# Summary
echo ""
echo -e "${GREEN}═══════════════════════════════════════════${NC}"
echo "Health Check Results:"
echo "  Total Checks: $CHECKS_TOTAL"
echo -e "  ${GREEN}Passed: $CHECKS_PASSED${NC}"
echo -e "  ${RED}Failed: $CHECKS_FAILED${NC}"
echo -e "${GREEN}═══════════════════════════════════════════${NC}"

# Return exit code
if [ $CHECKS_FAILED -eq 0 ]; then
    echo "Status: OK"
    exit 0
elif [ $CHECKS_FAILED -lt 3 ]; then
    echo "Status: WARNING"
    exit 1
else
    echo "Status: CRITICAL"
    exit 2
fi
