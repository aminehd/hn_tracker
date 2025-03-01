FROM rust:latest

# Install build dependencies
RUN apt-get update && \
    apt-get install -y \
    cmake \
    pkg-config \
    libssl-dev \
    build-essential \
    librdkafka-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

RUN cargo build --release

EXPOSE 3000

CMD ["cargo", "run", "--release"]
