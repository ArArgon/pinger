# Pinger - Docker Setup

This document describes how to run the Pinger application using Docker.

## Prerequisites

- Docker Engine 20.10+
- Make (optional, for using the Makefile)

## Quick Start

### Using Make Commands (Recommended)

1. **Build and run:**
   ```bash
   make build
   make run
   ```

2. **Check status:**
   ```bash
   make status
   ```

3. **View logs:**
   ```bash
   make logs
   ```

4. **Stop service:**
   ```bash
   make stop
   ```

### Using Docker directly

1. **Build the image:**
   ```bash
   make docker-build
   # or
   docker build -t pinger:latest .
   ```

2. **Run the container:**
   ```bash
   make docker-run
   # or
   docker run -d --name pinger -p 3000:3000 -v $(pwd)/config:/etc/pinger:ro pinger:latest
   ```

3. **Stop the container:**
   ```bash
   make docker-stop
   # or
   docker stop pinger
   ```

## Services

The basic setup runs only the **Pinger** application (port 3000).

For full monitoring capabilities, see the [Monitoring Setup](#monitoring-setup) section below.

## Configuration

### Environment Variables

- `RUST_LOG`: Log level (default: `info`)

### Command Line Arguments

- `--bind`: Metrics server bind address (default: `0.0.0.0`)
- `--port`: Metrics server port (default: `3000`)

### Volume Mounts

- `./config:/etc/pinger:ro`: Configuration directory

### Configuration Management

The application expects a configuration file at `/etc/pinger/config.json` inside the container. You can:

1. **Mount a config directory** (recommended):
   ```bash
   docker run -v $(pwd)/config:/etc/pinger:ro pinger:latest
   ```

2. **Use a custom config path**:
   ```bash
   docker run pinger:latest --config /path/to/your/config.json
   ```

3. **Override bind address and port**:
   ```bash
   docker run pinger:latest --bind 127.0.0.1 --port 8080 --config /etc/pinger/config.json
   ```

4. **Simple config**: Just ensure `config/config.json` exists - the file is self-documenting with examples.

### Ports

- **3000**: Pinger metrics endpoint

## Monitoring

### Metrics Endpoint

Access the pinger metrics at:
```
http://localhost:3000/metrics
```

## Monitoring Setup

For advanced monitoring with Prometheus and Grafana, you can use the example files in the `examples/` directory:

### Option 1: Use Example Files
```bash
# Copy example files to your project
cp examples/docker-compose.yml .
cp examples/prometheus.yml .

# Start the full monitoring stack
docker-compose up -d
```

### Option 2: Manual Setup

#### Prometheus
Create `prometheus.yml`:
```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'pinger'
    static_configs:
      - targets: ['pinger:3000']
    metrics_path: '/metrics'
    scrape_interval: 5s
    scrape_timeout: 3s
```

Run Prometheus:
```bash
docker run -d --name prometheus \
  -p 9090:9090 \
  -v $(pwd)/prometheus.yml:/etc/prometheus/prometheus.yml:ro \
  prom/prometheus:latest
```

#### Grafana
```bash
docker run -d --name grafana \
  -p 3001:3000 \
  -e GF_SECURITY_ADMIN_PASSWORD=admin \
  grafana/grafana:latest
```

Default Grafana credentials:
- Username: `admin`
- Password: `admin`

## Health Checks

The pinger container includes a health check that verifies the metrics endpoint is responding:

```bash
docker inspect pinger | grep Health -A 10
```

## Development

### Interactive Shell

Access a shell in the running container:
```bash
make shell
# or
docker exec -it pinger sh
```

### Rebuilding

After code changes:
```bash
make build
make run
```

### Testing

Run tests locally:
```bash
make test
# or
cargo test
```

## Production Deployment

### Multi-platform Build

Build for multiple architectures:
```bash
make release-build
# or
docker buildx build --platform linux/amd64,linux/arm64 -t pinger:latest --push .
```

### Security Scanning

The GitHub Actions workflow includes Trivy vulnerability scanning for security.

### Resource Limits

For production, consider adding resource limits:

```bash
docker run -d --name pinger \
  -p 3000:3000 \
  -v $(pwd)/config:/etc/pinger:ro \
  --memory=512m \
  --cpus=0.5 \
  pinger:latest
```

## Troubleshooting

### Common Issues

1. **Port already in use:**
   ```bash
   # Check what's using the port
   lsof -i :3000
   
   # Stop conflicting services
   make stop
   ```

2. **Permission denied:**
   ```bash
   # Fix file permissions
   sudo chown -R $USER:$USER .
   ```

3. **Container won't start:**
   ```bash
   # Check logs
   make logs
   
   # Check container status
   make status
   ```

### Logs

View application logs:
```bash
# Pinger logs
make logs

# Follow logs
docker logs -f pinger
```

### Cleanup

Remove all containers and images:
```bash
make clean
# or
docker stop pinger || true
docker rm pinger || true
docker rmi pinger:latest || true
docker system prune -f
```

## GitHub Actions

The repository includes GitHub Actions workflows that:

- Build Docker images on push to main/develop branches
- Build and push images on tag creation
- Run security scans with Trivy
- Support multi-platform builds (AMD64, ARM64)
- Cache Docker layers for faster builds

### Manual Trigger

You can manually trigger the workflow:
1. Go to Actions tab in GitHub
2. Select "Build and Push Docker Image"
3. Click "Run workflow"

## Examples

The `examples/` directory contains additional setup files for advanced use cases:

- `docker-compose.yml` - Full monitoring stack with Prometheus and Grafana
- `prometheus.yml` - Prometheus configuration for metrics collection
- `docker-run-examples.md` - Comprehensive Docker run command examples

## Contributing

When contributing:

1. Test your changes locally with Docker
2. Ensure the container builds successfully
3. Verify the application runs correctly
4. Check that metrics are accessible

## License

This project is licensed under the same terms as the main Pinger application.
