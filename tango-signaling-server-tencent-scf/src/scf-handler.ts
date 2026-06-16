/**
 * Tencent Cloud SCF Handler for WebSocket and HTTP
 * 
 * This file serves as the entry point for Tencent Cloud Serverless Cloud Function.
 * SCF has special requirements for WebSocket and HTTP handling.
 * 
 * For production WebSocket support, you typically need to use:
 * 1. API Gateway with WebSocket integration (recommended)
 * 2. Function URLs (for newer SCF versions)
 * 3. Custom CloudFlare/ALB setup
 */

import { Request, Response } from "express";
import { app, server } from "./index";

// Store active connections for SCF
const activeConnections = new Map<string, any>();

/**
 * HTTP handler for regular HTTP requests
 * This is used when the function is triggered via HTTP/API Gateway
 */
export async function httpHandler(event: any, context: any) {
  try {
    console.log("HTTP request received:", {
      path: event.path,
      httpMethod: event.httpMethod,
      headers: event.headers,
    });

    // Convert SCF event to Express request format
    const req = {
      method: event.httpMethod,
      url: event.path + (event.queryStringParameters 
        ? "?" + new URLSearchParams(event.queryStringParameters).toString()
        : ""),
      headers: event.headers || {},
      body: event.body || "",
    };

    // Create a response object
    let statusCode = 200;
    let responseBody = "";
    let responseHeaders: Record<string, any> = {};

    // Handle health check
    if (event.path === "/ok") {
      return {
        statusCode: 200,
        body: "ok",
        headers: { "Content-Type": "text/plain" },
      };
    }

    // For WebSocket upgrade requests, we need special handling
    if (event.headers?.upgrade?.toLowerCase() === "websocket") {
      // Note: Direct WebSocket handling via HTTP is complex in SCF
      // Recommend using API Gateway with WebSocket support instead
      return {
        statusCode: 426,
        body: JSON.stringify({
          error: "WebSocket upgrade required",
          message:
            "Use API Gateway WebSocket integration for WebSocket connections",
        }),
        headers: { "Content-Type": "application/json" },
      };
    }

    return {
      statusCode: 404,
      body: "not found",
      headers: { "Content-Type": "text/plain" },
    };
  } catch (error) {
    console.error("HTTP handler error:", error);
    return {
      statusCode: 500,
      body: JSON.stringify({
        error: "Internal Server Error",
        message: error instanceof Error ? error.message : "Unknown error",
      }),
      headers: { "Content-Type": "application/json" },
    };
  }
}

/**
 * WebSocket handler for API Gateway WebSocket trigger
 * 
 * SCF WebSocket functions have three types of triggers:
 * 1. CONNECT - when a client establishes a WebSocket connection
 * 2. MESSAGE - when a client sends a message
 * 3. DISCONNECT - when a client closes the connection
 */
export async function websocketHandler(event: any, context: any) {
  try {
    const requestContext = event.requestContext || {};
    const connectionId = requestContext.connectionId;
    const eventType = requestContext.eventType;
    const sessionId = event.queryStringParameters?.session_id;

    console.log("WebSocket event:", {
      eventType,
      connectionId,
      sessionId,
    });

    switch (eventType) {
      case "CONNECT":
        return handleConnect(event, connectionId, sessionId);
      case "MESSAGE":
        return handleMessage(event, connectionId, sessionId);
      case "DISCONNECT":
        return handleDisconnect(connectionId);
      default:
        console.warn("Unknown WebSocket event type:", eventType);
        return { statusCode: 400 };
    }
  } catch (error) {
    console.error("WebSocket handler error:", error);
    return { statusCode: 500 };
  }
}

/**
 * Handle WebSocket CONNECT event
 * Client initiates a connection to the signaling server
 */
function handleConnect(
  event: any,
  connectionId: string,
  sessionId: string | undefined,
): any {
  try {
    if (!sessionId) {
      console.warn("Connection attempt without session_id");
      return {
        statusCode: 400,
        body: JSON.stringify({ error: "Missing session_id parameter" }),
      };
    }

    // Store connection metadata
    activeConnections.set(connectionId, {
      sessionId,
      connectedAt: Date.now(),
      lastActivity: Date.now(),
    });

    console.log("WebSocket connected:", {
      connectionId,
      sessionId,
      totalConnections: activeConnections.size,
    });

    // Note: Sending initial hello message would require API Gateway callback
    // which is more complex. See handleMessage for actual message processing.
    return { statusCode: 200 };
  } catch (error) {
    console.error("Connect handler error:", error);
    return { statusCode: 500 };
  }
}

/**
 * Handle WebSocket MESSAGE event
 * Client sends a message to the signaling server
 */
function handleMessage(
  event: any,
  connectionId: string,
  sessionId: string | undefined,
): any {
  try {
    const body = event.body;
    if (!body) {
      return { statusCode: 400 };
    }

    const connection = activeConnections.get(connectionId);
    if (!connection) {
      return {
        statusCode: 400,
        body: JSON.stringify({ error: "Connection not found" }),
      };
    }

    // Update last activity
    connection.lastActivity = Date.now();

    // Note: Actual message processing would require:
    // 1. Parsing the protobuf message
    // 2. Processing via the signaling logic
    // 3. Sending responses back via API Gateway callback
    // 
    // For now, this is a placeholder for the actual implementation
    console.log("Message received from", connectionId, ":", body.length, "bytes");

    return { statusCode: 200 };
  } catch (error) {
    console.error("Message handler error:", error);
    return { statusCode: 500 };
  }
}

/**
 * Handle WebSocket DISCONNECT event
 * Client closes the connection
 */
function handleDisconnect(connectionId: string): any {
  try {
    const connection = activeConnections.get(connectionId);
    if (connection) {
      console.log("WebSocket disconnected:", {
        connectionId,
        sessionId: connection.sessionId,
        duration: Date.now() - connection.connectedAt,
      });
      activeConnections.delete(connectionId);
    }

    return { statusCode: 200 };
  } catch (error) {
    console.error("Disconnect handler error:", error);
    return { statusCode: 500 };
  }
}

/**
 * Alternative: Express app wrapper for direct HTTP trigger
 * Use this if deploying via direct HTTP/function URL instead of API Gateway
 */
export const handler = async (event: any, context: any) => {
  // Detect event type
  if (event.requestContext?.eventType) {
    // WebSocket event from API Gateway
    return websocketHandler(event, context);
  } else if (event.httpMethod) {
    // HTTP event
    return httpHandler(event, context);
  } else {
    // Fallback
    return {
      statusCode: 400,
      body: JSON.stringify({ error: "Invalid event" }),
    };
  }
};

export default handler;
