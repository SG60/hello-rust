# Copy executable into our base image.
ARG BASE_IMAGE=gcr.io/distroless/cc-debian11
FROM ${BASE_IMAGE}
ARG RUST_TARGET_DIR=target/release
COPY ${RUST_TARGET_DIR}/hello-rust-backend /
CMD ["/hello-rust-backend"]
