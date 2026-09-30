FROM mcr.microsoft.com/devcontainers/rust:2-1-trixie

ENV DEBIAN_FRONTEND=noninteractive

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

# Pre-create the app's XDG directories with vscode ownership.
# When devcontainer.json mounts named volumes at deeper paths (e.g.
# /home/vscode/.local/share/eu.luxuride.singularity), Docker would otherwise
# create the missing directories as root, breaking tools like fish that need
# to write under $HOME/.local/share. Pre-creating the app subdirectories also
# seeds the named volumes with vscode-owned content on first mount.
RUN mkdir -p \
		/home/vscode/.local/share/eu.luxuride.singularity \
		/home/vscode/.config/eu.luxuride.singularity \
		/home/vscode/.cache/eu.luxuride.singularity \
	&& chown -R vscode:vscode \
		/home/vscode/.local \
		/home/vscode/.config \
		/home/vscode/.cache
