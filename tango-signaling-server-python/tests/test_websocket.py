"""
WebSocket connection tests for Tango Signaling Server.
"""

import asyncio
import pytest
import json
from httpx import AsyncClient
from fastapi.testclient import TestClient

# Import after adding parent directory to path
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))

from app import app, MatchmakingHub, SessionAttachment


@pytest.fixture
def client():
    """Provide a test client."""
    return TestClient(app)


def test_health_check(client):
    """Test the /ok health check endpoint."""
    response = client.get("/ok")
    assert response.status_code == 200
    assert response.text == "ok"


def test_health_json(client):
    """Test the /health JSON endpoint."""
    response = client.get("/health")
    assert response.status_code == 200
    assert response.json() == {"status": "ok"}


def test_root_endpoint(client):
    """Test the root / endpoint."""
    response = client.get("/")
    assert response.status_code == 200
    assert response.text == "ok"


def test_websocket_without_session_id(client):
    """Test WebSocket connection without session_id."""
    with pytest.raises(Exception):
        with client.websocket_connect("/ws") as websocket:
            pass


@pytest.mark.asyncio
async def test_matchmaking_hub():
    """Test the MatchmakingHub class."""
    from fastapi import WebSocket
    
    hub = MatchmakingHub()
    
    # Create mock WebSocket objects (simplified)
    class MockWebSocket:
        def __init__(self, client_id):
            self.client_id = client_id
    
    ws1 = MockWebSocket("client1")
    ws2 = MockWebSocket("client2")
    
    # Test adding connections
    att1 = SessionAttachment("session1")
    att2 = SessionAttachment("session1")
    
    await hub.add_connection("session1", ws1, att1)
    await hub.add_connection("session1", ws2, att2)
    
    assert "session1" in hub.connections
    assert len(hub.connections["session1"]) == 2
    
    # Test finding offerer (should find none initially)
    offerer = hub.find_offerer("session1")
    assert offerer is None
    
    # Test removing connection
    await hub.remove_connection("session1", ws1)
    assert len(hub.connections["session1"]) == 1


@pytest.mark.asyncio
async def test_matchmaking_hub_offerer():
    """Test finding offerer in MatchmakingHub."""
    hub = MatchmakingHub()
    
    class MockWebSocket:
        def __init__(self, client_id):
            self.client_id = client_id
    
    ws1 = MockWebSocket("offerer")
    ws2 = MockWebSocket("answerer")
    
    # Offerer with SDP
    att1 = SessionAttachment("session1", offer_sdp="offer_data")
    # Answerer without SDP
    att2 = SessionAttachment("session1")
    
    await hub.add_connection("session1", ws1, att1)
    await hub.add_connection("session1", ws2, att2)
    
    # Find offerer
    offerer = hub.find_offerer("session1")
    assert offerer is not None
    assert offerer[0].client_id == "offerer"
    assert offerer[1].offer_sdp == "offer_data"


@pytest.mark.asyncio
async def test_session_attachment():
    """Test SessionAttachment dataclass."""
    att = SessionAttachment(
        session_id="test-session",
        offer_sdp="test-offer",
        connection_id="test-conn-id"
    )
    
    assert att.session_id == "test-session"
    assert att.offer_sdp == "test-offer"
    assert att.connection_id == "test-conn-id"
    
    # Test with defaults
    att2 = SessionAttachment("another-session")
    assert att2.offer_sdp is None
    assert att2.connection_id is None


def test_404_endpoint(client):
    """Test 404 response for unknown endpoints."""
    response = client.get("/unknown/path")
    assert response.status_code == 404
    assert response.text == "not found"


def test_connection_id_encoding():
    """Test connection_id encoding/decoding."""
    from app import encode_connection_id, decode_connection_id
    
    # Test with valid bytes
    test_bytes = b'\x01\x02\x03\x04'
    encoded = encode_connection_id(test_bytes)
    assert encoded == "01020304"
    
    # Test with decoded bytes
    decoded = decode_connection_id(encoded)
    assert decoded == test_bytes
    
    # Test with None
    assert encode_connection_id(None) is None
    assert encode_connection_id(b'') is None
    
    # Test decode with None
    assert decode_connection_id(None) is None
    assert decode_connection_id("") is None


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
