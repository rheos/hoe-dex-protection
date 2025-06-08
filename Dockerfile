# Use backpackapp/build:v0.31.0 as the base image
FROM backpackapp/build:v0.31.0

# Set working directory
WORKDIR /app

# Install Node.js 20.18.0 using binary
RUN curl -fsSL https://nodejs.org/dist/v20.18.0/node-v20.18.0-linux-x64.tar.xz | tar -xJ -C /usr/local && \
    ln -s /usr/local/node-v20.18.0-linux-x64/bin/node /usr/local/bin/node && \
    ln -s /usr/local/node-v20.18.0-linux-x64/bin/npm /usr/local/bin/npm && \
    npm install -g yarn

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

