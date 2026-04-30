# prettier-ignore
build: 
  cargo build

start: 
  cargo run

nats_gen: 
  docker run -d --name nats -p 4222:4222 -p 8222:8222 -v nats-data:/data nats:2.10 -js -sd /data

nats_start:
  docker start nats
