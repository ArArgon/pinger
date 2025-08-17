.PHONY: help build run stop clean logs shell test docker-build docker-run docker-stop docker-clean

# Default target
help:
	@echo "Available commands:"
	@echo "  build        - Build the Docker image locally"
	@echo "  run          - Run the application using Docker"
	@echo "  stop         - Stop the application"
	@echo "  clean        - Remove all containers and images"
	@echo "  logs         - Show logs from the application"
	@echo "  shell        - Open shell in running pinger container"
	@echo "  test         - Run tests"
	@echo "  docker-build - Build Docker image with specific tag"
	@echo "  docker-run   - Run Docker container directly"
	@echo "  docker-stop  - Stop Docker container"
	@echo "  docker-clean - Clean Docker container and image"

# Docker commands
build:
	@echo "Building Docker image..."
	docker build -t pinger:latest .

run:
	@echo "Starting pinger container..."
	docker run -d --name pinger -p 3000:3000 -v $(PWD)/config:/etc/pinger:ro pinger:latest

stop:
	@echo "Stopping pinger container..."
	docker stop pinger || true
	docker rm pinger || true

clean:
	@echo "Cleaning up containers and images..."
	docker stop pinger || true
	docker rm pinger || true
	docker rmi pinger:latest || true
	docker system prune -f

logs:
	@echo "Showing pinger logs..."
	docker logs -f pinger

shell:
	@echo "Opening shell in pinger container..."
	docker exec -it pinger sh

# Direct Docker commands
docker-build:
	@read -p "Enter tag (default: latest): " tag; \
	tag=$${tag:-latest}; \
	docker build -t pinger:$$tag .

docker-run:
	@read -p "Enter tag (default: latest): " tag; \
	tag=$${tag:-latest}; \
	docker run -d --name pinger-$$tag -p 3000:3000 -v $(PWD)/config:/etc/pinger:ro pinger:$$tag

docker-stop:
	@docker ps -q --filter "name=pinger-" | xargs -r docker stop

docker-clean:
	@docker ps -a -q --filter "name=pinger-" | xargs -r docker rm; \
	docker images -q pinger | xargs -r docker rmi

# Development commands
test:
	cargo test

# Release commands
release-build:
	docker buildx build --platform linux/amd64,linux/arm64 -t pinger:latest --push .

# Utility commands
status:
	docker ps --filter "name=pinger"

restart:
	@echo "Restarting pinger container..."
	$(MAKE) stop
	$(MAKE) run

# Show container resource usage
stats:
	docker stats --no-stream
