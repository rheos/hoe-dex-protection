# Use backpackapp/build:v0.31.0 as the base image
FROM backpackapp/build:v0.31.0

# Set working directory
WORKDIR /app

# Activate solana toolchain
RUN rustup default solana

# Copy project files
COPY . .

# Install Yarn dependencies
RUN yarn install

# Build the Anchor project
RUN anchor build

# Expose ports (optional, for localnet)
EXPOSE 8899

# Command to keep container running
CMD ["bash"]