# Quick Start Guide

## 5-Minute Setup

### 1. Prerequisites

```bash
# Check Python version (3.10+ required)
python --version

# Install pip if needed
python -m pip install --upgrade pip
```

### 2. Create Virtual Environment

```bash
# On Windows
python -m venv venv
venv\Scripts\activate

# On macOS/Linux
python -m venv venv
source venv/bin/activate
```

### 3. Install Dependencies

```bash
pip install -r requirements.txt
```

### 4. Run the Server

```bash
# Development mode (with auto-reload)
python -m uvicorn app:app --reload --host 0.0.0.0 --port 8000

# Or directly
python app.py
```

### 5. Verify It's Running

```bash
# In another terminal
curl http://localhost:8000/ok
# Should return: ok
```

## Connecting a WebSocket Client

### Python Client Example

```python
import asyncio
import websockets
import json

async def connect():
    uri = "ws://localhost:8000/ws?session_id=my-session"
    async with websockets.connect(uri) as websocket:
        # Receive hello message
        hello = await websocket.recv()
        print("Server hello:", hello)

asyncio.run(connect())
```

### JavaScript Client Example

```javascript
const socket = new WebSocket('ws://localhost:8000/ws?session_id=my-session');

socket.onopen = function(event) {
    console.log('WebSocket connected');
};

socket.onmessage = function(event) {
    const message = JSON.parse(event.data);
    console.log('Received:', message);
};

socket.onerror = function(event) {
    console.error('WebSocket error:', event);
};
```

## Docker Quick Start

### Build and Run

```bash
# Build image
docker build -t tango-signaling-server-python .

# Run container
docker run -p 8000:8000 \
  -e TURN_ADDR=turn.example.com:3478 \
  -e TURN_USER=username \
  -e TURN_CREDENTIAL=password \
  tango-signaling-server-python
```

### Using Docker Compose

```bash
# Start services
docker-compose up -d

# View logs
docker-compose logs -f signaling-server

# Stop services
docker-compose down
```

## Common Tasks

### Configure TURN Server

Edit `.env` file:

```env
TURN_ADDR=turn.example.com:3478
TURN_USER=your_username
TURN_CREDENTIAL=your_password
```

Then restart the server.

### Enable Debug Logging

```bash
# Run with debug logging
DEBUG=true LOG_LEVEL=DEBUG python app.py
```

Or export:

```bash
export LOG_LEVEL=DEBUG
python app.py
```

### Run Tests

```bash
# Install test dependencies
pip install pytest pytest-asyncio httpx

# Run tests
pytest tests/

# Run with verbose output
pytest tests/ -v

# Run specific test
pytest tests/test_websocket.py::test_health_check -v
```

### Check Health

```bash
# HTTP health check
curl http://localhost:8000/ok

# JSON health check
curl http://localhost:8000/health | jq .

# WebSocket health (from Python)
python -c "
import asyncio
import websockets

async def test():
    try:
        async with websockets.connect('ws://localhost:8000/ws?session_id=test') as ws:
            msg = await ws.recv()
            print('Connected:', msg)
    except Exception as e:
        print('Error:', e)

asyncio.run(test())
"
```

## Production Deployment

### Using Gunicorn

```bash
# Install gunicorn if not already
pip install gunicorn

# Run with 4 workers
gunicorn -w 4 -k uvicorn.workers.UvicornWorker \
  --bind 0.0.0.0:8000 \
  app:app
```

### Behind Nginx

```nginx
upstream tango_signaling {
    server 127.0.0.1:8000;
}

server {
    listen 80;
    server_name signaling.example.com;

    location / {
        proxy_pass http://tango_signaling;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

## Troubleshooting

### "Module not found" Error

```bash
# Ensure virtual environment is activated
source venv/bin/activate  # macOS/Linux
# or
venv\Scripts\activate  # Windows

# Reinstall requirements
pip install -r requirements.txt
```

### "Address already in use"

Change the port:

```bash
python -m uvicorn app:app --port 8001
```

Or kill the process using the port:

```bash
# macOS/Linux
lsof -ti:8000 | xargs kill -9

# Windows
netstat -ano | findstr :8000
taskkill /PID <PID> /F
```

### WebSocket Connection Refused

1. Check server is running: `curl http://localhost:8000/ok`
2. Check firewall allows port 8000
3. Check proxy/load balancer WebSocket support
4. Look at server logs for errors

### High Memory Usage

Monitor with:

```bash
# Watch memory usage
watch -n 1 'ps aux | grep "app.py"'

# Or use psutil
python -c "
import psutil
import time

while True:
    for proc in psutil.process_iter(['pid', 'name', 'memory_percent']):
        if 'app.py' in proc.name():
            print(f'{proc.memory_percent():.2f}%')
    time.sleep(1)
"
```

## Next Steps

- Read [README.md](README.md) for full documentation
- Check [IMPLEMENTATION.md](IMPLEMENTATION.md) for architecture details
- Review [examples](examples/) for more use cases
- Check out the [tests](tests/) for usage patterns

## Support

For issues or questions:

1. Check the logs: Look at console output or `docker-compose logs`
2. Review configuration: Ensure environment variables are set correctly
3. Check the protocol: Review `../tango-signaling/src/proto/signaling.proto`
4. Enable debug logging: Set `LOG_LEVEL=DEBUG`
