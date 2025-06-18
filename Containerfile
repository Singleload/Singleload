# Build stage - Install all runtimes with minimal dependencies
FROM debian:12-slim AS builder

# Install essential build tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    wget \
    gnupg \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Create directories for installations
RUN mkdir -p /opt/runtimes/bin /opt/runtimes/lib

# Install Python 3.13
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3.11 \
    python3.11-minimal \
    && rm -rf /var/lib/apt/lists/* \
    && cp /usr/bin/python3.11 /opt/runtimes/bin/python3 \
    && ldd /usr/bin/python3.11 | grep "=>" | awk '{print $3}' | xargs -I {} cp {} /opt/runtimes/lib/ || true

# Install Node.js 22 LTS
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/* \
    && cp /usr/bin/node /opt/runtimes/bin/ \
    && ldd /usr/bin/node | grep "=>" | awk '{print $3}' | xargs -I {} cp {} /opt/runtimes/lib/ || true

# Install PHP 8.3 (more stable than 8.4 for production)
RUN apt-get update && apt-get install -y --no-install-recommends \
    php8.2-cli \
    && rm -rf /var/lib/apt/lists/* \
    && cp /usr/bin/php8.2 /opt/runtimes/bin/php \
    && ldd /usr/bin/php8.2 | grep "=>" | awk '{print $3}' | xargs -I {} cp {} /opt/runtimes/lib/ || true

# Install Go 1.23 (latest stable)
RUN wget -q https://go.dev/dl/go1.23.0.linux-amd64.tar.gz \
    && tar -C /opt/runtimes -xzf go1.23.0.linux-amd64.tar.gz \
    && rm go1.23.0.linux-amd64.tar.gz \
    && mv /opt/runtimes/go/bin/go /opt/runtimes/bin/

# Install .NET 8 LTS runtime
RUN wget -q https://packages.microsoft.com/config/debian/12/packages-microsoft-prod.deb \
    && dpkg -i packages-microsoft-prod.deb \
    && rm packages-microsoft-prod.deb \
    && apt-get update && apt-get install -y --no-install-recommends \
    dotnet-runtime-8.0 \
    && rm -rf /var/lib/apt/lists/* \
    && cp -r /usr/share/dotnet /opt/runtimes/

# Install Rust (for running Rust scripts)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal \
    && /root/.cargo/bin/rustup default stable \
    && cp /root/.cargo/bin/rustc /opt/runtimes/bin/ \
    && cp /root/.cargo/bin/cargo /opt/runtimes/bin/

# Copy bash from base system
RUN cp /bin/bash /opt/runtimes/bin/ \
    && ldd /bin/bash | grep "=>" | awk '{print $3}' | xargs -I {} cp {} /opt/runtimes/lib/ || true

# Create script runner wrapper
RUN echo '#!/bin/bash\nset -euo pipefail\nexec "$@"' > /opt/runtimes/bin/runner \
    && chmod +x /opt/runtimes/bin/runner

# Production stage - Distroless base with only necessary files
FROM gcr.io/distroless/base-debian12:nonroot

# Copy runtime binaries and libraries
COPY --from=builder /opt/runtimes/bin /usr/local/bin
COPY --from=builder /opt/runtimes/lib /usr/local/lib
COPY --from=builder /opt/runtimes/dotnet /usr/share/dotnet

# Copy essential system libraries that might be needed
COPY --from=builder /lib/x86_64-linux-gnu/libc.so.6 /lib/x86_64-linux-gnu/
COPY --from=builder /lib/x86_64-linux-gnu/libm.so.6 /lib/x86_64-linux-gnu/
COPY --from=builder /lib/x86_64-linux-gnu/libpthread.so.0 /lib/x86_64-linux-gnu/
COPY --from=builder /lib/x86_64-linux-gnu/libdl.so.2 /lib/x86_64-linux-gnu/
COPY --from=builder /lib/x86_64-linux-gnu/librt.so.1 /lib/x86_64-linux-gnu/
COPY --from=builder /lib/x86_64-linux-gnu/libgcc_s.so.1 /lib/x86_64-linux-gnu/
COPY --from=builder /lib/x86_64-linux-gnu/libstdc++.so.6 /lib/x86_64-linux-gnu/
COPY --from=builder /lib64/ld-linux-x86-64.so.2 /lib64/

# Set up environment
ENV PATH=/usr/local/bin:$PATH
ENV LD_LIBRARY_PATH=/usr/local/lib:/lib/x86_64-linux-gnu:/lib64
ENV DOTNET_ROOT=/usr/share/dotnet
ENV DOTNET_CLI_TELEMETRY_OPTOUT=1

# Create workspace directory
WORKDIR /workspace

# Run as nonroot user (UID 65532)
USER nonroot:nonroot

# Set secure defaults
LABEL singleload.version="0.1.0" \
      singleload.security="rootless,distroless,no-new-privileges" \
      singleload.runtimes="python3.11,node22,php8.2,go1.23,dotnet8,rust1.87,bash5.2"