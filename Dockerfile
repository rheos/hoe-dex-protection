# Use backpackapp/build:v0.30.1 as the base image
FROM backpackapp/build:v0.30.1

# Set working directory
WORKDIR /app

# Install Anchor CLI 0.31.1
RUN cargo install --git https://github.com/coral-xyz/anchor --tag v0.31.1 anchor-cli --locked

# Update PATH
ENV PATH="/root/.cargo/bin:$PATH"

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