import express, { Request, Response } from "express";
import { WebSocketServer, WebSocket } from "ws";
import http from "http";
import { tango } from "./proto/signaling";

const app = express();
const Packet = tango.signaling.Packet;
const AbortReason = Packet.Abort.Reason;

const HUB_SINGLETON_NAME = "global";

// In-memory session store
// For production, replace with Redis
interface SessionData {
  sessionId: string;
  offerSdp?: string;
  connectionId?: string;
}

const sessions = new Map<string, Map<WebSocket, SessionData>>();

interface ICEServer {
  urls: string[];
  username?: string | null;
  credential?: string | null;
}

async function getICEServers(): Promise<ICEServer[]> {
  // Check for self-hosted TURN server from environment variables
  if (
    process.env.TURN_ADDR &&
    process.env.TURN_USER &&
    process.env.TURN_CREDENTIAL
  ) {
    return [
      {
        urls: [`turn:${process.env.TURN_ADDR}`],
        username: process.env.TURN_USER,
        credential: process.env.TURN_CREDENTIAL,
      },
    ];
  }

  // Default Google STUN servers
  return [
    "stun:stun.l.google.com:19302",
    "stun:stun1.l.google.com:19302",
    "stun:stun2.l.google.com:19302",
    "stun:stun3.l.google.com:19302",
    "stun:stun4.l.google.com:19302",
  ].map((uri) => ({ urls: [uri], username: null, credential: null }));
}

// Hex-encode a connection_id
function encodeConnectionId(
  connectionId: Uint8Array | null | undefined,
): string | undefined {
  if (connectionId == null || connectionId.length === 0) {
    return undefined;
  }
  let hex = "";
  for (const byte of connectionId) {
    hex += byte.toString(16).padStart(2, "0");
  }
  return hex;
}

// Find the offerer in a session
function findOfferer(
  sessionId: string,
): { ws: WebSocket; data: SessionData } | null {
  const sessionSockets = sessions.get(sessionId);
  if (!sessionSockets) return null;

  for (const [ws, data] of sessionSockets) {
    if (data.offerSdp != null) {
      return { ws, data };
    }
  }
  return null;
}

// Handle incoming WebSocket message
function handleWebSocketMessage(
  ws: WebSocket,
  sessionId: string,
  message: Buffer,
): void {
  let packet: tango.signaling.Packet;
  try {
    packet = Packet.decode(new Uint8Array(message));
  } catch (error) {
    console.error("Failed to decode packet:", error);
    ws.close(1008, "invalid packet");
    return;
  }

  const sessionSockets = sessions.get(sessionId);
  if (!sessionSockets || !sessionSockets.has(ws)) {
    ws.close(1011, "session not found");
    return;
  }

  const data = sessionSockets.get(ws);
  if (!data) {
    ws.close(1011, "missing session data");
    return;
  }

  switch (packet.which) {
    case "start":
      handleStart(ws, sessionId, data, packet.start!);
      break;
    case "answer":
      handleAnswer(ws, sessionId, data, packet.answer!);
      break;
    case "ping":
      ws.send(Packet.encode({ pong: {} }).finish());
      break;
    default:
      console.warn("Unknown packet type:", packet.which);
  }
}

function handleStart(
  ws: WebSocket,
  sessionId: string,
  data: SessionData,
  start: tango.signaling.Packet.IStart,
): void {
  const connectionId = encodeConnectionId(start.connectionId);
  const offerer = findOfferer(sessionId);

  if (offerer == null) {
    // No one is waiting yet: become the offerer
    data.offerSdp = start.offerSdp ?? "";
    data.connectionId = connectionId;
    return;
  }

  if (
    connectionId != null &&
    connectionId === offerer.data.connectionId
  ) {
    // Same connection_id as the offer already on file: this is the offerer
    // reconnecting with a fresh offer. Replace the stale offer.
    data.offerSdp = start.offerSdp ?? "";
    data.connectionId = connectionId;

    if (offerer.ws !== ws) {
      // Clear the stale socket's offer and close it
      const offererData = sessions.get(sessionId)?.get(offerer.ws);
      if (offererData) {
        offererData.offerSdp = undefined;
        offererData.connectionId = undefined;
      }
      try {
        offerer.ws.close(1000);
      } catch {}
    }
    return;
  }

  // A different peer: hand it the offerer's SDP so it can answer
  ws.send(
    Packet.encode({ offer: { sdp: offerer.data.offerSdp! } }).finish(),
  );
}

function handleAnswer(
  ws: WebSocket,
  sessionId: string,
  data: SessionData,
  answer: tango.signaling.Packet.IAnswer,
): void {
  const offerer = findOfferer(sessionId);
  if (offerer == null) {
    ws.close(1008, "unexpected answer");
    return;
  }

  try {
    offerer.ws.send(
      Packet.encode({ answer: { sdp: answer.sdp } }).finish(),
    );
    offerer.ws.close(1000);
  } catch (error) {
    console.error("Failed to send answer:", error);
  }

  ws.close(1000);
}

// Express HTTP request handler for health checks and WebSocket upgrade
app.get("/ok", (req: Request, res: Response) => {
  res.send("ok");
});

app.get("/", (req: Request, res: Response) => {
  res.status(404).send("not found");
});

// Create HTTP server
const server = http.createServer(app);

// Create WebSocket server attached to the HTTP server
const wss = new WebSocketServer({ server, noServer: true });

// Handle WebSocket upgrade
server.on("upgrade", async (request, socket, head) => {
  const url = new URL(request.url || "", `http://${request.headers.host}`);

  if (url.pathname !== "/") {
    socket.destroy();
    return;
  }

  const sessionId = url.searchParams.get("session_id");
  if (!sessionId) {
    try {
      const packet = Packet.encode({
        abort: { reason: AbortReason.REASON_MISSING_SESSION_ID },
      }).finish();
      socket.write(
        `HTTP/1.1 400 Bad Request\r\nContent-Type: application/octet-stream\r\nContent-Length: ${packet.length}\r\n\r\n`,
      );
      socket.write(packet);
    } catch (error) {
      console.error("Failed to send abort:", error);
    }
    socket.destroy();
    return;
  }

  if (request.headers.upgrade?.toLowerCase() !== "websocket") {
    try {
      const packet = Packet.encode({
        abort: { reason: AbortReason.REASON_NOT_UPGRADE },
      }).finish();
      socket.write(
        `HTTP/1.1 400 Bad Request\r\nContent-Type: application/octet-stream\r\nContent-Length: ${packet.length}\r\n\r\n`,
      );
      socket.write(packet);
    } catch (error) {
      console.error("Failed to send upgrade error:", error);
    }
    socket.destroy();
    return;
  }

  wss.handleUpgrade(request, socket, head, (ws) => {
    // Initialize session if needed
    if (!sessions.has(sessionId)) {
      sessions.set(sessionId, new Map());
    }

    const sessionSockets = sessions.get(sessionId)!;
    const sessionData: SessionData = { sessionId };
    sessionSockets.set(ws, sessionData);

    // Send hello message with ICE servers
    getICEServers()
      .then((iceServers) => {
        try {
          const packet = Packet.encode({
            hello: {
              iceServers: iceServers as any,
            },
          }).finish();
          ws.send(packet);
        } catch (error) {
          console.error("Failed to send hello:", error);
          ws.close(1011, "server error");
        }
      })
      .catch((error) => {
        console.error("Failed to get ICE servers:", error);
        ws.close(1011, "server error");
      });

    // Handle WebSocket messages
    ws.on("message", (data: Buffer) => {
      handleWebSocketMessage(ws, sessionId, data);
    });

    // Cleanup on close
    ws.on("close", () => {
      sessionSockets.delete(ws);
      if (sessionSockets.size === 0) {
        sessions.delete(sessionId);
      }
    });

    ws.on("error", (error) => {
      console.error("WebSocket error:", error);
    });
  });
});

// Start the server
const PORT = process.env.PORT || 3000;
server.listen(PORT, () => {
  console.log(`Server listening on port ${PORT}`);
});

// Export app for testing or cloud function wrapper
export { app, server };
