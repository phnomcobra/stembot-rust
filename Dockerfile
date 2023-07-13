FROM rust:latest
RUN apt update
RUN apt install -y rsync