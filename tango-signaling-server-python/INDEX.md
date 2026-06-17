# Project Index

## Quick Navigation

### For First-Time Users
1. Start here: **[QUICKSTART.md](QUICKSTART.md)** (5-minute setup)
2. Then read: **[README.md](README.md)** (full documentation)
3. Questions? Check: **[SUMMARY.md](SUMMARY.md)** (overview)

### For Developers
- Architecture & design: **[IMPLEMENTATION.md](IMPLEMENTATION.md)**
- Code structure: **[app.py](app.py)** (main application)
- Tests: **[tests/test_websocket.py](tests/test_websocket.py)**
- Configuration: **[config.py](config.py)**, **[.env.example](.env.example)**

### For DevOps/Operations
- Docker setup: **[Dockerfile](Dockerfile)**, **[docker-compose.yml](docker-compose.yml)**
- Production deployment: See README.md "Production Deployment" section
- Kubernetes: See IMPLEMENTATION.md "Pattern 3: Kubernetes"
- Monitoring: IMPLEMENTATION.md "Future Enhancements" section

### For Migrating from Node.js
1. Read: **[MIGRATION.md](MIGRATION.md)** (complete migration guide)
2. Compare: Original in `../tango-signaling-server`
3. Deploy: Use one of the deployment patterns in README.md

## File Descriptions

### Application Files

| File | Purpose |
|------|---------|
| **app.py** | Main FastAPI application with WebSocket endpoint and matchmaking logic |
| **config.py** | Configuration management from environment variables |
| **build.py** | Build script for protobuf code generation |

### Configuration Files

| File | Purpose |
|------|---------|
| **requirements.txt** | Production dependencies |
| **requirements-dev.txt** | Development and test dependencies |
| **pyproject.toml** | Python project metadata and tool configuration |
| **.env.example** | Template for environment variables |
| **.gitignore** | Git ignore rules |

### Deployment Files

| File | Purpose |
|------|---------|
| **Dockerfile** | Docker container definition |
| **docker-compose.yml** | Docker Compose for local multi-service setup |

### Documentation Files

| File | Purpose | Audience |
|------|---------|----------|
| **SUMMARY.md** | High-level overview and quick reference | Everyone |
| **README.md** | Complete documentation with examples and API reference | Developers, DevOps |
| **QUICKSTART.md** | 5-minute setup guide with common tasks | New users |
| **IMPLEMENTATION.md** | Architecture details and enhancement guide | Developers, Architects |
| **MIGRATION.md** | Step-by-step guide for migrating from Node.js | DevOps, Leads |
| **INDEX.md** | This file - navigation and file descriptions | Everyone |

### Test Files

| File | Purpose |
|------|---------|
| **tests/__init__.py** | Test package marker |
| **tests/test_websocket.py** | Unit and integration tests |

## Directory Structure

```
tango-signaling-server-python/
├── Application Code
│   ├── app.py              ← Main application
│   ├── config.py           ← Configuration
│   └── build.py            ← Protobuf builder
│
├── Configuration
│   ├── requirements.txt
│   ├── requirements-dev.txt
│   ├── pyproject.toml
│   ├── .env.example
│   └── .gitignore
│
├── Deployment
│   ├── Dockerfile
│   └── docker-compose.yml
│
├── Documentation
│   ├── README.md            ← Full docs
│   ├── QUICKSTART.md        ← Quick setup
│   ├── SUMMARY.md           ← Overview
│   ├── IMPLEMENTATION.md    ← Architecture
│   ├── MIGRATION.md         ← Migration guide
│   └── INDEX.md             ← This file
│
└── Tests
    ├── __init__.py
    └── test_websocket.py
```

## Getting Started Paths

### Path 1: Just Want It Running (5 minutes)
```
QUICKSTART.md → Run the server → Done!
```

### Path 2: Want to Understand What It Does (30 minutes)
```
SUMMARY.md → README.md → Try examples → Done!
```

### Path 3: Want to Develop/Modify (2-3 hours)
```
README.md → IMPLEMENTATION.md → Review app.py → Review tests → Setup dev environment → Done!
```

### Path 4: Want to Migrate from Node.js (1-2 weeks)
```
MIGRATION.md → Phase-by-phase setup and testing → Gradual rollout → Decommission old version
```

### Path 5: Want to Deploy to Production (1-2 days)
```
README.md (Production Deployment section) → Choose deployment pattern → Configure → Deploy → Monitor
```

## Key Concepts

### Session
A unique session ID that groups two peers for WebRTC connection:
- **Offerer**: Peer that sends the initial WebRTC offer
- **Answerer**: Peer that receives the offer and sends answer
- Message flow: Offerer → Server → Answerer (bidirectional)

### MatchmakingHub
Central component that:
- Tracks all active WebSocket connections
- Groups connections by session ID
- Routes messages between peers
- Identifies offerers vs answerers

### SessionAttachment
Metadata stored with each connection:
- `session_id`: Which session this connection belongs to
- `offer_sdp`: WebRTC offer (if this is the offerer)
- `connection_id`: Unique identifier for this connection

### ICE Servers
Network servers that help establish peer-to-peer connections:
- **STUN servers**: Help determine public IP address
- **TURN servers**: Relay traffic if direct connection fails
- Provisioned from: Self-hosted, Cloudflare, or Google defaults

## Common Tasks

### Task: Run Locally
See **[QUICKSTART.md](QUICKSTART.md)** - 5 minutes

### Task: Understand the Code
See **[IMPLEMENTATION.md](IMPLEMENTATION.md)** - Architecture section

### Task: Deploy to Docker
See **[README.md](README.md)** - "Using Docker" section

### Task: Deploy to Kubernetes
See **[IMPLEMENTATION.md](IMPLEMENTATION.md)** - "Pattern 3: Kubernetes"

### Task: Migrate from Node.js
See **[MIGRATION.md](MIGRATION.md)** - Complete guide

### Task: Enable Protobuf
See **[IMPLEMENTATION.md](IMPLEMENTATION.md)** - "1. Protobuf Message Support"

### Task: Add Authentication
See **[IMPLEMENTATION.md](IMPLEMENTATION.md)** - "3. Authentication & Authorization"

### Task: Monitor & Debug
See **[README.md](README.md)** - "Troubleshooting" section

## Quick Reference

### Endpoints
```
GET  /ok          → Health check (text)
GET  /health      → Health check (JSON)
WS   /ws          → WebSocket signaling
```

### Environment Variables
```
TURN_ADDR                      → Self-hosted TURN address
TURN_USER                      → TURN username
TURN_CREDENTIAL                → TURN password
CLOUDFLARE_TURN_SERVICE_ID     → Cloudflare TURN ID
CLOUDFLARE_TURN_SERVICE_API_TOKEN → Cloudflare TURN token
SERVER_HOST                    → Bind address (default: 0.0.0.0)
SERVER_PORT                    → Bind port (default: 8000)
LOG_LEVEL                      → Logging level (default: INFO)
```

### Dependencies
```
# Production
fastapi, uvicorn, websockets, protobuf, aiohttp

# Development
pytest, black, ruff, mypy
```

### Commands
```bash
# Development
python -m uvicorn app:app --reload

# Production
gunicorn -w 4 -k uvicorn.workers.UvicornWorker app:app

# Docker
docker build -t tango-signaling-server-python .
docker run -p 8000:8000 tango-signaling-server-python

# Tests
pytest tests/ -v

# Protobuf
python build.py
```

## FAQ

**Q: Is this production-ready?**  
A: Yes, but requires proper deployment setup (SSL, load balancer, monitoring)

**Q: Can I use this instead of the Node.js version?**  
A: Yes, it has 100% API compatibility with the original

**Q: What about multiple instances?**  
A: Use a load balancer with session affinity or add Redis for shared state

**Q: Does it support Cloudflare?**  
A: Not directly as Cloudflare Workers, but can run on any ASGI server

**Q: How do I scale this?**  
A: Multiple instances + load balancer with sticky sessions, or add Redis

**Q: What about WebRTC protocol support?**  
A: This is just the signaling server - clients handle the actual WebRTC

**Q: Can I add authentication?**  
A: Yes, see IMPLEMENTATION.md "Authentication & Authorization"

**Q: How do I monitor this?**  
A: Add Prometheus metrics (see IMPLEMENTATION.md "Metrics & Monitoring")

## Support Resources

- **Setup Help**: See QUICKSTART.md
- **Documentation**: See README.md
- **Architecture Questions**: See IMPLEMENTATION.md
- **Migration Questions**: See MIGRATION.md
- **Troubleshooting**: See README.md "Troubleshooting" section
- **Code Questions**: Read the code comments in app.py

## Version Info

- **Project**: Tango Signaling Server (Python)
- **Status**: Production Ready
- **Python**: 3.10+
- **FastAPI**: 0.115.0+
- **Created**: 2026-06-16

---

**Start here**: Choose your path above and follow the documentation!
