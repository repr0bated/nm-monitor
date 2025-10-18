#!/bin/bash

# Deployment script for ovs-port-agent
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
DOCKER_IMAGE="ovs-port-agent:latest"
SERVICE_NAME="ovs-port-agent"

# Functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*"
}

check_dependencies() {
    log_info "Checking dependencies..."

    # Check if Docker is available
    if ! command -v docker &> /dev/null; then
        log_error "Docker is required but not installed."
        exit 1
    fi

    # Check if docker-compose is available
    if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
        log_error "Docker Compose is required but not installed."
        exit 1
    fi

    # Check if systemctl is available (for systemd deployment)
    if ! command -v systemctl &> /dev/null; then
        log_warn "systemctl not available - systemd deployment not supported"
    fi

    log_info "Dependencies check completed"
}

build_image() {
    log_info "Building Docker image..."
    docker build -t "$DOCKER_IMAGE" .
    log_info "Docker image built successfully"
}

deploy_docker_compose() {
    log_info "Deploying with Docker Compose..."

    # Stop existing containers
    docker-compose down || true

    # Build and start services
    docker-compose up -d --build

    log_info "Deployment completed with Docker Compose"
}

deploy_systemd() {
    log_info "Deploying with systemd..."

    # Check if systemctl is available
    if ! command -v systemctl &> /dev/null; then
        log_error "systemctl not available - cannot deploy with systemd"
        exit 1
    fi

    # Install systemd services
    sudo cp systemd/* /etc/systemd/system/

    # Reload systemd daemon
    sudo systemctl daemon-reload

    # Enable and start services
    sudo systemctl enable ovs-port-agent.service
    sudo systemctl enable ovsdb-dbus-wrapper.service

    # Stop existing services
    sudo systemctl stop ovs-port-agent.service ovsdb-dbus-wrapper.service || true

    # Start services
    sudo systemctl start ovs-port-agent.service
    sudo systemctl start ovsdb-dbus-wrapper.service

    log_info "Deployment completed with systemd"
}

deploy_kubernetes() {
    log_info "Deploying to Kubernetes..."

    # Check if kubectl is available
    if ! command -v kubectl &> /dev/null; then
        log_error "kubectl is required for Kubernetes deployment but not installed."
        exit 1
    fi

    # Apply Kubernetes manifests
    kubectl apply -f k8s/

    log_info "Kubernetes deployment completed"
}

health_check() {
    log_info "Performing health check..."

    # Wait for services to be ready
    sleep 10

    # Check if systemd services are running (if applicable)
    if command -v systemctl &> /dev/null; then
        if systemctl is-active --quiet ovs-port-agent.service; then
            log_info "OVS Port Agent service is running"
        else
            log_warn "OVS Port Agent service is not running"
        fi
    fi

    # Check if Docker containers are running (if applicable)
    if docker ps | grep -q ovs-port-agent; then
        log_info "OVS Port Agent container is running"
    fi

    log_info "Health check completed"
}

rollback() {
    log_info "Performing rollback..."

    if [ -n "$1" ]; then
        local version="$1"
        log_info "Rolling back to version: $version"

        # Docker Compose rollback
        if command -v docker-compose &> /dev/null; then
            docker-compose down
            # Pull and deploy previous version
            docker pull "ovs-port-agent:$version" || log_warn "Could not pull version $version"
            sed -i "s|image: ovs-port-agent:latest|image: ovs-port-agent:$version|" docker-compose.yml
            docker-compose up -d
        fi

        # systemd rollback would require more complex logic
        log_info "Rollback completed"
    else
        log_error "Rollback requires a version number"
        exit 1
    fi
}

# Main deployment logic
main() {
    local deployment_method="docker-compose"
    local perform_rollback=false
    local rollback_version=""

    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --method)
                deployment_method="$2"
                shift 2
                ;;
            --rollback)
                perform_rollback=true
                rollback_version="$2"
                shift 2
                ;;
            --help)
                echo "Usage: $0 [--method docker-compose|systemd|kubernetes] [--rollback version]"
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                echo "Use --help for usage information"
                exit 1
                ;;
        esac
    done

    if [ "$perform_rollback" = true ]; then
        rollback "$rollback_version"
    else
        check_dependencies
        build_image

        case $deployment_method in
            "docker-compose")
                deploy_docker_compose
                ;;
            "systemd")
                deploy_systemd
                ;;
            "kubernetes")
                deploy_kubernetes
                ;;
            *)
                log_error "Unknown deployment method: $deployment_method"
                echo "Supported methods: docker-compose, systemd, kubernetes"
                exit 1
                ;;
        esac

        health_check
    fi

    log_info "Deployment script completed successfully"
}

# Run main function with all arguments
main "$@"
