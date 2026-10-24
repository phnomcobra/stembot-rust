#!/usr/bin/bash
set -e
sudo rm -rf log || true
sudo docker-compose down || true
sudo docker-compose build
sudo docker-compose up
