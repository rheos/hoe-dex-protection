# Use backpackapp/build:v0.31.1 as the base image
FROM backpackapp/build:v0.31.1

# Set working directory
WORKDIR /app

# Ensure Rust has bpfel-unknown-unknown target
RUN rustup target add bpfel-unknown-unknown

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