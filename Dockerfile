# 第一阶段：编译 Rust 后端
FROM rust:1.82-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# 先编译依赖（利用 Docker 缓存）
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src
# 再编译真正的代码
COPY src ./src
RUN touch src/main.rs && cargo build --release

# 第二阶段：构建前端
FROM node:22-alpine AS frontend

WORKDIR /app/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ .
RUN npm run build

# 第三阶段：运行
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 从 builder 复制后端二进制
COPY --from=builder /app/target/release/kid-control3 .

# 从前端阶段复制构建产物
COPY --from=frontend /app/web/dist ./web/dist

# 创建数据目录
RUN mkdir -p /data

ENV DB_PATH=/data/kid-control.db
ENV LISTEN_ADDR=0.0.0.0:18089

EXPOSE 18089

CMD ["./kid-control3"]
