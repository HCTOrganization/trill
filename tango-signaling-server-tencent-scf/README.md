# Tango Signaling Server - Tencent Cloud SCF Edition

A refactored version of the Tango signaling server optimized for deployment on Tencent Cloud's Serverless Cloud Function (SCF).

## What is this?

This is the signaling server component of the Tango real-time communication framework, adapted for Tencent Cloud SCF. It handles WebSocket connections for SDP offer/answer exchange during WebRTC peer connection establishment.

**Original version**: `../tango-signaling-server` (Cloudflare Workers)
**This version**: Tencent Cloud SCF with Node.js Express

## Key Changes from Original

| Aspect | Original (Cloudflare) | SCF Version (Tencent) |
|--------|----------------------|---------------------|
| **Framework** | Cloudflare Workers API | Express.js + WebSocket |
| **State Backend** | Durable Objects | In-memory (or Redis) |
| **Entry Point** | `src/index.ts` (Worker module) | `src/scf-handler.ts` (SCF handler) |
| **Deployment** | `wrangler deploy` | `serverless deploy` |
| **Runtime** | Cloudflare Worker runtime | Node.js 18 |

## Quick Start

### Prerequisites
- Node.js 18+
- Tencent Cloud account with SCF enabled
- `serverless` CLI installed: `npm install -g serverless`

### Local Development
```bash
# Install dependencies
npm install

# Build (generate protos + compile TS)
npm run build

# Run locally
npm run dev
```

Server starts on `http://localhost:3000`. Test with:
```bash
curl http://localhost:3000/ok
```

### Deploy to Tencent Cloud
```bash
# Configure Tencent Cloud credentials
serverless credentials --provider tencentcloud

# Deploy
npm run deploy
```

For more details, see [DEPLOYMENT.md](./DEPLOYMENT.md).

## Architecture

### Components

1. **Express App** (`src/index.ts`)
   - HTTP endpoints for health checks
   - WebSocket server management
   - Session state tracking
   - SDP offer/answer routing

2. **SCF Handler** (`src/scf-handler.ts`)
   - Tencent Cloud function entry point
   - API Gateway WebSocket integration
   - CONNECT/MESSAGE/DISCONNECT event handling

3. **Signaling Protocol**
   - Protobuf-based message format (shared with original)
   - Located at `src/proto/signaling.ts` (generated)

### Data Flow

```
Client 1 (Offerer)
    ↓
WebSocket → SCF Function → Session Store → WebSocket ← SCF Function ← Client 2 (Answerer)
    ↓ (SDP Offer)                                           ↓ (SDP Answer)
    ←────────────────────────────────────────────────────────←
```

## Configuration

### Environment Variables

- `TURN_ADDR` - Custom TURN server address (optional)
- `TURN_USER` - TURN server username (optional)
- `TURN_CREDENTIAL` - TURN server credential (optional)
- `NODE_ENV` - Set to `production` for deployments
- `PORT` - Listen port (default: 3000, ignored on SCF)

### Session Management

Currently uses in-memory storage. For production multi-instance setups, upgrade to Redis:

```typescript
// Update src/index.ts to use Redis
import { createClient } from 'redis';
const redis = createClient({...});
```

## WebSocket Protocol

The signaling protocol uses Protocol Buffers. Key message types:

- **Hello**: Server → Client (ICE servers, session info)
- **Start**: Client → Server (SDP offer, connection ID)
- **Offer**: Server → Client (SDP offer to answerer)
- **Answer**: Client → Server (SDP answer)
- **Ping/Pong**: Keepalive messages

See `../tango-signaling/src/proto/signaling.proto` for full schema.

## Deployment

### Serverless Framework
```bash
serverless deploy                    # Deploy to default region
serverless remove                    # Remove deployment
serverless logs -f websocket         # View function logs
```

### Tencent Cloud Console
Upload `dist/` and `node_modules/` as a ZIP file to SCF console.

### API Gateway Setup
For WebSocket support, configure API Gateway:
1. Create WebSocket API
2. Add routes: `$connect`, `$default`, `$disconnect`
3. Map to SCF function
4. Enable auto-invoke

## File Structure

```
.
├── src/
│   ├── index.ts          # Main Express app + WebSocket server
│   ├── scf-handler.ts    # SCF entry point for Tencent Cloud
│   └── proto/            # Generated protobuf types (not in git)
├── dist/                 # Compiled output (generated)
├── package.json
├── tsconfig.json
├── serverless.yml        # Serverless Framework config
├── wrangler.toml         # (Unused) Cloudflare config
├── DEPLOYMENT.md         # Detailed deployment guide
└── README.md             # This file
```

## Development Workflow

1. **Make changes** to `src/index.ts` or `src/scf-handler.ts`
2. **Test locally**: `npm run dev`
3. **Rebuild**: `npm run build`
4. **Deploy**: `npm run deploy`

## Troubleshooting

### WebSocket connections failing
- Verify API Gateway WebSocket integration is enabled
- Check function timeout (set to at least 30s)
- Review SCF logs for errors

### High latency
- Check TURN/STUN server accessibility
- Consider deploying closer to user regions
- Monitor function cold start duration

### Lost session state
- Upgrade to Redis backend for persistence
- Implement client-side reconnect logic

## Performance Characteristics

- **Memory**: 512 MB recommended (adjustable)
- **Timeout**: 30 seconds per request/connection
- **Concurrency**: Auto-scales with SCF
- **Cold start**: ~2-5s for Node.js 18
- **Warm start**: ~10-100ms

## Known Limitations

1. **In-memory state**: Lost on function restart
2. **Single-region**: Deploy to multiple regions for HA
3. **WebSocket duration**: Limited by SCF timeout (30s default)

## Next Steps

- [ ] Set up API Gateway with WebSocket
- [ ] Configure TURN server (optional)
- [ ] Deploy to Tencent Cloud
- [ ] Run end-to-end WebRTC tests
- [ ] Monitor performance and logs
- [ ] Add Redis backend for multi-instance

## Related Projects

- **tango-signaling**: Protocol definitions
- **tango-signaling-server**: Original Cloudflare Workers version
- **Tango Framework**: Main real-time communication framework

## License

Same as parent project

## Support

For issues:
1. Check [DEPLOYMENT.md](./DEPLOYMENT.md) for detailed guidance
2. Review SCF logs in Tencent Cloud console
3. Compare with original version for protocol questions
4. Consult Serverless Framework docs for deployment issues
