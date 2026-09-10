FROM mcr.microsoft.com/devcontainers/rust:2-1-trixie

ENV DEBIAN_FRONTEND=noninteractive
ENV SINGULARITY_DEV_CONTAINER="true"

RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.7/install.sh | bash \
	&& \. "$HOME/.nvm/nvm.sh" \
	&& nvm install 24 \
	&& ln -s $(dirname $(which node)) /usr/local/bin/node

ENV PATH="/usr/local/bin/node:$PATH"

# Install dependencies for building and running Singularity
RUN apt-get update \
	&& apt-get install -y --no-install-recommends \
		build-essential \
		curl \
		file \
		libayatana-appindicator3-dev \
		libgstreamer1.0-dev \
		libgstreamer-plugins-base1.0-dev \
		libssl-dev \
		libwebkit2gtk-4.1-dev \
		libxdo-dev \
		librsvg2-dev \
		gstreamer1.0-libav \
		gstreamer1.0-plugins-bad \
		gstreamer1.0-plugins-base \
		gstreamer1.0-plugins-good \
		gstreamer1.0-plugins-ugly \
		gstreamer1.0-tools \
	&& apt-get clean \
	&& rm -rf /var/lib/apt/lists/*
