FROM gcr.io/distroless/static-debian13:nonroot@sha256:18f4cc26c9ffa7f20a359298be7deadac5c607c8e03a02b8119f1a5fc2f82c99
ARG TARGETARCH
COPY --chmod=0755 ${TARGETARCH}/geoip-builder /usr/local/bin/geoip-builder
COPY --chown=65532:65532 work /work
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/geoip-builder"]
