#!/bin/bash

# Stop all running containers
echo "Stopping all Docker containers..."
docker stop $(docker ps -q) || true

# Remove all containers
echo "Removing all Docker containers..."
docker rm $(docker ps -a -q) || true

echo "All Docker containers have been stopped and removed."