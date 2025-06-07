# Use backpackapp/build:v0.31.0 as the base image
FROM backpackapp/build:v0.31.0

# Set working directory
WORKDIR /app

# Update Node.js to version 20.18.0
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - && \
    apt-get install -y nodejs=20.18.0-1nodesource1 && \
    npm install -g yarn

# Copy Solana development wallet
COPY ~/.config/solana/dev-wallet.json /root/.config/solana/dev-wallet.json

# Copy project files
COPY . .

# Install Yarn dependencies
RUN yarn install

# Build the Anchor project
RUN anchor build

# Expose ports (optional, for localnet)
EXPOSE 8899

# Command to keep container running or run tests
CMD ["bash"]