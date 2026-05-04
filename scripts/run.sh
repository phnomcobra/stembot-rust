#!/usr/bin/bash
set -e
sudo docker-compose build
sudo rm -rf log || true
sudo docker-compose down || true
sudo docker-compose up
