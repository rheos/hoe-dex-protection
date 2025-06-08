# Use backpackapp/build:v0.31.0 as the base image
FROM backpackapp/build:v0.31.0

# Set working directory
WORKDIR /app

# Remove existing Node.js and nvm
RUN rm -rf /root/.nvm /usr/bin/node /usr/bin/npm /usr/local/bin/node /usr/local/bin/npm

# Install Node.js 20.18.0 using binary
RUN curl -fsSL https://nodejs.org/dist/v20.18.0/node-v20.18.0-linux-x64.tar.xz | tar -xJ -C /usr/local && \
    ln -s /usr/local/node-v20.18.0-linux-x64/bin/node /usr/local/bin/node && \
    ln -s /usr/local/node-v20.18.0-linux-x64/bin/npm /usr/local/bin/npm && \
    /usr/local/bin/npm install -g yarn

# Update PATH
ENV PATH="/usr/local/node-v20.18.0-linux-x64/bin:/usr/local/bin:$PATH"

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