# Tango Signaling Server (Python)

A Python implementation of the Tango signaling server for WebRTC peer-to-peer communication coordination.

This server handles the signaling protocol for establishing WebRTC connections, managing session matchmaking, and ICE server provisioning.

## Features

- **WebSocket-based signaling** for WebRTC offer/answer exchange
- **Session matchmaking** - pairs offerers and answerers
- **ICE server provisioning** (Cloudflare TURN or self-hosted fallback)
- **Geolocation-aware access control** (China mainland optional)
- **Async/await** for high-performance I/O
- **Protocol buffer** message support (when proto files are available)

## Architecture

### Key Components

- **MatchmakingHub**: Manages WebSocket connections and pairs peers
  - Tracks sessions with multiple connections
  - Identifies offerers (peers with offer SDP) vs answerers
  - Routes messages between peers
  
- **SessionAttachment**: Stores per-connection metadata
  - `session_id`: Session identifier
  - `offer_sdp`: WebRTC offer (for offerers)
  - `connection_id`: Unique connection identifier

### Message Flow

1. **Connection**: Client connects with `session_id` query parameter
2. **Hello**: Server sends ICE server list
3. **Start**: Client sends initial offer (becomes offerer) or receives offer
4. **Answer**: Answering peer responds with answer
5. **Close**: Connection closed after successful exchange

## Requirements

- Python 3.10+
- FastAPI
- websockets
- protobuf
- aiohttp

## Installation

```bash
# Clone or navigate to the project
cd tango-signaling-server-python

# Create virtual environment
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install dependencies
pip install -r requirements.txt
```

## Build

Generate protobuf types (optional, when proto files are available):

```bash
python build.py
```

## Development

```bash
# Run with auto-reload
python -m uvicorn app:app --reload --host 0.0.0.0 --port 8000

# Or directly
python app.py
```

Visit `http://localhost:8000/ok` to verify the server is running.

## Production Deployment

### VPS with Nginx Reverse Proxy (Recommended)

For a complete step-by-step guide on deploying to a VPS with Nginx and SSL:

**→ See [VPS_DEPLOYMENT.md](VPS_DEPLOYMENT.md)**

This includes:
- System setup and Python environment
- Systemd service configuration
- Nginx reverse proxy setup
- SSL/TLS with Let's Encrypt
- Health monitoring and maintenance
- Performance tuning and scaling
- Automated deployment scripts

Quick start:

```bash
# Download and run automated setup
curl -O https://raw.githubusercontent.com/.../scripts/setup-vps.sh
sudo bash setup-vps.sh signaling.example.com
```

### Using Gunicorn

```bash
gunicorn -w 4 -k uvicorn.workers.UvicornWorker \
  --bind 0.0.0.0:8000 \
  --timeout 120 \
  --access-logfile - \
  app:app
```

### Using Docker

```bash
# Build
docker build -t tango-signaling-server-python .

# Run
docker run -p 8000:8000 \
  -e TURN_ADDR=turn.example.com:3478 \
  -e TURN_USER=username \
  -e TURN_CREDENTIAL=password \
  tango-signaling-server-python
```

### Using Docker Compose

```bash
docker-compose up -d
```

## Configuration

### Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `TURN_ADDR` | Self-hosted TURN server address (e.g., `turn.example.com:3478`) | No |
| `TURN_USER` | TURN server username | No (required if TURN_ADDR set) |
| `TURN_CREDENTIAL` | TURN server credential/password | No (required if TURN_ADDR set) |
| `CLOUDFLARE_TURN_SERVICE_ID` | Cloudflare TURN service ID | No |
| `CLOUDFLARE_TURN_SERVICE_API_TOKEN` | Cloudflare TURN service API token | No |
| `SERVER_HOST` | Server bind address (default: `0.0.0.0`) | No |
| `SERVER_PORT` | Server port (default: `8000`) | No |
| `LOG_LEVEL` | Logging level (default: `INFO`) | No |
| `DEBUG` | Enable debug mode (default: `false`) | No |

### Example `.env` file

```env
# Self-hosted TURN
TURN_ADDR=turn.example.com:3478
TURN_USER=username
TURN_CREDENTIAL=password

# Or Cloudflare TURN
CLOUDFLARE_TURN_SERVICE_ID=your-service-id
CLOUDFLARE_TURN_SERVICE_API_TOKEN=your-token

# Server config
SERVER_HOST=0.0.0.0
SERVER_PORT=8000
LOG_LEVEL=INFO
```

## API Endpoints

### `GET /ok`
Health check endpoint. Returns `ok`.

### `GET /health`
Health check endpoint (JSON). Returns `{"status": "ok"}`.

### `GET /`
Main endpoint. Returns `ok` or `not found` depending on path.

### `WebSocket /ws`
Main WebSocket endpoint for signaling.

**Query Parameters:**
- `session_id` (required): Unique session identifier

**Headers (optional):**
- `cf-ipcountry`: Cloudflare geolocation header for access control

**Connection Flow:**

1. Connect with `ws://server:8000/ws?session_id=YOUR_SESSION_ID`
2. Receive `hello` message with ICE servers
3. Send/receive signaling messages (binary format based on protobuf)

## Protocol

This server implements the Tango signaling protocol defined in `../tango-signaling/src/proto/signaling.proto`.

### Message Types

- **hello**: Server→Client, contains ICE server list
- **start**: Client→Server, contains WebRTC offer (offerer role)
- **offer**: Server→Client, contains WebRTC offer for answerer
- **answer**: Client→Server, contains WebRTC answer (answerer role)
- **answer**: Server→Client, contains WebRTC answer for offerer
- **ping**: Client→Server, keep-alive message
- **ping**: Server→Client, keep-alive response
- **abort**: Server→Client, connection error with reason

## Testing

### Basic Connection Test

```python
import asyncio
import websockets
import json

async def test():
    uri = "ws://localhost:8000/ws?session_id=test-session"
    async with websockets.connect(uri) as websocket:
        # Receive hello
        hello = await websocket.recv()
        print("Hello:", hello)
        
        # Send ping
        await websocket.send(b"ping")

asyncio.run(test())
```

## Performance

- **Async I/O**: Handles thousands of concurrent connections
- **Memory Efficient**: Uses asyncio for non-blocking I/O
- **Stateless**: Can be run behind a load balancer (with session affinity)

## Limitations

Currently, the server operates in a single-instance mode. For multi-instance deployments, you would need:
1. A shared session store (e.g., Redis)
2. Message broadcasting between instances
3. Session affinity in the load balancer

## Troubleshooting

### Connection Refused

```bash
# Check if server is running
curl http://localhost:8000/ok

# Check port is bound
netstat -tuln | grep 8000  # Linux
netstat -ano | findstr 8000  # Windows
```

### Geolocation Access Denied

The server checks the `cf-ipcountry` header for "CN" (China). If running behind Cloudflare, ensure the header is forwarded. You can disable geolocation checking by removing the check in `app.py`.

### ICE Server Errors

Check environment variables for TURN server configuration:

```bash
echo $CLOUDFLARE_TURN_SERVICE_ID
echo $TURN_ADDR
```

## License

Same as the main Tango project.

## Migration from Node.js Version

The Python version maintains API compatibility with the original Node.js implementation but uses FastAPI instead of Cloudflare Workers. Key differences:

- **Runtime**: Python/FastAPI instead of Node.js/Cloudflare Workers
- **Deployment**: Traditional servers, Docker, or serverless (with HTTP adapter)
- **Database**: No persistent state (can be added with Redis/PostgreSQL)
- **Scaling**: Requires explicit session affinity or shared state
