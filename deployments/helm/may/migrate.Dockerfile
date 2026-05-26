FROM rust:slim

RUN cargo install sqlx-cli --no-default-features --features rustls,postgres
COPY may_auth/migrations /migrations

ENTRYPOINT ["sqlx", "migrate", "run", "--source", "/migrations"]
