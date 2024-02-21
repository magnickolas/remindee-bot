FROM rust:1 as build-env
WORKDIR /app
COPY . /app
RUN cargo build --release


FROM gcr.io/distroless/cc-debian12
ENV REMINDEE_DB=/data/remindee.db
VOLUME ["/data"]

COPY --from=build-env /app/target/release/remindee-bot /
CMD ["./remindee-bot"]
