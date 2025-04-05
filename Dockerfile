FROM rust:1.86-alpine AS builder
WORKDIR /app
COPY . .
RUN apk add g++ libressl-dev && cargo build --release

FROM scratch
COPY --from=builder /app/target/release/gitlab_pipeline .
ENV DOCKER=true
ENTRYPOINT ["/gitlab_pipeline"]
