# Stage 1: Build frontend
FROM node:22-alpine AS frontend-build
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# Stage 2: Build Rust binary
FROM ubuntu:24.04 AS rust-build
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl ca-certificates \
    cmake g++ make pkg-config \
    libssl-dev libdlib-dev libatlas-base-dev liblapack-dev \
    && rm -rf /var/lib/apt/lists/*
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app

# Cache dependency build
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
    echo 'fn main() { println!("dummy"); }' > src/main.rs && \
    echo '' > src/lib.rs && \
    cargo build --release && \
    rm -rf src

# Build real binary
COPY src/ src/
RUN touch src/main.rs src/lib.rs && cargo build --release

# Stage 3: Runtime image
FROM ubuntu:24.04
RUN apt-get update && apt-get install -y --no-install-recommends \
    ffmpeg \
    libdlib19.1 \
    libatlas3-base \
    liblapack3 \
    libssl3 \
    libgomp1 \
    ca-certificates \
    curl \
    bzip2 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Download models
RUN mkdir -p models && \
    curl -fsSL -o models/dmhead_nomask_Nx3x224x224.onnx \
    https://github.com/PINTO0309/DMHead/releases/download/1.1.2/dmhead_nomask_Nx3x224x224.onnx && \
    curl -fsSL http://dlib.net/files/shape_predictor_68_face_landmarks.dat.bz2 \
    | bzip2 -d > models/shape_predictor_68_face_landmarks.dat && \
    apt-get purge -y --autoremove curl bzip2

# Copy artifacts from build stages
COPY --from=rust-build /app/target/release/immich-timelapse ./
COPY --from=frontend-build /app/frontend/dist/ ./frontend/dist/

# Create non-root user
RUN useradd --create-home --shell /bin/bash timelapse && \
    mkdir -p output config && \
    chown -R timelapse:timelapse /app

USER timelapse
EXPOSE 5000
CMD ["./immich-timelapse"]
