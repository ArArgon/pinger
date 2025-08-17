# Docker Run Examples

This document provides examples of how to run the pinger application with different configurations.

## Basic Usage

### Run with default config
```bash
docker run -d --name pinger \
  -p 3000:3000 \
  -v $(pwd)/config:/etc/pinger:ro \
  pinger:latest
```

### Run with custom config path
```bash
docker run -d --name pinger \
  -p 3000:3000 \
  -v $(pwd)/my-config:/etc/pinger:ro \
  pinger:latest --config /etc/pinger/my-config.json
```

## Custom Bind Address and Port

### Bind to specific interface
```bash
docker run -d --name pinger \
  -p 127.0.0.1:3000:3000 \
  -v $(pwd)/config:/etc/pinger:ro \
  pinger:latest --bind 127.0.0.1
```

### Use custom port
```bash
docker run -d --name pinger \
  -p 8080:8080 \
  -v $(pwd)/config:/etc/pinger:ro \
  pinger:latest --port 8080
```

### Custom bind address and port
```bash
docker run -d --name pinger \
  -p 127.0.0.1:8080:8080 \
  -v $(pwd)/config:/etc/pinger:ro \
  pinger:latest --bind 127.0.0.1 --port 8080
```

## Environment Variables

### Set log level
```bash
docker run -d --name pinger \
  -p 3000:3000 \
  -v $(pwd)/config:/etc/pinger:ro \
  -e RUST_LOG=debug \
  pinger:latest
```

### Multiple environment variables
```bash
docker run -d --name pinger \
  -p 3000:3000 \
  -v $(pwd)/config:/etc/pinger:ro \
  -e RUST_LOG=info \
  -e TZ=UTC \
  pinger:latest
```

## Network Configuration

### Use host networking
```bash
docker run -d --name pinger \
  --network host \
  -v $(pwd)/config:/etc/pinger:ro \
  pinger:latest
```

### Custom network
```bash
# Create network
docker network create pinger-net

# Run container
docker run -d --name pinger \
  --network pinger-net \
  -p 3000:3000 \
  -v $(pwd)/config:/etc/pinger:ro \
  pinger:latest
```

## Resource Limits

### Memory and CPU limits
```bash
docker run -d --name pinger \
  -p 3000:3000 \
  -v $(pwd)/config:/etc/pinger:ro \
  --memory=512m \
  --cpus=0.5 \
  pinger:latest
```

### Memory reservation
```bash
docker run -d --name pinger \
  -p 3000:3000 \
  -v $(pwd)/config:/etc/pinger:ro \
  --memory=1g \
  --memory-reservation=512m \
  pinger:latest
```

## Health Check Override

### Custom health check
```bash
docker run -d --name pinger \
  -p 3000:3000 \
  -v $(pwd)/config:/etc/pinger:ro \
  --health-cmd="curl -f http://localhost:3000/metrics" \
  --health-interval=1m \
  --health-timeout=10s \
  --health-retries=5 \
  pinger:latest
```

## Debug Mode

### Enable debug logging
```bash
docker run -d --name pinger \
  -p 3000:3000 \
  -v $(pwd)/config:/etc/pinger:ro \
  pinger:latest --debug
```

## Production Deployment

### With restart policy
```bash
docker run -d --name pinger \
  -p 3000:3000 \
  -v $(pwd)/config:/etc/pinger:ro \
  --restart=unless-stopped \
  --memory=1g \
  --cpus=1.0 \
  pinger:latest
```

### With logging driver
```bash
docker run -d --name pinger \
  -p 3000:3000 \
  -v $(pwd)/config:/etc/pinger:ro \
  --log-driver=json-file \
  --log-opt max-size=10m \
  --log-opt max-file=3 \
  pinger:latest
```

## Troubleshooting

### Run in foreground for debugging
```bash
docker run --rm --name pinger \
  -p 3000:3000 \
  -v $(pwd)/config:/etc/pinger:ro \
  pinger:latest --debug
```

### Check container logs
```bash
docker logs pinger
docker logs -f pinger  # Follow logs
```

### Execute commands in container
```bash
docker exec -it pinger sh
docker exec pinger cat /etc/pinger/config.json
```
