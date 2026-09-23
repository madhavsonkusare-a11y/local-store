ARG BASE_IMAGE
FROM ${BASE_IMAGE}

# Development payload only. Ubuntu's signed apt repositories resolve the OS
# dependencies; the exported inventory records their versions. Engine packages
# are downloaded and SHA-256 checked by build-engine-payload.py before this COPY.
COPY packages/ /tmp/local-store-packages/
COPY packages.lock.tsv /usr/share/local-store/packages.lock.tsv
RUN printf '#!/bin/sh\nexit 101\n' > /usr/sbin/policy-rc.d \
    && chmod 755 /usr/sbin/policy-rc.d \
    && apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
       ca-certificates systemd systemd-sysv iproute2 procps \
       /tmp/local-store-packages/*.deb \
    && mkdir -p /usr/share/local-store /etc/docker \
    && dpkg-query -W -f='${Package}\t${Version}\t${Architecture}\n' \
       > /usr/share/local-store/packages.tsv \
    && cmp /usr/share/local-store/packages.lock.tsv /usr/share/local-store/packages.tsv \
    && rm -rf /tmp/local-store-packages /var/lib/apt/lists/* \
    && rm /usr/sbin/policy-rc.d
COPY wsl.conf /etc/wsl.conf
COPY daemon.json /etc/docker/daemon.json
COPY components.lock.json /usr/share/local-store/components.lock.json

# No TCP Docker API. The future broker runs the CLI inside the owned distro.
# Do not start this image privileged inside another daemon.
CMD ["/bin/true"]
