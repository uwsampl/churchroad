# syntax=docker/dockerfile-upstream:master-labs
# The above enables use of ADD of git repo.

FROM ubuntu:22.04
ARG MAKE_JOBS=2
SHELL ["/bin/bash", "-c"] 

# Update, get add-apt-repository.
RUN apt update \
  &&  apt install -y software-properties-common

# Install apt dependencies
# `noninteractive` prevents the tzdata package from asking for a timezone on the
# command line.
ENV DEBIAN_FRONTEND=noninteractive
RUN apt install -y \
  autoconf \
  bison \
  ccache \
  cmake \
  curl \
  flex \
  g++ \
  git \
  jq \
  libboost-filesystem-dev \
  libfl-dev \
  libfl2 \
  libgmp-dev \
  libgoogle-perftools-dev \
  libreadline-dev \
  libssl-dev \
  libzmq3-dev \
  llvm-14 \
  make \
  ninja-build \
  numactl \
  openssl \
  perl \
  perl-doc \
  pkg-config \
  python3 \
  python3-pip \
  racket \
  tcl \
  tcl8.6-dev \
  wget \
  zlib1g \
  zlib1g-dev

# Install Rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:$PATH"

# Build Rust package.
WORKDIR /root/churchroad
# ADD has weird behavior when it comes to directories. That's why we need so
# many ADDs.
ADD egglog_src egglog_src
ADD src src
ADD tests tests
ADD Cargo.toml Cargo.toml
ADD Cargo.lock Cargo.lock
RUN cargo build

# Build Yosys.
WORKDIR /root
ARG MAKE_JOBS=2
ADD dependencies.sh .
RUN source /root/dependencies.sh \
  && mkdir yosys && cd yosys \
  && wget -qO- https://github.com/YosysHQ/yosys/archive/$YOSYS_COMMIT_HASH.tar.gz | tar xz --strip-components=1 \
  && PREFIX="/root/.local" CPLUS_INCLUDE_PATH="/usr/include/tcl8.6/:$CPLUS_INCLUDE_PATH" make config-gcc \
  && PREFIX="/root/.local" CPLUS_INCLUDE_PATH="/usr/include/tcl8.6/:$CPLUS_INCLUDE_PATH" make -j ${MAKE_JOBS} install \
  && rm -rf /root/yosys

# Add /root/.local/bin to PATH.
ENV PATH="/root/.local/bin:$PATH"

# Add the synlig plugin
RUN curl https://api.github.com/repos/chipsalliance/synlig/releases/latest \
  | jq -r '.assets | .[] | select(.name | startswith("synlig-plugin-debian")) | .browser_download_url' \
  | xargs wget -O - \
  | tar -xz \
  && ./install_plugin.sh

# Build Yosys plugin.
WORKDIR /root/churchroad/yosys-plugin
ADD yosys-plugin .
RUN make -j ${MAKE_JOBS}

# Make a binary for `lit`. If you're on Mac, you can install lit via Brew.
# Ubuntu doesn't have a binary for it, but it is available on pip and is
# installed later in this Dockerfile.
WORKDIR /root
RUN mkdir -p /root/.local/bin \
  && echo "#!/usr/bin/env python3" >> /root/.local/bin/lit \
  && echo "from lit.main import main" >> /root/.local/bin/lit \
  && echo "if __name__ == '__main__': main()" >> /root/.local/bin/lit \
  && chmod +x /root/.local/bin/lit

# Point to llvm-config binary. Alternatively, make sure you have a binary called
# `llvm-config` on your PATH.
ENV LLVM_CONFIG=llvm-config-14

# Python dependencies.
WORKDIR /root/churchroad
ADD requirements.txt .
RUN pip install -r requirements.txt

ENV CHURCHROAD_DIR=/root/churchroad

# Add other Churchroad files. It's useful to put this as far down as possible.
# In general, only ADD files just before they're needed. This maximizes the
# ability to cache intermediate containers and minimizes rebuilding.
#
# We use the git functionality of ADD (enabled in our case via the optional flag
# --keep-git-dir) to add all of the checked-in files of the Churchroad repo (but
# not including the .git directory itself). We could cut this down further if we
# wanted, but I think this is a clean approach for now.
WORKDIR /root/churchroad
ADD --keep-git-dir=false . .

WORKDIR /root/churchroad
ADD fmt.sh run-tests.sh ./
CMD [ "/bin/bash", "run-tests.sh" ]
