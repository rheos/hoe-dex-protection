FROM backpackapp/build:v0.31.0

# Set working directory to the project root
WORKDIR /app

# Copy the project files
COPY . .

# Remove Cargo.lock to force regeneration by the container's Cargo version
RUN rm -f Cargo.lock

# Build the program using anchor build
CMD ["anchor", "build"] 