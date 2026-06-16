# Quick Start Guide - Tango Signaling Server on Tencent Cloud SCF

## 5-Minute Setup

### 1. Install Dependencies
```bash
npm install
```

### 2. Build the Project
```bash
npm run build
```
This will:
- Generate protobuf types from `../tango-signaling/src/proto/signaling.proto`
- Compile TypeScript to JavaScript in `dist/`

### 3. Test Locally
```bash
npm run dev
```

Server runs on `http://localhost:3000`. Open another terminal:
```bash
curl http://localhost:3000/ok
# Output: ok
```

## Deploy to Tencent Cloud

### Prerequisites
```bash
# Install Serverless Framework globally
npm install -g serverless

# Configure Tencent Cloud credentials
serverless credentials --provider tencentcloud
# Follow prompts to enter your Secret ID and Secret Key
```

### Deploy
```bash
npm run deploy
```

The Serverless Framework will:
1. Build the project
2. Create/update a Tencent Cloud SCF function
3. Return your function URL and WebSocket endpoint

## Next Steps

1. **Configure API Gateway** (for WebSocket support)
   - Go to Tencent Cloud console
   - API Gateway → Create WebSocket API
   - Add routes and map to your SCF function

2. **Set Environment Variables** (optional TURN server)
   - Edit `serverless.yml` or set in console:
     ```yaml
     environment:
       TURN_ADDR: your-turn-server.com
       TURN_USER: username
       TURN_CREDENTIAL: password
     ```

3. **Test WebSocket Connection**
   - Use a WebSocket client to connect to your API Gateway URL
   - Include `?session_id=test-session` in the connection URL

4. **Monitor Deployment**
   ```bash
   serverless logs -f websocket  # View function logs
   serverless remove             # Delete deployment
   ```

## Project Structure

| File | Purpose |
|------|---------|
| `src/index.ts` | Express server + WebSocket logic |
| `src/scf-handler.ts` | Tencent Cloud SCF entry point |
| `serverless.yml` | Deployment configuration |
| `package.json` | Dependencies and scripts |
| `tsconfig.json` | TypeScript settings |
| `DEPLOYMENT.md` | Detailed deployment guide |

## Common Commands

```bash
npm run build       # Build the project
npm run dev         # Run locally
npm run deploy      # Deploy to Tencent Cloud
npm run typecheck   # Check TypeScript types
```

## Troubleshooting

### Build fails: "Cannot find module 'protobufjs'"
```bash
npm install  # Reinstall dependencies
npm run build
```

### "Missing session_id" error
Add `?session_id=your-session-id` to WebSocket URL:
```
wss://your-api-gateway-url/?session_id=my-session
```

### Function not responding
Check logs:
```bash
serverless logs -f websocket
```

## What's Different from Original?

Original (Cloudflare Workers) → This Version (Tencent Cloud SCF):
- ✅ Same signaling protocol (Protobuf)
- ✅ Same WebSocket offer/answer flow
- ❌ Durable Objects → In-memory sessions (or Redis)
- ❌ Cloudflare Workers API → Express.js + Node.js
- ❌ `wrangler deploy` → `serverless deploy`

## Next: Advanced Configuration

See [DEPLOYMENT.md](./DEPLOYMENT.md) for:
- Redis backend setup
- Multi-region deployment
- Performance tuning
- Production monitoring

## Support

For detailed documentation, see:
- [DEPLOYMENT.md](./DEPLOYMENT.md) - Full deployment guide
- [README.md](./README.md) - Architecture overview
- [Tencent Cloud SCF Docs](https://cloud.tencent.com/document/product/583)
