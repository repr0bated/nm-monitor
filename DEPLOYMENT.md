# Deployment Guide for OVS Port Agent

This guide covers multiple deployment strategies for the OVS Port Agent network monitoring system.

## Quick Start

### Using Docker Compose (Recommended for Development)

```bash
# Build and deploy with Docker Compose
./deploy.sh --method docker-compose

# Or manually
docker-compose up -d --build
```

### Using Systemd (Recommended for Production)

```bash
# Deploy with systemd (requires root)
sudo ./deploy.sh --method systemd
```

### Using Kubernetes

```bash
# Deploy to Kubernetes cluster
./deploy.sh --method kubernetes
```

## Deployment Methods

### 1. Docker Compose Deployment

**Best for**: Development, testing, and single-node deployments

**Features**:
- Multi-container setup with OVS components
- Easy scaling and management
- Built-in networking and volume management

**Services Included**:
- `ovs-port-agent`: Main application container
- `ovsdb`: OVSDB server container
- `ovs-vswitchd`: OVS vswitch daemon container

**Usage**:
```bash
# Start services
docker-compose up -d

# View logs
docker-compose logs -f ovs-port-agent

# Stop services
docker-compose down

# Update deployment
docker-compose pull && docker-compose up -d
```

### 2. Systemd Deployment

**Best for**: Production environments, bare metal servers

**Features**:
- Native systemd integration
- System-level service management
- Proper privilege escalation for network management

**Services**:
- `ovs-port-agent.service`: Main application service
- `ovsdb-dbus-wrapper.service`: D-Bus wrapper service

**Installation**:
```bash
# Copy service files
sudo cp systemd/* /etc/systemd/system/

# Enable and start services
sudo systemctl enable ovs-port-agent.service
sudo systemctl start ovs-port-agent.service

# Check status
sudo systemctl status ovs-port-agent.service
```

### 3. Kubernetes Deployment

**Best for**: Cloud-native, scalable deployments

**Features**:
- Horizontal pod autoscaling support
- Service discovery and load balancing
- Persistent storage and configuration management

**Components**:
- Deployment with 3 replicas for high availability
- Service for internal communication
- ConfigMaps for configuration management
- PersistentVolumes for data persistence

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Log level | `info` |
| `CONFIG_PATH` | Configuration file path | `/etc/ovs-port-agent/config.toml` |
| `OVS_BRIDGE` | Default OVS bridge name | `ovsbr0` |

### Configuration Files

1. **Docker Compose**: Edit `docker-compose.yml` for container-specific settings
2. **Systemd**: Modify service files in `systemd/` directory
3. **Kubernetes**: Update `k8s/deployment.yaml` for cluster settings

## Monitoring

### Health Checks

The deployment includes built-in health checks:

- **Liveness Probe**: Application responsiveness
- **Readiness Probe**: Service availability
- **Startup Probe**: Initial application startup

### Logging

- **Docker**: Container logs accessible via `docker-compose logs`
- **Systemd**: Journal logs via `journalctl -u ovs-port-agent`
- **Kubernetes**: Pod logs via `kubectl logs`

### Metrics

Prometheus metrics are available at:
- **Docker/Systemd**: `http://localhost:8080/metrics`
- **Kubernetes**: `http://ovs-port-agent-service:8080/metrics`

## Scaling

### Docker Compose
```bash
# Scale services
docker-compose up -d --scale ovs-port-agent=3
```

### Kubernetes
```bash
# Scale deployment
kubectl scale deployment ovs-port-agent --replicas=5
```

## Troubleshooting

### Common Issues

1. **OVS Not Available**
   ```bash
   # Check OVS status
   sudo systemctl status openvswitch-switch.service

   # Restart OVS
   sudo systemctl restart openvswitch-switch.service
   ```

2. **D-Bus Connection Issues**
   ```bash
   # Check D-Bus
   systemctl status dbus

   # Restart D-Bus
   systemctl restart dbus
   ```

3. **Network Interface Issues**
   ```bash
   # Check network interfaces
   ip addr show

   # Check OVS bridges
   ovs-vsctl show
   ```

### Logs and Debugging

```bash
# Docker Compose logs
docker-compose logs -f ovs-port-agent

# Systemd logs
sudo journalctl -u ovs-port-agent -f

# Kubernetes logs
kubectl logs -f deployment/ovs-port-agent
```

## Security Considerations

1. **Network Privileges**: The application requires `CAP_NET_ADMIN` for network management
2. **Host Network Access**: Required for direct network interface management
3. **Configuration Security**: Store sensitive configuration in secure locations
4. **Access Control**: Limit D-Bus access to authorized users only

## Rollback

### Docker Compose
```bash
# Rollback to specific version
./deploy.sh --rollback v1.0.0
```

### Kubernetes
```bash
# Rollback deployment
kubectl rollout undo deployment/ovs-port-agent
```

## Performance Tuning

### Memory Limits
- **Docker**: Set memory limits in `docker-compose.yml`
- **Kubernetes**: Configure resource requests/limits in deployment

### CPU Limits
- Monitor CPU usage and adjust resource allocation
- Consider horizontal scaling for high-load scenarios

### Network Performance
- Ensure adequate network bandwidth for monitoring traffic
- Monitor OVS flow performance in high-throughput environments

## Backup and Recovery

### Data Backup
- **OVS Database**: `/var/lib/openvswitch/conf.db`
- **Configuration**: `/etc/ovs-port-agent/`
- **Logs**: Journal logs and application logs

### Recovery Procedures
1. Restore OVS database backup
2. Reapply configuration files
3. Restart services in dependency order
4. Verify network connectivity and bridge configuration

## Support

For issues and questions:
1. Check the troubleshooting section
2. Review application and system logs
3. Verify network and OVS configuration
4. Check GitHub issues for known problems
