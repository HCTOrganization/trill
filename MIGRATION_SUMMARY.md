# Tango Signaling Server - Tencent Cloud SCF Migration Summary

## What Was Done

Successfully created **`tango-signaling-server-tencent-scf`** as a refactored version of the original Tango signaling server, optimized for deployment on Tencent Cloud's Serverless Cloud Function (SCF).

### Directory Structure

```
trill5/
├── tango-signaling-server/                    (Original - Cloudflare Workers)
└── tango-signaling-server-tencent-scf/        (NEW - Tencent Cloud SCF)
    ├── src/
    │   ├── index.ts                           # Express.js + WebSocket server
    │   ├── scf-handler.ts                     # Tencent Cloud SCF entry points
    │   └── proto/                             # Generated from ../tango-signaling/src/proto/signaling.proto
    ├── dist/                                   # Compiled output (generated on build)
    ├── package.json                           # Updated dependencies for Node.js + Express
    ├── tsconfig.json                          # Node.js TypeScript config
    ├── serverless.yml                         # Serverless Framework deployment config
    ├── .env.example                           # Environment variables template
    ├── .gitignore                             # Updated for SCF development
    ├── README.md                              # SCF-specific documentation
    ├── DEPLOYMENT.md                          # Detailed deployment guide
    ├── QUICKSTART.md                          # 5-minute setup guide
    └── wrangler.toml                          # (Legacy - not used)
```

## Key Changes Made

### 1. Framework Migration
| Aspect | Original | SCF Version |
|--------|----------|------------|
| **Base Framework** | Cloudflare Workers API | Express.js + Node.js |
| **WebSocket Library** | Cloudflare WebSocket API | `ws` library |
| **State Storage** | Durable Objects | In-memory Map (upgradeable to Redis) |
| **HTTP Server** | Native Workers | `http.createServer()` |
| **Runtime** | Cloudflare Worker V8 | Node.js 18+ |

### 2. Files Updated

**package.json**
- Removed: `wrangler`, `cf-typegen`
- Added: `express`, `ws`, `@types/express`, `@types/ws`, `serverless` (optional)
- Updated scripts for Node.js builds (`tsc` instead of `wrangler`)

**tsconfig.json**
- Changed `module` from `ESNext` to `commonjs` (SCF standard)
- Added `outDir: "./dist"` for compiled output
- Removed `moduleResolution: "Bundler"`
- Removed Cloudflare-specific type definitions

**src/index.ts**
- Removed: Cloudflare Workers imports, Durable Objects, `ExportedHandler`
- Added: Express.js HTTP server setup, native WebSocket server (`ws` library)
- Refactored: Session management to use in-memory Map instead of Durable Objects
- Replaced: WebSocket upgrade handling to use HTTP server's `upgrade` event
- Kept: Protobuf message handling, signaling protocol logic

**New: src/scf-handler.ts**
- Tencent Cloud SCF entry point
- Handles three event types:
  - `CONNECT`: Client connects via WebSocket/API Gateway
  - `MESSAGE`: Client sends signaling message
  - `DISCONNECT`: Client closes connection
- HTTP handler for health checks
- Connection state management for SCF environment

### 3. Configuration Files Created

**serverless.yml**
- Serverless Framework configuration for Tencent Cloud
- Function definition with 512MB memory, 30s timeout
- Environment variable templates for TURN server
- Deployment packaging configuration

**.env.example**
- Template for local development and deployment variables
- TURN server configuration options
- Tencent Cloud credentials placeholders

### 4. Documentation Created

**README.md** - Overview and quick reference
- Architecture overview
- WebSocket protocol explanation
- Deployment options
- Performance characteristics
- Known limitations

**DEPLOYMENT.md** - Comprehensive deployment guide
- Prerequisites and setup instructions
- Three deployment options:
  1. Serverless Framework (recommended)
  2. Tencent Cloud Console (manual)
  3. Tencent Cloud CLI
- WebSocket configuration details
- State management guidance
- Monitoring and troubleshooting

**QUICKSTART.md** - 5-minute setup guide
- Quick start for local testing
- Fast deployment steps
- Common commands reference

## How It Works

### Local Development Flow
```bash
npm install              # Install dependencies
npm run build           # Generate protos + compile TS to dist/
npm run dev             # Start Express server on http://localhost:3000
curl http://localhost:3000/ok  # Test health endpoint
```

### Deployment Flow (Serverless Framework)
```bash
npm run build           # Build locally
serverless deploy       # Deploy to Tencent Cloud SCF
serverless logs -f websocket  # View logs
```

### WebSocket Connection Flow
1. Client connects with `?session_id=xyz` parameter
2. SCF/API Gateway upgrades to WebSocket
3. Server sends `Hello` with ICE servers
4. Offerer sends `Start` with offer SDP
5. Answerer sends `Answer` with answer SDP
6. Server routes SDP between peers
7. Connection closes

## Architecture Differences

### Original (Cloudflare Workers)
```
Client → WebSocket → Cloudflare Worker
                    ↓
              Durable Objects (State)
                    ↓
         Persistent global state
```

### SCF Version
```
Client → WebSocket → API Gateway
                    ↓
              SCF Function
                    ↓
         In-memory session store
         (or Redis for multi-instance)
```

## State Management

### Current Implementation: In-Memory
- Suitable for single-instance deployments
- Lost on function restart
- Fast access to session data
- See `sessions` Map in `src/index.ts`

### Production Recommendation: Redis
To upgrade to Redis for multi-instance deployments:
1. Provision Tencent Cloud Redis or external Redis
2. Update `src/index.ts` to use Redis client
3. Replace `sessions` Map with Redis operations
4. Update environment variables with Redis connection string

Example:
```typescript
import { createClient } from 'redis';
const redis = createClient({
  socket: { host: process.env.REDIS_HOST }
});
```

## Dependencies Added

```json
{
  "express": "^4.18.2",           // HTTP server framework
  "ws": "^8.14.2",                // WebSocket library
  "redis": "^4.6.11",             // For future Redis backend
  "@types/express": "^4.17.21",   // TypeScript types
  "@types/ws": "^8.5.10",         // TypeScript types
  "serverless": "^3.38.0"         // Optional deployment tool
}
```

## Next Steps

### Before Deployment
1. [ ] Create Tencent Cloud account (if not already)
2. [ ] Configure Tencent Cloud credentials: `serverless credentials --provider tencentcloud`
3. [ ] Set up API Gateway with WebSocket support (optional but recommended)
4. [ ] Configure TURN server (optional - uses Google STUN by default)

### First Deployment
```bash
cd tango-signaling-server-tencent-scf
npm install
npm run build
npm run deploy          # or: serverless deploy
```

### Post-Deployment
1. [ ] Test WebSocket connection via API Gateway
2. [ ] Monitor function logs and metrics
3. [ ] Configure custom domain (optional)
4. [ ] Set up CloudWatch monitoring (optional)
5. [ ] Consider Redis backend for production

### Production Checklist
- [ ] Enable VPC if database access is needed
- [ ] Set up reserved concurrency for consistent performance
- [ ] Configure TURN server for better connectivity
- [ ] Implement Redis backend for state persistence
- [ ] Add CloudWatch alarms for errors/latency
- [ ] Document deployment procedures for team
- [ ] Test failover and recovery scenarios

## Comparing Original and New Versions

| Feature | Original | SCF Version |
|---------|----------|------------|
| **Framework** | Cloudflare Workers | Express.js |
| **Signaling Protocol** | ✅ Same Protobuf | ✅ Same Protobuf |
| **WebSocket Support** | ✅ Native | ✅ Via `ws` library |
| **Session Persistence** | ✅ Durable Objects | ⚠️ In-memory (upgradeable) |
| **Deployment Tool** | `wrangler` | `serverless` or console |
| **Cold Start** | ~50ms | ~2-5s |
| **Regional Availability** | Global CDN | Tencent Cloud regions |
| **Cost Model** | Request-based | Execution time + memory |

## Troubleshooting

### Build Fails
```bash
npm install  # Reinstall dependencies
npm run build
```

### WebSocket Connection Fails
- Verify API Gateway WebSocket configuration
- Check SCF function logs: `serverless logs -f websocket`
- Ensure function timeout ≥ 30 seconds
- Test with `wscat` tool: `npm install -g wscat`

### High Latency
- Check TURN/STUN server accessibility
- Monitor function duration in logs
- Consider regional deployment
- Upgrade to larger memory allocation

## References

- Original Tango Signaling Server: `../tango-signaling-server`
- Protobuf Definitions: `../tango-signaling/src/proto/signaling.proto`
- [Tencent Cloud SCF Docs](https://cloud.tencent.com/document/product/583)
- [Serverless Framework Tencent Provider](https://www.serverless.com/framework/docs/providers/tencentcloud)
- [Express.js Documentation](https://expressjs.com/)
- [ws WebSocket Library](https://github.com/websockets/ws)

## Support & Questions

For deployment-specific issues, refer to:
1. **QUICKSTART.md** for immediate setup
2. **DEPLOYMENT.md** for detailed configuration
3. **README.md** for architecture and API details
4. Tencent Cloud console logs for runtime errors
5. Original `../tango-signaling-server` for protocol questions

---

**Status**: ✅ Migration Complete - Ready for Testing and Deployment
