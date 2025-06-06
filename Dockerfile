FROM rust:1.70.0-slim

# Install required dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libudev-dev \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Solana tools
RUN sh -c "$(curl -sSfL https://release.solana.com/v1.16.13/install)"

# Add Solana to PATH
ENV PATH="/root/.local/share/solana/install/active_release/bin:${PATH}"

# Install Anchor CLI 0.29.0
RUN cargo install anchor-cli --version 0.29.0 --locked

# Set working directory
WORKDIR /app

# Copy the project files
COPY . .

# Remove Cargo.lock to force regeneration by the container's Cargo version
RUN rm -f Cargo.lock

# Build the program
CMD ["cargo", "build"] 