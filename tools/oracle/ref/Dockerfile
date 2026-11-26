FROM ubuntu@sha256:562111b88caa79c71391c0c4d3c2114a7d5c35915c20561fd893cb1e07929e6b
ARG SNAPSHOT=20261123T000000Z
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
RUN apt-get update --snapshot ${SNAPSHOT} \
 && apt-get install -y --no-install-recommends --snapshot ${SNAPSHOT} \
    ca-certificates curl xz-utils fontconfig \
    libasound2t64 libatk-bridge2.0-0t64 libatk1.0-0t64 libatspi2.0-0t64 libcairo2 libcups2t64 \
    libdbus-1-3 libdrm2 libgbm1 libglib2.0-0t64 libnspr4 libnss3 libpango-1.0-0 libx11-6 libxcb1 \
    libxcomposite1 libxdamage1 libxext6 libxfixes3 libxkbcommon0 libxrandr2 libxshmfence1 \
    fonts-noto-core fonts-noto-color-emoji fonts-noto-cjk fonts-liberation \
 && rm -rf /var/lib/apt/lists/*
RUN set -e; keep='NotoSans-|NotoSansArabic-|NotoSansHebrew-|NotoColorEmoji|NotoSansCJK-|LiberationMono-'; \
    find /usr/share/fonts -type f \( -name '*.ttf' -o -name '*.otf' -o -name '*.ttc' \) | grep -Ev "/($keep)" | xargs rm -f; \
    find /usr/share/fonts -type f | sort > /fonts.list
RUN curl -fsSL https://nodejs.org/dist/v22.11.0/node-v22.11.0-linux-x64.tar.xz | tar -xJ -C /opt \
 && ln -s /opt/node-v22.11.0-linux-x64/bin/node /usr/local/bin/node \
 && ln -s /opt/node-v22.11.0-linux-x64/bin/npm /usr/local/bin/npm \
 && ln -s /opt/node-v22.11.0-linux-x64/bin/npx /usr/local/bin/npx
WORKDIR /oracle
RUN npm init -y >/dev/null && npm install playwright-core@1.56.1 pngjs@7.0.0 \
 && npx playwright-core install chromium-headless-shell
COPY fonts.conf /etc/fonts/local.conf
RUN fc-cache -f && dpkg-query -W -f '${Package}=${Version}\n' fonts-noto-core fonts-noto-color-emoji fonts-noto-cjk fonts-liberation fontconfig > /fonts.packages && dpkg-query -W -f '${Package}=${Version}\n' > /all.packages
