FROM gcr.io/distroless/static-debian13:nonroot@sha256:25ac8f0c3b2334a9eb05452266bd4697864a8c8ef0ef71e5e6480d7f92868329
ARG TARGETARCH
COPY --chmod=0755 ${TARGETARCH}/geoip-builder /usr/local/bin/geoip-builder
COPY --chown=65532:65532 work /work
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/geoip-builder"]
