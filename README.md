

A real-time analytics system that tracks and visualizes the most popular domains shared on Hacker News.

## Overview

This project fetches the latest stories from Hacker News, processes them through a Kafka message stream, aggregates domain statistics, and displays the results in an interactive visualization.

## Features

- Real-time domain popularity tracking
- Kafka-based data processing pipeline
- RESTful API for domain statistics
- Responsive web interface

## Architecture

The system consists of:

- **Backend**: Rust service that fetches from HackerNews API and processes data through Kafka
- **Frontend**: React application with D3.js visualizations
- **Kafka**: Message broker for reliable data streaming
- **Docker**: Containerization for easy deployment

## Getting Started

### Prerequisites

- Docker and Docker Compose

### Running the Application

```bash
# Frontend stack 

docker-compose -f docker-compose-fe.yml up --build

# Backend stack 
docker-compose -f docker-compose-kafka.yml up -d
```

This command starts all necessary services:
- Kafka and Zookeeper
- Backend processor
- Frontend web application



## Backend API Endpoints

- `/api/top-domains` - Returns the most referenced domains and their counts
- `/api/healthcheck` - Service health endpoint

## Development

To run individual components in development mode:



## License

MIT
