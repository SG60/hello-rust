# Copy executable into our base image.
FROM gcr.io/distroless/cc-debian11
ARG RUST_TARGET_DIR=target/release
COPY ${RUST_TARGET_DIR}/hello-rust-backend /
CMD ["/hello-rust-backend"]
