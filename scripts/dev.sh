#!/bin/bash

# Development script for Pinger application

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check if Docker is running
check_docker() {
    if ! docker info > /dev/null 2>&1; then
        print_error "Docker is not running. Please start Docker and try again."
        exit 1
    fi
    print_success "Docker is running"
}

# Function to build the application
build_app() {
    print_status "Building Docker image..."
    docker build -t pinger:latest .
    print_success "Build completed"
}

# Function to start the application
start_app() {
    print_status "Starting application..."
    
    # Check if config exists
    if [ ! -f "config/config.json" ]; then
        print_error "Configuration file not found: config/config.json"
        print_error "Please create a config file or copy from examples."
        exit 1
    fi
    
    # Check if container is already running
    if docker ps -q --filter "name=pinger" | grep -q .; then
        print_warning "Pinger container is already running. Stopping it first..."
        docker stop pinger
        docker rm pinger
    fi
    
    docker run -d --name pinger -p 3000:3000 -v "$(pwd)/config:/etc/pinger:ro" pinger:latest
    print_success "Application started"
    
    # Wait for service to be ready
    print_status "Waiting for service to be ready..."
    sleep 5
    
    # Check service status
    docker ps --filter "name=pinger"
}

# Function to stop the application
stop_app() {
    print_status "Stopping application..."
    docker stop pinger 2>/dev/null || true
    docker rm pinger 2>/dev/null || true
    print_success "Application stopped"
}

# Function to show logs
show_logs() {
    print_status "Showing logs..."
    if docker ps -q --filter "name=pinger" | grep -q .; then
        docker logs -f pinger
    else
        print_error "Pinger container is not running"
    fi
}

# Function to restart the application
restart_app() {
    print_status "Restarting application..."
    stop_app
    start_app
}

# Function to clean up
cleanup() {
    print_status "Cleaning up..."
    docker stop pinger 2>/dev/null || true
    docker rm pinger 2>/dev/null || true
    docker rmi pinger:latest 2>/dev/null || true
    docker system prune -f
    print_success "Cleanup completed"
}

# Function to run tests
run_tests() {
    print_status "Running tests..."
    cargo test
    print_success "Tests completed"
}

# Function to show help
show_help() {
    echo "Usage: $0 [COMMAND]"
    echo ""
    echo "Commands:"
    echo "  build     - Build Docker image"
    echo "  start     - Start the application"
    echo "  stop      - Stop the application"
    echo "  restart   - Restart the application"
    echo "  logs      - Show application logs"
    echo "  test      - Run Rust tests"
    echo "  cleanup   - Clean up containers and images"
    echo "  status    - Show service status"
    echo "  help      - Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 build"
    echo "  $0 start"
    echo "  $0 logs"
    echo ""
    echo "Configuration:"
    echo "  Ensure config/config.json exists before starting the application."
}

# Function to show status
show_status() {
    print_status "Service status:"
    if docker ps -q --filter "name=pinger" | grep -q .; then
        docker ps --filter "name=pinger"
    else
        print_warning "Pinger container is not running"
    fi
}

# Main script logic
main() {
    # Check if Docker is running
    check_docker
    
    case "${1:-help}" in
        build)
            build_app
            ;;
        start)
            start_app
            ;;
        stop)
            stop_app
            ;;
        restart)
            restart_app
            ;;
        logs)
            show_logs
            ;;
        test)
            run_tests
            ;;
        cleanup)
            cleanup
            ;;
        status)
            show_status
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            print_error "Unknown command: $1"
            show_help
            exit 1
            ;;
    esac
}

# Run main function with all arguments
main "$@"
