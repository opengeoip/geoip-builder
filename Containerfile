FROM gcr.io/distroless/static-debian13:nonroot@sha256:8ce6dd87e9823c5b48dcfb2c8604714ccb351ce59607327bcc4957448da279b5
ARG TARGETARCH
COPY --chmod=0755 ${TARGETARCH}/geoip-builder /usr/local/bin/geoip-builder
COPY --chown=65532:65532 work /work
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/geoip-builder"]
