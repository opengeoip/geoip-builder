FROM gcr.io/distroless/static-debian13:nonroot@sha256:aafe993665bf6200db38a02c11a9351e782a84b6a45e7f33b6695638c88e3931
ARG TARGETARCH
COPY --chmod=0755 ${TARGETARCH}/geoip-builder /usr/local/bin/geoip-builder
COPY --chown=65532:65532 work /work
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/geoip-builder"]
