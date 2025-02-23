FROM rust:latest

WORKDIR /app
COPY . .

RUN cargo build --release
EXPOSE 3000

CMD ["cargo", "run"]
# CMD ["./target/release/backend"]