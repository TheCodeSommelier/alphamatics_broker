# prettier-ignore
build: 
  cargo build

dev: 
  cargo run src/main.rs

nats: 
  docker run -d --name nats -p 4222:4222 -p 8222:8222 -v nats-data:/data nats:2.10 -js -sd /data
