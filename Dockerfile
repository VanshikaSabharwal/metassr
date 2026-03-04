FROM rust:slim-trixie AS base

# Image descriptor
LABEL copyright.name="Vicente Eduardo Ferrer Garcia" \
	copyright.address="vic798@gmail.com" \
	maintainer.name="Vicente Eduardo Ferrer Garcia" \
	maintainer.address="vic798@gmail.com" \
	vendor="MetaCall Inc." \
	version="0.1"

# Install MetaCall dependencies
RUN apt-get update \
	&& DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
		ca-certificates \
        git \
		wget \
		npm \
		nodejs

# Set working directory to root
WORKDIR /root

# Debug Image
FROM base AS build_debug

#Argument for the name of the app
ARG APP_NAME

# Install MetaCall in debug mode
RUN wget -O - https://raw.githubusercontent.com/metacall/install/master/install.sh | bash -s -- --debug

# Copy the project
COPY . .

ENV RUSTFLAGS="-g"

# Build with debug mode
RUN cargo build

# Copy binary
RUN cp target/debug/metassr /usr/local/bin/metassr

# Application location
WORKDIR /root/${APP_NAME}

# Install packages
RUN npm install

# Build the Application
RUN npm run build:ssr

CMD ["npm", "run", "run"]