FROM gcr.io/distroless/static-debian13:nonroot@sha256:e2e927ec666bae08560abb3c55d0659eceabb657f56b6782ab500a9fc7f555e3
ARG TARGETARCH
COPY --chmod=0755 ${TARGETARCH}/geoip-builder /usr/local/bin/geoip-builder
COPY --chown=65532:65532 work /work
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/geoip-builder"]
