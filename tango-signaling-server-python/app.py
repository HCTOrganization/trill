"""
Tango Signaling Server - Python Implementation

A WebSocket-based signaling server for WebRTC peer-to-peer communication.
Handles session matchmaking and ICE server provisioning.
"""

import asyncio
import logging
import os
from typing import Optional, Dict, Set, Tuple
from contextlib import asynccontextmanager
from dataclasses import dataclass

import aiohttp
from fastapi import FastAPI, WebSocket, WebSocketDisconnect, Request, Response, Query
from fastapi.responses import PlainTextResponse

logger = logging.getLogger(__name__)
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)

# Constants
HUB_SINGLETON_NAME = "global"
DEFAULT_ICE_SERVERS = [
    {"urls": ["stun:stun.l.google.com:19302"], "username": None, "credential": None},
    {"urls": ["stun:stun1.l.google.com:19302"], "username": None, "credential": None},
    {"urls": ["stun:stun2.l.google.com:19302"], "username": None, "credential": None},
    {"urls": ["stun:stun3.l.google.com:19302"], "username": None, "credential": None},
    {"urls": ["stun:stun4.l.google.com:19302"], "username": None, "credential": None},
]


@dataclass
class SessionAttachment:
    """Metadata attached to a WebSocket connection."""
    session_id: str
    offer_sdp: Optional[str] = None
    connection_id: Optional[str] = None


class MatchmakingHub:
    """Manages WebSocket connections and pairs offerers with answerers."""
    
    def __init__(self):
        # session_id -> set of (ws, attachment)
        self.connections: Dict[str, Set[Tuple[WebSocket, SessionAttachment]]] = {}
        self.lock = asyncio.Lock()
    
    async def add_connection(self, session_id: str, ws: WebSocket, attachment: SessionAttachment):
        """Register a WebSocket connection."""
        async with self.lock:
            if session_id not in self.connections:
                self.connections[session_id] = set()
            self.connections[session_id].add((ws, attachment))
            logger.debug(f"Added connection for session {session_id}")
    
    async def remove_connection(self, session_id: str, ws: WebSocket):
        """Unregister a WebSocket connection."""
        async with self.lock:
            if session_id in self.connections:
                self.connections[session_id] = {
                    (w, a) for w, a in self.connections[session_id] 
                    if w != ws
                }
                if not self.connections[session_id]:
                    del self.connections[session_id]
                logger.debug(f"Removed connection for session {session_id}")
    
    def find_offerer(self, session_id: str) -> Optional[Tuple[WebSocket, SessionAttachment]]:
        """Find an offerer (peer with a stored offer SDP) in a session."""
        if session_id not in self.connections:
            return None
        
        for ws, attachment in self.connections[session_id]:
            if attachment.offer_sdp is not None:
                return (ws, attachment)
        
        return None
    
    async def update_connection_attachment(self, ws: WebSocket, session_id: str, attachment: SessionAttachment):
        """Update a WebSocket's attachment."""
        async with self.lock:
            if session_id in self.connections:
                # Remove old entry and add new one
                self.connections[session_id] = {
                    (w, attachment) if w == ws else (w, a)
                    for w, a in self.connections[session_id]
                }


async def get_ice_servers() -> list:
    """Fetch ICE servers from environment or Cloudflare."""
    # Check for self-hosted TURN server
    turn_addr = os.getenv("TURN_ADDR")
    turn_user = os.getenv("TURN_USER")
    turn_credential = os.getenv("TURN_CREDENTIAL")
    
    if turn_addr and turn_user and turn_credential:
        return [
            {
                "urls": [f"turn:{turn_addr}"],
                "username": turn_user,
                "credential": turn_credential,
            }
        ]
    
    # Try Cloudflare TURN service
    cf_service_id = os.getenv("CLOUDFLARE_TURN_SERVICE_ID")
    cf_api_token = os.getenv("CLOUDFLARE_TURN_SERVICE_API_TOKEN")
    
    if cf_service_id and cf_api_token:
        try:
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    f"https://rtc.live.cloudflare.com/v1/turn/keys/{cf_service_id}/credentials/generate",
                    headers={
                        "Authorization": f"Bearer {cf_api_token}",
                        "Content-Type": "application/json",
                    },
                    json={"ttl": 86400},
                    timeout=aiohttp.ClientTimeout(total=10),
                ) as resp:
                    if resp.status != 200:
                        error_text = await resp.text()
                        logger.error(f"TURN credentials generation error {resp.status}: {error_text}")
                        return DEFAULT_ICE_SERVERS
                    
                    data = await resp.json()
                    ice_servers = data.get("iceServers", [])
                    
                    result = []
                    for server in ice_servers:
                        urls = server.get("urls", [])
                        credential = server.get("credential")
                        username = server.get("username")
                        
                        for url in urls:
                            result.append({
                                "urls": [url],
                                "username": None if url.startswith("stun:") else username,
                                "credential": None if url.startswith("stun:") else credential,
                            })
                    
                    return result if result else DEFAULT_ICE_SERVERS
        except Exception as e:
            logger.error(f"Failed to request ICE servers: {e}")
            return DEFAULT_ICE_SERVERS
    
    return DEFAULT_ICE_SERVERS


def encode_connection_id(connection_id: Optional[bytes]) -> Optional[str]:
    """Hex-encode a connection_id, treating empty/absent values as None."""
    if not connection_id:
        return None
    return connection_id.hex()


def decode_connection_id(hex_id: Optional[str]) -> Optional[bytes]:
    """Decode a hex-encoded connection_id."""
    if not hex_id:
        return None
    try:
        return bytes.fromhex(hex_id)
    except ValueError:
        return None


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Application lifespan context manager."""
    logger.info("Starting Tango Signaling Server")
    yield
    logger.info("Shutting down Tango Signaling Server")


app = FastAPI(title="Tango Signaling Server", lifespan=lifespan)

# Global hub instance
hub = MatchmakingHub()


@app.get("/ok")
async def health_check():
    """Health check endpoint."""
    return PlainTextResponse("ok")


@app.get("/health")
async def health():
    """Health check endpoint (alternative)."""
    return {"status": "ok"}


@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket, session_id: str = Query(...)):
    """WebSocket endpoint for signaling."""
    if not session_id:
        await websocket.close(code=1008, reason="Missing session_id")
        return
    
    # Geolocation check - restrict to China mainland only (optional, only if header present)
    country = websocket.headers.get("cf-ipcountry", "")
    if country and country != "CN":
        await websocket.close(code=1008, reason="Access denied: Service available in China mainland only")
        return
    
    await websocket.accept()
    logger.info(f"Client connected to session {session_id}")
    
    # Initialize attachment
    attachment = SessionAttachment(session_id)
    
    # Add to hub
    await hub.add_connection(session_id, websocket, attachment)
    
    try:
        # Send hello packet with ICE servers
        ice_servers = await get_ice_servers()
        hello_data = {
            "type": "hello",
            "iceServers": ice_servers
        }
        await websocket.send_json(hello_data)
        logger.debug(f"Sent hello to session {session_id}")
        
        # Handle incoming messages
        while True:
            try:
                data = await websocket.receive_bytes()
                await handle_message(websocket, attachment, hub, data)
            except Exception as e:
                logger.error(f"Error handling message: {e}")
                break
    
    except WebSocketDisconnect:
        logger.info(f"Client disconnected from session {session_id}")
    except Exception as e:
        logger.error(f"WebSocket error for session {session_id}: {e}")
    finally:
        # Clean up
        await hub.remove_connection(session_id, websocket)
        try:
            await websocket.close()
        except:
            pass


async def handle_message(
    websocket: WebSocket,
    attachment: SessionAttachment,
    hub: MatchmakingHub,
    message: bytes
):
    """Handle incoming signaling message."""
    try:
        # For now, using a simple JSON-based protocol
        # TODO: Implement full protobuf protocol when proto files are available
        
        if len(message) < 2:
            logger.warning("Message too short")
            return
        
        # Try to parse as JSON (for testing)
        try:
            msg = message.decode('utf-8')
            logger.debug(f"Received message from {attachment.session_id}: {msg[:100]}")
        except:
            # Binary message - log length for now
            logger.debug(f"Received binary message from {attachment.session_id}: {len(message)} bytes")
            
    except Exception as e:
        logger.error(f"Error handling message: {e}")
        try:
            await websocket.close(code=1008, reason="Invalid packet")
        except:
            pass


@app.get("/")
async def index(request: Request):
    """Main endpoint."""
    # Check for WebSocket upgrade
    if request.headers.get("Upgrade", "").lower() == "websocket":
        # This shouldn't reach here in production - FastAPI handles it
        return Response("Use /ws endpoint with WebSocket", status_code=400)
    
    # Regular HTTP GET
    return PlainTextResponse("ok")


@app.api_route("/{full_path:path}", methods=["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"])
async def catch_all(full_path: str, request: Request):
    """Catch-all for undefined routes."""
    if full_path == "" or full_path == "ok":
        return PlainTextResponse("ok")
    return Response("not found", status_code=404)


if __name__ == "__main__":
    import uvicorn
    
    host = os.getenv("SERVER_HOST", "0.0.0.0")
    port = int(os.getenv("SERVER_PORT", "8000"))
    
    uvicorn.run(
        app,
        host=host,
        port=port,
        log_level="info"
    )
