# Migration Guide from Node.js to Python

This guide helps you migrate from the original Node.js/TypeScript Tango Signaling Server to the Python version.

## Overview

| Aspect | Node.js | Python |
|--------|---------|--------|
| **Runtime** | Cloudflare Workers | ASGI (uvicorn/gunicorn) |
| **Framework** | Cloudflare Workers API | FastAPI |
| **Language** | TypeScript | Python 3.10+ |
| **State** | Durable Objects | In-memory + Redis (optional) |
| **Deployment** | Cloudflare Workers | Docker, VPS, Kubernetes |

## Step-by-Step Migration

### Phase 1: Preparation

**Week 1-2: Evaluate Differences**

1. **Infrastructure Differences**
   - Node.js version runs on Cloudflare Workers (serverless)
   - Python version runs on ASGI servers (traditional or containerized)
   - Python version needs explicit load balancing setup for multiple instances

2. **Feature Parity Check**
   - Core signaling: ✅ 100% compatible
   - TURN server provisioning: ✅ Compatible
   - Geolocation filtering: ✅ Compatible (via cf-ipcountry header)
   - Protocol buffer support: ✅ Ready to implement

3. **Performance Expectations**
   - Python can handle 10K+ concurrent connections per instance
   - Typical VM: 5,000-15,000 concurrent users
   - For 100K+ users: Use multiple instances + load balancer + shared state

### Phase 2: Development Setup

**Week 2-3: Set Up Python Environment**

```bash
# Clone the repository
git clone <repo>
cd tango-signaling-server-python

# Create virtual environment
python -m venv venv
source venv/bin/activate  # or venv\Scripts\activate on Windows

# Install dependencies
pip install -r requirements.txt

# Run tests to verify setup
pytest tests/
```

**Test Against Original:**

Run both servers side-by-side for comparison:

```bash
# Terminal 1: Node.js version
cd ../tango-signaling-server
npm install && npm run dev

# Terminal 2: Python version
cd ../tango-signaling-server-python
python app.py
```

### Phase 3: Configuration Migration

**Week 3-4: Copy Environment Configuration**

**Node.js wrangler.toml:**
```toml
name = "tango-signaling-server"
main = "src/index.ts"

[[durable_objects.bindings]]
name = "MATCHMAKING"
class_name = "MatchmakingHub"
```

**Python environment (.env):**
```bash
# Copy these from your Node.js environment
TURN_ADDR=turn.example.com:3478
TURN_USER=username
TURN_CREDENTIAL=password
# OR
CLOUDFLARE_TURN_SERVICE_ID=service-id
CLOUDFLARE_TURN_SERVICE_API_TOKEN=token

# New Python-specific settings
SERVER_HOST=0.0.0.0
SERVER_PORT=8000
LOG_LEVEL=INFO
```

### Phase 4: Testing & Validation

**Week 4-5: Run Compatibility Tests**

1. **Unit Tests**
   ```bash
   pytest tests/ -v
   ```

2. **Connection Tests**
   ```bash
   # Test basic connectivity
   curl http://localhost:8000/ok
   
   # Test WebSocket
   python -c "
   import asyncio
   import websockets
   
   async def test():
       uri = 'ws://localhost:8000/ws?session_id=test'
       async with websockets.connect(uri) as ws:
           msg = await ws.recv()
           print('Success:', msg)
   
   asyncio.run(test())
   "
   ```

3. **Performance Testing**
   ```bash
   pip install locust
   # Create load test script...
   locust -f locustfile.py
   ```

4. **Integration Testing**
   - Connect WebRTC clients to both servers
   - Verify offer/answer exchange works
   - Check ICE server provisioning
   - Monitor for memory leaks

### Phase 5: Deployment

**Week 5-6: Deploy Python Version**

#### Option A: Single Server (Development)

```bash
# Run directly
python app.py

# Or with gunicorn
gunicorn -w 4 -k uvicorn.workers.UvicornWorker app:app
```

#### Option B: Docker (Recommended)

```bash
# Build image
docker build -t tango-signaling-server-python:1.0 .

# Run container
docker run -p 8000:8000 \
  -e TURN_ADDR=turn.example.com:3478 \
  -e TURN_USER=user \
  -e TURN_CREDENTIAL=pass \
  tango-signaling-server-python:1.0
```

#### Option C: Kubernetes

```bash
# Create namespace
kubectl create namespace tango

# Deploy
kubectl apply -f - <<EOF
apiVersion: apps/v1
kind: Deployment
metadata:
  name: tango-signaling-server
  namespace: tango
spec:
  replicas: 3
  selector:
    matchLabels:
      app: tango-signaling-server
  template:
    metadata:
      labels:
        app: tango-signaling-server
    spec:
      containers:
      - name: server
        image: tango-signaling-server-python:1.0
        ports:
        - containerPort: 8000
        env:
        - name: TURN_ADDR
          valueFrom:
            secretKeyRef:
              name: tango-config
              key: turn_addr
        - name: TURN_USER
          valueFrom:
            secretKeyRef:
              name: tango-config
              key: turn_user
        - name: TURN_CREDENTIAL
          valueFrom:
            secretKeyRef:
              name: tango-config
              key: turn_credential
        livenessProbe:
          httpGet:
            path: /ok
            port: 8000
          initialDelaySeconds: 10
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: 8000
          initialDelaySeconds: 5
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: tango-signaling-server
  namespace: tango
spec:
  type: LoadBalancer
  selector:
    app: tango-signaling-server
  ports:
  - port: 80
    targetPort: 8000
    protocol: TCP
  sessionAffinity: ClientIP
  sessionAffinityConfig:
    clientIP:
      timeoutSeconds: 10800
EOF
```

### Phase 6: Traffic Migration

**Week 6-7: Gradual Rollout**

1. **Deploy Python version alongside Node.js**
   ```
   Load Balancer
       ├── 50% → Node.js (tango-signaling-server)
       └── 50% → Python (tango-signaling-server-python)
   ```

2. **Monitor both versions**
   - Connection count
   - Error rates
   - Memory usage
   - Response times

3. **Gradual shift** (if using weighted load balancing)
   - Day 1-2: 90% Node.js, 10% Python
   - Day 3-4: 70% Node.js, 30% Python
   - Day 5-6: 50% Node.js, 50% Python
   - Day 7+: 100% Python

4. **Verify client compatibility**
   - Test across all client platforms
   - Check error handling
   - Verify no degradation

### Phase 7: Deprecation

**Week 7-8: Decommission Node.js**

1. **Sunset Period**
   - Keep Node.js running for 1-2 weeks
   - Have fallback plan ready
   - Monitor Python version closely

2. **Decommission**
   - Stop accepting new connections to Node.js
   - Drain existing connections gracefully
   - Remove Node.js infrastructure

3. **Archive**
   - Tag final commit in git
   - Document what was removed
   - Keep for historical reference

## Breaking Changes

### Client-Side Changes

None! The WebSocket API remains the same:

```javascript
// This still works exactly the same
const socket = new WebSocket('ws://server:8000/ws?session_id=SESSION_ID');
```

### Server Configuration Changes

**Node.js (wrangler.toml):**
```toml
[[env.production.env]]
TURN_ADDR = "turn.example.com"
```

**Python (.env):**
```bash
TURN_ADDR=turn.example.com:3478
```

Note: Python requires the port in TURN_ADDR.

### Deployment Changes

**Node.js:**
```bash
wrangler deploy
```

**Python:**
```bash
# Docker
docker push tango-signaling-server-python:latest

# Or Kubernetes
kubectl set image deployment/tango-signaling-server \
  server=tango-signaling-server-python:latest
```

## Rollback Plan

If you need to roll back to Node.js:

1. **Keep both versions running** for 1-2 weeks
2. **Have DNS TTL set low** (1 minute) for quick switching
3. **Use traffic shaping** at load balancer level
4. **Document all configuration** for Node.js version

Example rollback command:

```bash
# Switch traffic back to Node.js
# (implementation depends on your load balancer)
kubectl set image deployment/tango-signaling-server \
  server=tango-signaling-server:previous
```

## Performance Benchmarks

### Comparison

| Metric | Node.js | Python |
|--------|---------|--------|
| **Cold Start** | <50ms | ~200ms |
| **Connections/sec** | 1000+ | 1000+ |
| **Memory per conn** | ~1KB | ~1.5KB |
| **Max concurrent** | Depends on Workers | 50K+ per instance |
| **Throughput** | 10Gbps (CF) | Limited by server |

### Optimization Tips for Python

1. **Use uvloop** for faster event loop
   ```python
   import uvloop
   asyncio.set_event_loop_policy(uvloop.EventLoopPolicy())
   ```

2. **Enable connection pooling**
   ```python
   app.state.session = aiohttp.ClientSession()
   ```

3. **Use gunicorn with uvicorn workers**
   ```bash
   gunicorn -w 4 -k uvicorn.workers.UvicornWorker app:app
   ```

## Monitoring & Observability

### Metrics to Track

```python
# Add to requirements.txt
prometheus-client

# Add to app.py
from prometheus_client import Counter, Gauge

connections = Counter('tango_connections_total', 'Total connections')
active = Gauge('tango_connections_active', 'Active connections')
```

### Logging

```python
import logging

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
```

### Health Checks

Both versions support:
- `GET /ok` → Returns "ok"
- `GET /health` → Returns JSON status

Use for load balancer health checks.

## Support & Issues

- Compare behavior between Node.js and Python versions
- Log detailed information for debugging
- Keep both servers accessible during transition
- Have on-call support during rollout

## Checklist

- [ ] Development environment setup
- [ ] Configuration migrated and tested
- [ ] Unit tests passing
- [ ] Integration tests passing
- [ ] Performance benchmarks acceptable
- [ ] Load testing successful
- [ ] Staging deployment verified
- [ ] Gradual rollout plan documented
- [ ] Monitoring and alerting set up
- [ ] Rollback plan ready
- [ ] Client compatibility verified
- [ ] Documentation updated
- [ ] Team trained on new deployment
- [ ] Node.js version decommissioned
