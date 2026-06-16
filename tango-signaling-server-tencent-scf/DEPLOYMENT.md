# Tango Signaling Server - Tencent Cloud SCF Deployment

This is a refactored version of the tango-signaling-server for deployment on Tencent Cloud's Serverless Cloud Function (SCF).

## Architecture Overview

- **Framework**: Express.js with `ws` library
- **State Management**: In-memory session store (can be upgraded to Redis for multi-instance)
- **WebSocket**: Native Node.js WebSocket support via `ws` library
- **Deployment**: Serverless Framework or Tencent Cloud Console

## Prerequisites

1. **Tencent Cloud Account** with SCF enabled
2. **Node.js 18+** installed locally
3. **Serverless Framework** installed: `npm install -g serverless`
4. **Tencent Cloud credentials** configured:
   ```bash
   serverless credentials --provider tencentcloud
   ```

## Local Development

### Build the project
```bash
npm install
npm run proto  # Generate protobuf types
npm run build  # Compile TypeScript
```

### Run locally
```bash
npm run dev
```

The server will start on `http://localhost:3000`.

### Test the signaling server
```bash
# Health check
curl http://localhost:3000/ok

# WebSocket connection (requires WebSocket client)
# ws://localhost:3000/?session_id=test-session
```

## Deployment Options

### Option 1: Serverless Framework (Recommended)

#### Setup
```bash
npm install -g serverless
npm install
npm run build
```

#### Deploy
```bash
# Deploy to Tencent Cloud
serverless deploy

# Deploy with custom parameters
serverless deploy --param="turnAddr=your-turn-server.com" \
  --param="turnUser=your-user" \
  --param="turnCredential=your-credential"
```

#### Remove deployment
```bash
serverless remove
```

### Option 2: Tencent Cloud Console (Manual)

1. **Create a new function**:
   - Region: Choose appropriate region (e.g., ap-shanghai)
   - Runtime: Node.js 18
   - Handler: `dist/scf-handler.handler`
   - Memory: 512 MB
   - Timeout: 30 seconds

2. **Upload code**:
   ```bash
   npm run build
   zip -r function.zip dist/ node_modules/ package.json
   ```
   Upload `function.zip` via console

3. **Configure triggers**:
   - **For HTTP-triggered functions**: Use API Gateway
   - **For WebSocket**: Use API Gateway with WebSocket integration

### Option 3: Tencent Cloud CLI

```bash
# Install tencent-cli
npm install -g tencentcloud-cli

# Deploy
tencentcloud scf CreateFunction --FunctionName tango-signaling --Runtime nodejs18 \
  --Handler "dist/scf-handler.handler" --Code "S3Bucket=your-bucket,S3Key=function.zip"
```

## WebSocket Configuration

### API Gateway WebSocket Integration (Recommended)

For production WebSocket support, configure API Gateway with WebSocket:

1. Create an API Gateway with WebSocket support
2. Set up three routes:
   - `$connect` → triggers `scf-handler.websocketHandler`
   - `$default` → triggers `scf-handler.websocketHandler`
   - `$disconnect` → triggers `scf-handler.websocketHandler`
3. Enable API Gateway to invoke your SCF function
4. Point clients to the API Gateway WebSocket URL

### Function URL (Alternative)

If using Function URLs (supported in newer SCF versions):

1. Enable Function URL in SCF console
2. Set up WebSocket trigger on the function URL
3. Clients connect to: `wss://your-function-url?session_id=xyz`

## Environment Variables

Set these in the SCF console or via `serverless.yml`:

- **TURN_ADDR** (optional): Your TURN server address
  - Default: Uses Google STUN servers only
- **TURN_USER** (optional): TURN server username
- **TURN_CREDENTIAL** (optional): TURN server credential
- **NODE_ENV**: Set to `production` for deployments

## State Management

### Current: In-Memory

The current implementation uses in-memory session storage. This works for:
- Single-instance deployments
- Testing and development
- Stateless function invocations

### Production: Redis (Future)

For production with multiple instances, upgrade to Redis:

1. Set up Tencent Cloud Redis or external Redis
2. Update `src/index.ts` to use Redis client
3. Example:
   ```typescript
   import { createClient } from 'redis';
   const redisClient = createClient({
     socket: { host: process.env.REDIS_HOST, port: 6379 }
   });
   ```
4. Replace `sessions` Map with Redis storage

## Monitoring and Logs

### View logs
```bash
# Via Serverless Framework
serverless logs -f websocket

# Via Tencent Cloud console
# Navigate to: SCF → Functions → Your Function → Logs
```

### CloudWatch/Monitoring
- Function invocation count
- Error rate
- Duration
- Memory usage

## Performance Tuning

### Memory Configuration
- **512 MB** (default): Good for most workloads
- **1 GB+**: For high-throughput signaling (many concurrent sessions)

### Concurrency
- SCF scales automatically
- Configure reserved concurrency in console if needed

### Cold Start Optimization
- Use Provisioned Concurrency for consistent performance
- Keep function package size minimal

## Troubleshooting

### WebSocket connections timing out
- Check API Gateway timeout configuration
- Verify TURN/STUN server accessibility
- Check function logs for errors

### High latency
- Check network performance to TURN/STUN servers
- Consider using regional TURN servers
- Monitor function duration and cold starts

### Session data loss
- Switch to Redis backend for persistence
- Implement retry logic in client code

## File Structure

```
tango-signaling-server-tencent-scf/
├── src/
│   ├── index.ts              # Main Express server with WebSocket
│   ├── scf-handler.ts        # Tencent Cloud SCF entry points
│   └── proto/                # Generated protobuf types
├── dist/                     # Compiled JavaScript (generated)
├── package.json             # Dependencies
├── tsconfig.json            # TypeScript configuration
├── serverless.yml           # Serverless Framework config
├── DEPLOYMENT.md            # This file
└── wrangler.toml            # (Legacy) Cloudflare config
```

## References

- [Tencent Cloud SCF Documentation](https://cloud.tencent.com/document/product/583)
- [Serverless Framework for Tencent Cloud](https://www.serverless.com/framework/docs/providers/tencentcloud)
- [Node.js WebSocket Library (ws)](https://github.com/websockets/ws)
- [Protobuf.js](https://github.com/protobufjs/protobuf.js)

## Support

For issues with:
- **Deployment**: Check Serverless Framework logs
- **WebSocket**: Review Tencent Cloud API Gateway configuration
- **Signaling logic**: Reference original Cloudflare Workers version
