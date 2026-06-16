"""
Tango Signaling Server - Python Implementation

A WebSocket-based signaling server for WebRTC peer-to-peer communication.
Handles session matchmaking and ICE server provisioning.

Ported from the Cloudflare Workers TypeScript implementation.
Uses the same protobuf wire protocol (tango.signaling.Packet).
"""

import asyncio
import logging
import os
from typing import Optional, Dict, List, Tuple
from contextlib import asynccontextmanager
from dataclasses import dataclass, field

import aiohttp
from fastapi import FastAPI, WebSocket, WebSocketDisconnect, Request, Response, Query
from fastapi.responses import PlainTextResponse

# Proto-generated classes — run `python -m grpc_tools.protoc` to regenerate signaling_pb2.py
from signaling_pb2 import Packet

logger = logging.getLogger(__name__)
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

HUB_SINGLETON_NAME = "global"

DEFAULT_ICE_SERVERS = [
    {"urls": ["stun:stun.l.google.com:19302"],  "username": None, "credential": None},
    {"urls": ["stun:stun1.l.google.com:19302"], "username": None, "credential": None},
    {"urls": ["stun:stun2.l.google.com:19302"], "username": None, "credential": None},
    {"urls": ["stun:stun3.l.google.com:19302"], "username": None, "credential": None},
    {"urls": ["stun:stun4.l.google.com:19302"], "username": None, "credential": None},
]

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def encode_connection_id(connection_id: Optional[bytes]) -> Optional[str]:
    """Hex-encode a connection_id, treating empty/absent values as None."""
    if not connection_id:
        return None
    return connection_id.hex()


async def get_ice_servers() -> list:
    """Fetch ICE servers from environment or Cloudflare TURN service."""
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
                        logger.error(f"TURN credentials error {resp.status}: {await resp.text()}")
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


def build_hello_packet(ice_servers: list) -> bytes:
    """Encode a Hello packet with ICE servers."""
    servers = []
    for s in ice_servers:
        server = Packet.Hello.ICEServer()
        for url in s.get("urls", []):
            server.urls.append(url)
        if s.get("username"):
            server.username = s["username"]
        if s.get("credential"):
            server.credential = s["credential"]
        servers.append(server)

    pkt = Packet()
    pkt.hello.CopyFrom(Packet.Hello(ice_servers=servers))
    return pkt.SerializeToString()


def build_offer_packet(sdp: str) -> bytes:
    pkt = Packet()
    pkt.offer.CopyFrom(Packet.Offer(sdp=sdp))
    return pkt.SerializeToString()


def build_answer_packet(sdp: str) -> bytes:
    pkt = Packet()
    pkt.answer.CopyFrom(Packet.Answer(sdp=sdp))
    return pkt.SerializeToString()


def build_ping_packet() -> bytes:
    pkt = Packet()
    pkt.ping.CopyFrom(Packet.Ping())
    return pkt.SerializeToString()


def build_abort_packet(reason: int) -> bytes:
    pkt = Packet()
    pkt.abort.CopyFrom(Packet.Abort(reason=reason))
    return pkt.SerializeToString()


# ---------------------------------------------------------------------------
# Session attachment
# ---------------------------------------------------------------------------

@dataclass
class SessionAttachment:
    """Per-connection metadata, analogous to CF Workers' serializeAttachment."""
    session_id: str
    offer_sdp: Optional[str] = None
    connection_id: Optional[str] = None   # hex-encoded

    # Identity-based hashing so instances can live in sets/dicts keyed by ws.
    def __hash__(self):
        return id(self)

    def __eq__(self, other):
        return self is other


# ---------------------------------------------------------------------------
# Matchmaking hub
# ---------------------------------------------------------------------------

class MatchmakingHub:
    """
    In-process equivalent of the Cloudflare Durable Object.
    Maps session_id -> list of (ws, attachment) pairs.
    """

    def __init__(self):
        # Using a list per session so we never have hashing issues with ws objects.
        self.connections: Dict[str, List[Tuple[WebSocket, SessionAttachment]]] = {}
        self.lock = asyncio.Lock()

    async def add_connection(self, session_id: str, ws: WebSocket, attachment: SessionAttachment):
        async with self.lock:
            self.connections.setdefault(session_id, []).append((ws, attachment))
        logger.debug(f"Added connection for session {session_id}")

    async def remove_connection(self, session_id: str, ws: WebSocket):
        async with self.lock:
            if session_id in self.connections:
                self.connections[session_id] = [
                    (w, a) for w, a in self.connections[session_id] if w is not ws
                ]
                if not self.connections[session_id]:
                    del self.connections[session_id]
        logger.debug(f"Removed connection for session {session_id}")

    def find_offerer(self, session_id: str) -> Optional[Tuple[WebSocket, SessionAttachment]]:
        """Return the first connection that holds an offer SDP."""
        for ws, attachment in self.connections.get(session_id, []):
            if attachment.offer_sdp is not None:
                return (ws, attachment)
        return None

    def update_attachment(self, session_id: str, ws: WebSocket, new_attachment: SessionAttachment):
        """Replace the attachment for a specific ws in-place (no lock needed — called under lock)."""
        conns = self.connections.get(session_id, [])
        for i, (w, _) in enumerate(conns):
            if w is ws:
                conns[i] = (w, new_attachment)
                break


# ---------------------------------------------------------------------------
# Message handlers (mirror of TypeScript handleStart / handleAnswer)
# ---------------------------------------------------------------------------

async def handle_start(
    ws: WebSocket,
    attachment: SessionAttachment,
    hub: MatchmakingHub,
    start: Packet.Start,
):
    connection_id = encode_connection_id(start.connection_id)
    session_id = attachment.session_id

    async with hub.lock:
        offerer = hub.find_offerer(session_id)

        if offerer is None:
            # No one waiting — become the offerer.
            attachment.offer_sdp = start.offer_sdp or ""
            attachment.connection_id = connection_id
            hub.update_attachment(session_id, ws, attachment)
            logger.debug(f"[{session_id}] Stored offer, waiting for answerer")
            return

        offerer_ws, offerer_att = offerer

        if connection_id is not None and connection_id == offerer_att.connection_id:
            # Same connection_id: offerer is reconnecting with a fresh offer.
            # Replace the stale entry.
            attachment.offer_sdp = start.offer_sdp or ""
            attachment.connection_id = connection_id
            hub.update_attachment(session_id, ws, attachment)

            if offerer_ws is not ws:
                # Clear stale socket's offer before closing it.
                offerer_att.offer_sdp = None
                hub.update_attachment(session_id, offerer_ws, offerer_att)
                try:
                    await offerer_ws.close(code=1000)
                except Exception:
                    pass

            logger.debug(f"[{session_id}] Replaced stale offer from reconnecting offerer")
            return

        # Different peer — hand it the offerer's SDP so it can answer.
        logger.debug(f"[{session_id}] Answerer arrived, sending offer SDP")

    await ws.send_bytes(build_offer_packet(offerer_att.offer_sdp or ""))


async def handle_answer(
    ws: WebSocket,
    attachment: SessionAttachment,
    hub: MatchmakingHub,
    answer: Packet.Answer,
):
    session_id = attachment.session_id

    async with hub.lock:
        offerer = hub.find_offerer(session_id)
        if offerer is None:
            logger.warning(f"[{session_id}] Unexpected answer — no offerer found")
            await ws.close(code=1008, reason="unexpected answer")
            return
        offerer_ws, _ = offerer

    try:
        await offerer_ws.send_bytes(build_answer_packet(answer.sdp))
        await offerer_ws.close(code=1000)
    except Exception:
        pass

    await ws.close(code=1000)
    logger.debug(f"[{session_id}] Answer relayed, both peers closed")


async def handle_message(
    ws: WebSocket,
    attachment: SessionAttachment,
    hub: MatchmakingHub,
    raw: bytes,
):
    """Decode a protobuf Packet and dispatch to the appropriate handler."""
    try:
        packet = Packet()
        packet.ParseFromString(raw)
    except Exception:
        logger.warning(f"[{attachment.session_id}] Failed to parse packet, closing")
        await ws.close(code=1008, reason="invalid packet")
        return

    which = packet.WhichOneof("which")

    if which == "start":
        await handle_start(ws, attachment, hub, packet.start)
    elif which == "answer":
        await handle_answer(ws, attachment, hub, packet.answer)
    elif which == "ping":
        await ws.send_bytes(build_ping_packet())
    elif which == "hello" or which == "offer" or which == "abort":
        # Server should never receive these from a client
        logger.warning(f"[{attachment.session_id}] Unexpected packet type from client: {which}")
        await ws.close(code=1008, reason=f"unexpected packet type: {which}")
    else:
        logger.warning(f"[{attachment.session_id}] Unknown packet type, ignoring")


# ---------------------------------------------------------------------------
# FastAPI app
# ---------------------------------------------------------------------------

@asynccontextmanager
async def lifespan(app: FastAPI):
    logger.info("Starting Tango Signaling Server")
    yield
    logger.info("Shutting down Tango Signaling Server")


app = FastAPI(title="Tango Signaling Server", lifespan=lifespan)

hub = MatchmakingHub()


@app.get("/ok")
async def health_check():
    return PlainTextResponse("ok")


@app.get("/health")
async def health():
    return {"status": "ok"}


@app.websocket("/ws")
@app.websocket("/")
async def websocket_endpoint(websocket: WebSocket, session_id: str = Query(...)):
    """WebSocket signaling endpoint. Requires ?session_id=<id>."""
    await websocket.accept()
    logger.info(f"Client connected to session {session_id}")

    attachment = SessionAttachment(session_id=session_id)
    await hub.add_connection(session_id, websocket, attachment)

    try:
        ice_servers = await get_ice_servers()
        await websocket.send_bytes(build_hello_packet(ice_servers))
        logger.debug(f"[{session_id}] Sent hello")

        while True:
            try:
                data = await websocket.receive_bytes()
                await handle_message(websocket, attachment, hub, data)
            except WebSocketDisconnect:
                raise
            except Exception as e:
                logger.error(f"[{session_id}] Error handling message: {e}")
                break

    except WebSocketDisconnect:
        logger.info(f"[{session_id}] Client disconnected")
    except Exception as e:
        logger.error(f"[{session_id}] WebSocket error: {e}")
    finally:
        await hub.remove_connection(session_id, websocket)
        try:
            await websocket.close()
        except Exception:
            pass


@app.get("/")
async def index(request: Request):
    if request.headers.get("Upgrade", "").lower() == "websocket":
        return Response("Use WebSocket upgrade", status_code=400)
    return PlainTextResponse("ok")


@app.api_route("/{full_path:path}", methods=["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"])
async def catch_all(full_path: str, request: Request):
    if full_path in ("", "ok"):
        return PlainTextResponse("ok")
    return Response("not found", status_code=404)


if __name__ == "__main__":
    import uvicorn

    host = os.getenv("SERVER_HOST", "0.0.0.0")
    port = int(os.getenv("SERVER_PORT", "8000"))

    uvicorn.run(app, host=host, port=port, log_level="info")
