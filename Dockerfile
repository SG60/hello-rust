# Copy executable into our base image.
FROM gcr.io/distroless/cc-debian11
COPY target/aarch64-unknown-linux-gnu/release/hello-rust /
CMD ["/hello-rust"]
