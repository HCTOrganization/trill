# Implementation Guide

## Overview

This document describes the Python implementation of the Tango Signaling Server and how it differs from the original Node.js/TypeScript version.

## Architecture Comparison

### Node.js (Original)
- **Runtime**: Cloudflare Workers (Durable Objects)
- **Language**: TypeScript
- **Framework**: Native Cloudflare Workers API
- **Persistence**: Durable Objects (built-in state management)
- **Deployment**: Cloudflare Workers

### Python (This Implementation)
- **Runtime**: ASGI (uvicorn, gunicorn)
- **Language**: Python 3.10+
- **Framework**: FastAPI
- **Persistence**: In-memory (can be extended to Redis/database)
- **Deployment**: Traditional server, Docker, serverless adapters

## Key Implementation Details

### Session Management

**Node.js Original:**
```typescript
// Durable Object singleton per namespace
const stub = env.MATCHMAKING.get(
  env.MATCHMAKING.idFromName(HUB_SINGLETON_NAME),
);
```

**Python Implementation:**
```python
# Global hub instance
hub = MatchmakingHub()

# For distributed deployments, replace with:
# hub = await Redis().get_hub(HUB_SINGLETON_NAME)
```

### WebSocket Handling

**Node.js:**
```typescript
const [client, server] = Object.values(new WebSocketPair());
this.ctx.acceptWebSocket(server, [wsTag(sessionId)]);
server.serializeAttachment({ sessionId } satisfies Attachment);
```

**Python:**
```python
await websocket.accept()
await hub.add_connection(session_id, websocket, attachment)
```

### Message Processing

**Node.js (Protobuf binary):**
```typescript
packet = Packet.decode(new Uint8Array(message));
switch (packet.which) {
  case "start": this.handleStart(...); break;
  case "answer": this.handleAnswer(...); break;
  case "ping": ws.send(Packet.encode(...)); break;
}
```

**Python (Currently JSON, proto-ready):**
```python
# Binary protobuf support ready when proto files are available
data = await websocket.receive_bytes()
# TODO: packet = Packet().ParseFromString(data)
```

## Future Enhancements

### 1. Protobuf Message Support

When protobuf definitions are available:

```bash
pip install grpcio-tools
python build.py  # Generates Python proto modules
```

Then update `app.py`:

```python
from tango.signaling_pb2 import Packet, Start, Answer, Hello

async def handle_message(websocket, attachment, hub, message):
    packet = Packet()
    packet.ParseFromString(message)
    
    if packet.HasField("start"):
        await handle_start(websocket, attachment, hub, packet.start)
    elif packet.HasField("answer"):
        await handle_answer(websocket, attachment, hub, packet.answer)
```

### 2. Distributed Session State

Replace in-memory `MatchmakingHub` with Redis-backed version:

```python
class RedisMatchmakingHub:
    def __init__(self, redis_url: str):
        self.redis = redis.from_url(redis_url)
    
    async def find_offerer(self, session_id: str):
        # Retrieve from Redis
        offerer_data = await self.redis.get(f"offerer:{session_id}")
        return deserialize_offerer(offerer_data)
```

### 3. Authentication & Authorization

Add JWT validation:

```python
from fastapi import HTTPException, Depends
from fastapi.security import HTTPBearer

security = HTTPBearer()

async def verify_token(credentials = Depends(security)):
    token = credentials.credentials
    try:
        payload = jwt.decode(token, SECRET_KEY, algorithms=["HS256"])
        return payload
    except jwt.InvalidTokenError:
        raise HTTPException(status_code=401)

@app.websocket("/ws")
async def websocket_endpoint(websocket, session_id, token=Depends(verify_token)):
    # Token verified, proceed
    pass
```

### 4. Metrics & Monitoring

Add Prometheus metrics:

```python
from prometheus_client import Counter, Gauge, Histogram

connections_total = Counter("tango_connections_total", "Total connections")
active_connections = Gauge("tango_active_connections", "Active connections")
message_latency = Histogram("tango_message_latency_seconds", "Message latency")

@app.websocket("/ws")
async def websocket_endpoint(websocket, session_id):
    connections_total.inc()
    active_connections.inc()
    try:
        # ... handle connection
    finally:
        active_connections.dec()
```

### 5. Logging & Tracing

Enable structured logging and trace support:

```python
import structlog

log = structlog.get_logger()

@app.websocket("/ws")
async def websocket_endpoint(websocket, session_id):
    log.info("connection_started", session_id=session_id, 
             client=websocket.client)
```

## Deployment Patterns

### Pattern 1: Single Instance

```bash
# Development
python app.py

# Production
gunicorn -w 4 -k uvicorn.workers.UvicornWorker app:app
```

### Pattern 2: Docker Compose

```yaml
version: '3.8'
services:
  signaling:
    build: .
    ports:
      - "8000:8000"
    environment:
      TURN_ADDR: turn.example.com:3478
```

### Pattern 3: Kubernetes

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: tango-signaling-server
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
      - name: signaling-server
        image: tango-signaling-server-python:latest
        ports:
        - containerPort: 8000
        env:
        - name: TURN_ADDR
          value: "turn.example.com:3478"
        livenessProbe:
          httpGet:
            path: /ok
            port: 8000
          initialDelaySeconds: 10
          periodSeconds: 10
```

**Note:** Requires session affinity or Redis for distributed sessions:

```yaml
kind: Service
metadata:
  name: tango-signaling-server
spec:
  sessionAffinity: ClientIP
  sessionAffinityConfig:
    clientIP:
      timeoutSeconds: 10800
```

### Pattern 4: Serverless (AWS Lambda)

Use Mangum adapter:

```python
# lambda_handler.py
from mangum import Mangum
from app import app

handler = Mangum(app, lifespan="off")
```

Deploy with:

```bash
pip install -r requirements.txt -t package/
cd package && zip -r ../deployment.zip . && cd ..
aws lambda create-function --function-name tango-signaling \
  --runtime python3.11 --role arn:aws:iam::...:role/... \
  --handler lambda_handler.handler --zip-file fileb://deployment.zip
```

## Testing

### Unit Tests

```bash
pip install -r requirements-dev.txt
pytest tests/
```

### Integration Tests

```bash
# Start server
python app.py &

# Run integration tests
pytest tests/integration/

# Test WebSocket connection
python tests/integration/test_websocket.py
```

### Load Testing

```bash
pip install locust

# Create locustfile.py with WebSocket load test
locust -f locustfile.py --headless -u 1000 -r 100 -t 60s
```

## Performance Tuning

### Async Connection Pooling

```python
async with aiohttp.ClientSession() as session:
    # Connection pooling automatic
    await session.get(url)
```

### WebSocket Buffer Tuning

```python
from starlette.websockets import WebSocketState

# Adjust in FastAPI/Starlette configuration
app = FastAPI()
```

### Memory Optimization

Monitor with:

```bash
pip install memory-profiler
python -m memory_profiler app.py
```

## Troubleshooting

### Issue: WebSocket Disconnections

**Symptom:** Clients disconnecting randomly

**Solution:**
1. Increase keep-alive timeout
2. Add ping/pong messages
3. Check firewall/proxy WebSocket support

### Issue: Memory Leaks

**Symptom:** Growing memory usage over time

**Solution:**
1. Check that connections are properly cleaned up in finally blocks
2. Verify no circular references
3. Use `gc.collect()` if needed

### Issue: High CPU Usage

**Symptom:** 100% CPU with few connections

**Solution:**
1. Reduce worker count (adjust gunicorn `-w` parameter)
2. Check for busy-wait loops
3. Profile with `cProfile`

## Migration Path

To migrate from the Node.js version:

1. **Update clients** to use new WebSocket URL format
2. **Copy environment variables** for TURN server configuration
3. **Deploy alongside** Node.js version for gradual migration
4. **Monitor** both servers in parallel
5. **Redirect traffic** to Python version
6. **Deprecate** Node.js version
